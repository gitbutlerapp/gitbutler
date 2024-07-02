use crate::git::HasCommitHeaders;
use crate::{error::Marker, git::CommitExt, git::RepositoryExt, project_repository};
use anyhow::{anyhow, Context, Result};
use bstr::ByteSlice;

/// cherry-pick based rebase, which handles empty commits
/// this function takes a commit range and generates a Vector of commit oids
/// and then passes them to `cherry_rebase_group` to rebase them onto the target commit
pub fn cherry_rebase(
    project_repository: &project_repository::Repository,
    target_commit_oid: git2::Oid,
    start_commit_oid: git2::Oid,
    end_commit_oid: git2::Oid,
) -> Result<Option<git2::Oid>> {
    // get a list of the commits to rebase
    let mut ids_to_rebase = project_repository.l(
        end_commit_oid,
        project_repository::LogUntil::Commit(start_commit_oid),
    )?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id =
        cherry_rebase_group(project_repository, target_commit_oid, &mut ids_to_rebase)?;

    Ok(Some(new_head_id))
}

/// takes a vector of commit oids and rebases them onto a target commit and returns the
/// new head commit oid if it's successful
/// the difference between this and a libgit2 based rebase is that this will successfully
/// rebase empty commits (two commits with identical trees)
pub fn cherry_rebase_group(
    project_repository: &project_repository::Repository,
    target_commit_oid: git2::Oid,
    ids_to_rebase: &mut [git2::Oid],
) -> Result<git2::Oid> {
    ids_to_rebase.reverse();
    // now, rebase unchanged commits onto the new commit
    let commits_to_rebase = ids_to_rebase
        .iter()
        .map(|oid| project_repository.repo().find_commit(oid.to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read commits to rebase")?;

    let new_head_id = commits_to_rebase
        .into_iter()
        .fold(
            project_repository
                .repo()
                .find_commit(target_commit_oid)
                .context("failed to find new commit"),
            |head, to_rebase| {
                let head = head?;

                let mut cherrypick_index = project_repository
                    .repo()
                    .cherrypick_commit(&to_rebase, &head, 0, None)
                    .context("failed to cherry pick")?;

                if cherrypick_index.has_conflicts() {
                    return Err(anyhow!("failed to rebase")).context(Marker::BranchConflict);
                }

                let merge_tree_oid = cherrypick_index
                    .write_tree_to(project_repository.repo())
                    .context("failed to write merge tree")?;

                let merge_tree = project_repository
                    .repo()
                    .find_tree(merge_tree_oid)
                    .context("failed to find merge tree")?;

                let commit_headers = to_rebase.gitbutler_headers();

                let commit_oid = project_repository
                    .repo()
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

                project_repository
                    .repo()
                    .find_commit(commit_oid)
                    .context("failed to find commit")
            },
        )?
        .id();

    Ok(new_head_id)
}
