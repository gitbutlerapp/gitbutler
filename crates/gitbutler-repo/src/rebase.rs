use anyhow::{Context, Result};
use bstr::ByteSlice;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};

use crate::{LogUntil, RepoActionsExt, RepositoryExt};

/// cherry-pick based rebase, which handles empty commits
/// this function takes a commit range and generates a Vector of commit oids
/// and then passes them to `cherry_rebase_group` to rebase them onto the target commit
///
/// Returns the new head commit id
pub fn cherry_rebase(
    ctx: &CommandContext,
    target_commit_oid: git2::Oid,
    to_commit_oid: git2::Oid,
    from_commit_oid: git2::Oid,
) -> Result<Option<git2::Oid>> {
    // get a list of the commits to rebase
    let mut ids_to_rebase = ctx.l(from_commit_oid, LogUntil::Commit(to_commit_oid))?;

    dbg!(&ids_to_rebase);

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id = cherry_rebase_group(ctx, target_commit_oid, &mut ids_to_rebase)?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
pub fn cherry_rebase_group(
    ctx: &CommandContext,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &mut [git2::Oid],
) -> Result<git2::Oid> {
    ids_to_rebase.reverse();
    // now, rebase unchanged commits onto the new commit
    let commits_to_rebase = ids_to_rebase
        .iter()
        .map(|oid| ctx.repository().find_commit(oid.to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read commits to rebase")?;

    let repository = ctx.repository();

    let new_head_id = commits_to_rebase
        .into_iter()
        .fold(
            repository
                .find_commit(target_commit_oid)
                .context("failed to find new commit"),
            |head, to_rebase| {
                let head = head?;

                let mut cherrypick_index = repository
                    .cherry_pick_gitbutler(&head, &to_rebase)
                    .context("failed to cherry pick")?;

                let commit_headers = to_rebase.gitbutler_headers();

                if cherrypick_index.has_conflicts() {
                    // return Err(anyhow!("failed to rebase")).context(Marker::BranchConflict);

                    // there is a merge conflict, let's store the state so they can fix it later
                    // store tree as the same as the parent
                    let merge_base_commit_oid = repository
                        .merge_base(head.id(), to_rebase.id())
                        .context("failed to find merge base")?;
                    let merge_base_commit = repository
                        .find_commit(merge_base_commit_oid)
                        .context("failed to find merge base commit")?;
                    let base_tree = repository.find_real_tree(&merge_base_commit, None)?;

                    // in case someone checks this out with vanilla Git, we should warn why it looks like this
                    let readme_content = b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
                    let readme_blob = repository.blob(readme_content)?;

                    // get a list of conflicted files from the index
                    let mut conflicted_files = Vec::new();
                    let index_conflicts = cherrypick_index.conflicts()?;
                    index_conflicts.for_each(|conflict| {
                        if let Ok(conflict_file) = conflict {
                            if let Some(ours) = conflict_file.our {
                                let path = std::str::from_utf8(&ours.path).unwrap().to_string();
                                conflicted_files.push(path)
                            }
                        }
                    });
                    // convert files into a string and save as a blob
                    let conflicted_files_string = conflicted_files.join("\n");
                    let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

                    // create a treewriter
                    let mut tree_writer = repository.treebuilder(None)?;

                    let side0 = repository.find_real_tree(&head, None)?;
                    let side1 = repository.find_real_tree(&to_rebase, Some(".conflict-side-1".to_string()))?;

                    // save the state of the conflict, so we can recreate it later
                    tree_writer.insert(".conflict-side-0", side0.id(), 0o040000)?;
                    tree_writer.insert(".conflict-side-1", side1.id(), 0o040000)?;
                    tree_writer.insert(".conflict-base-0", base_tree.id(), 0o040000)?;
                    tree_writer.insert(".conflict-files", conflicted_files_blob, 0o100644)?;
                    tree_writer.insert("README.txt", readme_blob, 0o100644)?;

                    let tree_oid = tree_writer.write().context("failed to write tree")?;

                    let commit_headers = commit_headers.map(|commit_headers| {
                        let conflicted_file_count = conflicted_files
                            .len()
                            .try_into()
                            .expect("If you have more than 2^64 conflicting files, we've got bigger problems");
                        CommitHeadersV2 { conflicted: Some(conflicted_file_count), ..commit_headers}
                    });

                    // write a commit
                    let commit_oid = repository
                        .commit_with_signature(
                            None,
                            &to_rebase.author(),
                            &to_rebase.committer(),
                            &to_rebase.message_bstr().to_str_lossy(),
                            &repository
                                .find_tree(tree_oid)
                                .context("failed to find tree")?,
                            &[&head],
                            commit_headers
                        )
                        .context("failed to create commit")?;

                    repository
                        .find_commit(commit_oid)
                        .context("failed to find commit")
                } else {
                    let merge_tree_oid = cherrypick_index
                        .write_tree_to(ctx.repository())
                        .context("failed to write merge tree")?;

                    let merge_tree = ctx
                        .repository()
                        .find_tree(merge_tree_oid)
                        .context("failed to find merge tree")?;

                    let commit_oid = ctx
                        .repository()
                        .commit_with_signature(
                            None,
                            &to_rebase.author(),
                            &to_rebase.committer(),
                            &to_rebase.message_bstr().to_str_lossy(),
                            &merge_tree,
                            &[&head],
                            commit_headers,
                        )
                        .context("failed to create commit")?;

                    ctx.repository()
                        .find_commit(commit_oid)
                        .context("failed to find commit")
                }
            },
        )?
        .id();

    Ok(new_head_id)
}
