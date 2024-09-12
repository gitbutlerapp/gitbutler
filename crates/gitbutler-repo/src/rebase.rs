use anyhow::{anyhow, Context, Result};
use bstr::ByteSlice;
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use gitbutler_error::error::Marker;

use crate::{LogUntil, RepositoryExt as _};

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
    let ids_to_rebase = ctx
        .repository()
        .l(from_commit_oid, LogUntil::Commit(to_commit_oid))?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id = cherry_rebase_group(
        ctx.repository(),
        target_commit_oid,
        &ids_to_rebase,
        ctx.project().succeeding_rebases,
    )?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
pub fn cherry_rebase_group(
    repository: &git2::Repository,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &[git2::Oid],
    succeeding_rebases: bool,
) -> Result<git2::Oid> {
    // now, rebase unchanged commits onto the new commit
    let commits_to_rebase = ids_to_rebase
        .iter()
        .map(|oid| repository.find_commit(oid.to_owned()))
        .rev()
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read commits to rebase")?;

    let new_head_id = commits_to_rebase
        .into_iter()
        .fold(
            repository
                .find_commit(target_commit_oid)
                .context("failed to find new commit"),
            |head, to_rebase| {
                let head = head?;

                if to_rebase.parent_ids().len() == 1 && head.id() == to_rebase.parent_id(0)? {
                    return Ok(to_rebase);
                };

                let cherrypick_index = repository
                    .cherry_pick_gitbutler(&head, &to_rebase, None)
                    .context("failed to cherry pick")?;

                if cherrypick_index.has_conflicts() {
                    if !succeeding_rebases {
                        return Err(anyhow!("failed to rebase")).context(Marker::BranchConflict);
                    }
                    commit_conflicted_cherry_result(
                        repository,
                        head,
                        to_rebase,
                        cherrypick_index,
                        None,
                    )
                } else {
                    commit_unconflicted_cherry_result(
                        repository,
                        head,
                        to_rebase,
                        cherrypick_index,
                        None,
                    )
                }
            },
        )?
        .id();

    Ok(new_head_id)
}

pub struct OverrideCommitDetails<'a, 'repository> {
    message: &'a str,
    parents: &'a [&'a git2::Commit<'repository>],
    author: &'a git2::Signature<'repository>,
    commiter: &'a git2::Signature<'repository>,
}

fn commit_unconflicted_cherry_result<'repository>(
    repository: &'repository git2::Repository,
    head: git2::Commit<'repository>,
    to_rebase: git2::Commit,
    mut cherrypick_index: git2::Index,
    override_commit_details: Option<OverrideCommitDetails>,
) -> Result<git2::Commit<'repository>> {
    let commit_headers = to_rebase.gitbutler_headers();

    let is_merge_commit = to_rebase.parent_count() > 0;

    let merge_tree_oid = cherrypick_index
        .write_tree_to(repository)
        .context("failed to write merge tree")?;

    // Remove empty merge commits
    if is_merge_commit && merge_tree_oid == head.tree_id() {
        return Ok(head);
    }

    let merge_tree = repository
        .find_tree(merge_tree_oid)
        .context("failed to find merge tree")?;

    let commit_headers = commit_headers.map(|commit_headers| CommitHeadersV2 {
        conflicted: None,
        ..commit_headers
    });

    let commit_oid = if let Some(override_commit_details) = override_commit_details {
        crate::RepositoryExt::commit_with_signature(
            repository,
            None,
            override_commit_details.author,
            override_commit_details.commiter,
            override_commit_details.message,
            &merge_tree,
            override_commit_details.parents,
            commit_headers,
        )
        .context("failed to create commit")?
    } else {
        crate::RepositoryExt::commit_with_signature(
            repository,
            None,
            &to_rebase.author(),
            &to_rebase.committer(),
            &to_rebase.message_bstr().to_str_lossy(),
            &merge_tree,
            &[&head],
            commit_headers,
        )
        .context("failed to create commit")?
    };

    repository
        .find_commit(commit_oid)
        .context("failed to find commit")
}

fn commit_conflicted_cherry_result<'repository>(
    repository: &'repository git2::Repository,
    head: git2::Commit,
    to_rebase: git2::Commit,
    cherrypick_index: git2::Index,
    override_commit_details: Option<OverrideCommitDetails>,
) -> Result<git2::Commit<'repository>> {
    let commit_headers = to_rebase.gitbutler_headers();

    // If the commit we're rebasing is conflicted, use the commits original base.
    let base_tree = if to_rebase.is_conflicted() {
        repository.find_real_tree(&to_rebase, ConflictedTreeKey::Base)?
    } else {
        let base_commit = to_rebase.parent(0)?;
        repository.find_real_tree(&base_commit, Default::default())?
    };

    // in case someone checks this out with vanilla Git, we should warn why it looks like this
    let readme_content =
        b"You have checked out a GitButler Conflicted commit. You probably didn't mean to do this.";
    let readme_blob = repository.blob(readme_content)?;

    let mut conflicted_files = Vec::new();

    // get a list of conflicted files from the index
    let index_conflicts = cherrypick_index.conflicts()?.flatten().collect::<Vec<_>>();
    for conflict in index_conflicts {
        // For some reason we have to resolve the index with the "their" side
        // rather than the "our" side, so we then go and later overwrite the
        // output tree with the "our" side.
        if let Some(their) = conflict.their {
            let path = std::str::from_utf8(&their.path).unwrap().to_string();
            conflicted_files.push(path);
        }
    }

    let mut resolved_index = repository.cherry_pick_gitbutler(
        &head,
        &to_rebase,
        Some(git2::MergeOptions::default().file_favor(git2::FileFavor::Ours)),
    )?;
    let resolved_tree_id = resolved_index.write_tree_to(repository)?;

    // convert files into a string and save as a blob
    let conflicted_files_string = conflicted_files.join("\n");
    let conflicted_files_blob = repository.blob(conflicted_files_string.as_bytes())?;

    // create a treewriter
    let mut tree_writer = repository.treebuilder(None)?;

    let side0 = repository.find_real_tree(&head, Default::default())?;
    let side1 = repository.find_real_tree(&to_rebase, ConflictedTreeKey::Theirs)?;

    // save the state of the conflict, so we can recreate it later
    tree_writer.insert(&*ConflictedTreeKey::Ours, side0.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Theirs, side1.id(), 0o040000)?;
    tree_writer.insert(&*ConflictedTreeKey::Base, base_tree.id(), 0o040000)?;
    tree_writer.insert(
        &*ConflictedTreeKey::AutoResolution,
        resolved_tree_id,
        0o040000,
    )?;
    tree_writer.insert(
        &*ConflictedTreeKey::ConflictFiles,
        conflicted_files_blob,
        0o100644,
    )?;
    tree_writer.insert("README.txt", readme_blob, 0o100644)?;

    let tree_oid = tree_writer.write().context("failed to write tree")?;

    let commit_headers =
        commit_headers
            .or_else(|| Some(Default::default()))
            .map(|commit_headers| {
                let conflicted_file_count = conflicted_files.len().try_into().expect(
                    "If you have more than 2^64 conflicting files, we've got bigger problems",
                );
                CommitHeadersV2 {
                    conflicted: Some(conflicted_file_count),
                    ..commit_headers
                }
            });

    let commit_oid = if let Some(override_commit_details) = override_commit_details {
        crate::RepositoryExt::commit_with_signature(
            repository,
            None,
            override_commit_details.author,
            override_commit_details.commiter,
            override_commit_details.message,
            &repository
                .find_tree(tree_oid)
                .context("failed to find tree")?,
            override_commit_details.parents,
            commit_headers,
        )
        .context("failed to create commit")?
    } else {
        crate::RepositoryExt::commit_with_signature(
            repository,
            None,
            &to_rebase.author(),
            &to_rebase.committer(),
            &to_rebase.message_bstr().to_str_lossy(),
            &repository
                .find_tree(tree_oid)
                .context("failed to find tree")?,
            &[&head],
            commit_headers,
        )
        .context("failed to create commit")?
    };

    repository
        .find_commit(commit_oid)
        .context("failed to find commit")
}

pub fn gitbutler_merge_commits<'repository>(
    repository: &'repository git2::Repository,
    target_commit: git2::Commit<'repository>,
    incoming_commit: git2::Commit<'repository>,
    target_branch_name: &str,
    incoming_branch_name: &str,
) -> Result<git2::Commit<'repository>> {
    let cherrypick_index =
        repository.cherry_pick_gitbutler(&target_commit, &incoming_commit, None)?;

    let (author, committer) = repository.signatures()?;

    let override_commit_details = OverrideCommitDetails {
        message: &format!(
            "Merge branch `{}` into `{}`",
            incoming_branch_name, target_branch_name
        ),
        parents: &[&target_commit.clone(), &incoming_commit.clone()],
        author: &author,
        commiter: &committer,
    };

    if cherrypick_index.has_conflicts() {
        commit_conflicted_cherry_result(
            repository,
            target_commit,
            incoming_commit,
            cherrypick_index,
            Some(override_commit_details),
        )
    } else {
        commit_unconflicted_cherry_result(
            repository,
            target_commit,
            incoming_commit,
            cherrypick_index,
            Some(override_commit_details),
        )
    }
}
