use crate::{
    branch_trees::checkout_branch_trees, conflicts::RepoConflictsExt, status::get_applied_status,
    VirtualBranchesExt,
};
use anyhow::{anyhow, bail, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{rebase::cherry_rebase_group, LogUntil, RepositoryExt};
use gitbutler_stack::{OwnershipClaim, StackId};
use std::collections::HashMap;

/// moves commit from the branch it's in to the top of the target branch
pub(crate) fn move_commit(
    ctx: &CommandContext,
    target_branch_id: StackId,
    commit_id: git2::Oid,
    perm: &mut WorktreeWritePermission,
    source_branch_id: StackId,
) -> Result<()> {
    ctx.assure_resolved()?;
    let vb_state = ctx.project().virtual_branches();

    let applied_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_branches.iter().any(|b| b.id == target_branch_id) {
        bail!("branch {target_branch_id} is not among applied branches")
    }

    let mut applied_statuses = get_applied_status(ctx, None)?.branches;

    let (ref mut source_branch, source_status) = applied_statuses
        .iter_mut()
        .find(|(b, _)| b.id == source_branch_id)
        .ok_or_else(|| anyhow!("the source branch could not be found"))?;

    let is_head_commit = commit_id == source_branch.head();
    let source_branch_non_comitted_files = source_status;

    let source_commit = ctx
        .repository()
        .find_commit(commit_id)
        .with_context(|| format!("commit {commit_id} to be moved could not be found"))?;

    if source_commit.is_conflicted() {
        bail!("Can not move conflicted commits");
    }

    let source_commit_parent = source_commit
        .parent(0)
        .context("failed to get parent commit")?;
    let source_commit_tree = source_commit.tree().context("failed to get commit tree")?;
    let source_commit_parent_tree = source_commit_parent
        .tree()
        .context("failed to get parent tree")?;
    let source_commit_diff = gitbutler_diff::trees(
        ctx.repository(),
        &source_commit_parent_tree,
        &source_commit_tree,
    )?;

    let default_target = vb_state.get_default_target()?;
    let merge_base = ctx
        .repository()
        .merge_base(default_target.sha, commit_id)
        .context("failed to find merge base")?;
    let merge_base = ctx
        .repository()
        .find_commit(merge_base)
        .context("failed to find merge base")?;

    let source_commit_diff: HashMap<_, _> =
        gitbutler_diff::diff_files_into_hunks(source_commit_diff).collect();
    let is_source_locked = check_source_lock(source_branch_non_comitted_files, &source_commit_diff);

    let mut ancestor_commits = ctx.repository().log(
        source_commit_parent.id(),
        LogUntil::Commit(merge_base.id()),
        false,
    )?;
    ancestor_commits.push(merge_base);
    let ancestor_commits = ancestor_commits;

    let mut descendant_commits = None;
    if !is_head_commit {
        descendant_commits = Some(ctx.repository().log(
            source_branch.head(),
            LogUntil::Commit(commit_id),
            false,
        )?);
    }

    let is_ancestor_locked =
        check_source_lock_to_commits(ctx.repository(), &ancestor_commits, &source_commit_diff);

    if is_source_locked {
        bail!("the source branch contains hunks locked to the target commit")
    }

    if is_ancestor_locked {
        bail!("the target commit contains hunks locked to its ancestors")
    }

    if let Some(commits_to_check) = descendant_commits.as_mut() {
        // we append the source commit so that we can create the diff between
        // the source commit and its first descendant
        let mut commits_to_check = commits_to_check.clone();
        commits_to_check.push(source_commit.clone());
        let is_descendant_locked =
            check_source_lock_to_commits(ctx.repository(), &commits_to_check, &source_commit_diff);

        if is_descendant_locked {
            bail!("the target commit contains hunks locked to its descendants")
        }
    }

    // move files ownerships from source branch to the destination branch

    let ownerships_to_transfer = source_commit_diff
        .iter()
        .map(|(file_path, hunks)| {
            (
                file_path.clone(),
                hunks.iter().map(Into::into).collect::<Vec<_>>(),
            )
        })
        .map(|(file_path, hunks)| OwnershipClaim { file_path, hunks })
        .flat_map(|file_ownership| source_branch.ownership.take(&file_ownership))
        .collect::<Vec<_>>();

    // move the commit to destination branch target branch

    let mut destination_branch = vb_state.get_branch_in_workspace(target_branch_id)?;

    for ownership in ownerships_to_transfer {
        destination_branch.ownership.put(ownership);
    }

    let new_destination_head_oid = cherry_rebase_group(
        ctx.repository(),
        destination_branch.head(),
        &[source_commit.id()],
    )?;

    // if the source commit has children, move them to the source commit's parent

    let mut new_source_head_oid = source_commit_parent.id();
    if let Some(child_commits) = descendant_commits.as_ref() {
        let ids_to_rebase: Vec<git2::Oid> = child_commits.iter().map(|c| c.id()).collect();
        new_source_head_oid =
            cherry_rebase_group(ctx.repository(), source_commit_parent.id(), &ids_to_rebase)?;
    }

    // reset the source branch to the newer parent commit
    // and update the destination branch head
    source_branch.set_stack_head(ctx, new_source_head_oid, None)?;
    vb_state.set_branch(source_branch.clone())?;

    destination_branch.set_stack_head(ctx, new_destination_head_oid, None)?;

    checkout_branch_trees(ctx, perm)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

/// determines if the uncommitted files are locked to commit
fn check_source_lock(
    source_branch_non_comitted_files: &[crate::file::VirtualBranchFile],
    source_commit_diff: &HashMap<std::path::PathBuf, Vec<gitbutler_diff::GitHunk>>,
) -> bool {
    let is_source_locked = source_branch_non_comitted_files.iter().any(|file| {
        source_commit_diff
            .get(&file.path)
            .map_or(false, |source_diff_hunks| {
                file.hunks.iter().any(|hunk| {
                    let hunk: gitbutler_diff::GitHunk = hunk.clone().into();
                    source_diff_hunks.iter().any(|source_hunk| {
                        lines_overlap(
                            source_hunk.new_start,
                            source_hunk.new_start + source_hunk.new_lines,
                            hunk.new_start,
                            hunk.new_start + hunk.new_lines,
                        )
                    })
                })
            })
    });
    is_source_locked
}

/// determines if the source commit is locked to any commits
///
/// The commits are used to calculate the diffs between them in the following way:
/// - Let A be the source commit and B, C its ancestors.
/// - `source_commit_diff` is the diff between A and B
/// - `commits` is a list of commits [B, C]
/// - This function calculates the  diff between B and C check it against the hunks in `source_commit_diff`
fn check_source_lock_to_commits(
    repository: &git2::Repository,
    commits: &Vec<git2::Commit>,
    source_commit_diff: &HashMap<std::path::PathBuf, Vec<gitbutler_diff::GitHunk>>,
) -> bool {
    let mut previous: Option<&git2::Commit> = None;

    for commit in commits {
        if previous.is_none() {
            previous = Some(commit);
            continue;
        }

        let previous_commit = previous.take().unwrap();

        let old_tree = commit.tree().unwrap();
        let new_tree = previous_commit.tree().unwrap();

        let diff = gitbutler_diff::trees(repository, &old_tree, &new_tree);

        if diff.is_err() {
            previous = Some(commit);
            continue;
        }

        let diff = diff.unwrap();
        let diff: HashMap<_, _> = gitbutler_diff::diff_files_into_hunks(diff).collect();

        let is_source_locked = diff.iter().any(|(file_path, hunks)| {
            source_commit_diff
                .get(file_path)
                .map_or(false, |source_hunks| {
                    hunks.iter().any(|hunk| {
                        source_hunks.iter().any(|source_hunk| {
                            lines_overlap(
                                hunk.new_start,
                                hunk.new_start + hunk.new_lines,
                                source_hunk.new_start,
                                source_hunk.new_start + source_hunk.new_lines,
                            )
                        })
                    })
                })
        });

        if is_source_locked {
            return true;
        }

        previous = Some(commit);
    }

    false
}

fn lines_overlap(start_a: u32, end_a: u32, start_b: u32, end_b: u32) -> bool {
    ((start_a >= start_b && start_a <= end_b) || (end_a >= start_b && end_a <= end_b))
        || ((start_b >= start_a && start_b <= end_a) || (end_b >= start_a && end_b <= end_a))
}

#[cfg(test)]
mod tests {
    use gitbutler_diff::Hunk;

    use crate::hunk::VirtualBranchHunk;

    use super::*;

    fn create_virtual_branch_files(
        path: &str,
        start: u32,
        end: u32,
    ) -> Vec<crate::file::VirtualBranchFile> {
        let source_branch_non_comitted_files = vec![crate::file::VirtualBranchFile {
            id: path.to_string(),
            path: path.into(),
            hunks: vec![VirtualBranchHunk {
                id: "1-2".into(),
                diff: "".into(),
                modified_at: 0,
                file_path: path.into(),
                old_start: 0,
                old_lines: 0,
                start,
                end,
                binary: false,
                hash: Hunk::hash_diff("".as_bytes()),
                locked: false,
                locked_to: None,
                change_type: gitbutler_diff::ChangeType::Modified,
                poisoned: false,
            }],
            modified_at: 0,
            conflicted: false,
            binary: false,
            large: false,
        }];
        source_branch_non_comitted_files
    }

    fn create_source_commit_diff(
        path: &str,
        new_start: u32,
        new_lines: u32,
    ) -> HashMap<std::path::PathBuf, Vec<gitbutler_diff::GitHunk>> {
        let source_commit_diff: HashMap<_, _> = vec![(
            path.into(),
            vec![gitbutler_diff::GitHunk {
                old_start: 0,
                old_lines: 0,
                new_start,
                new_lines,
                diff_lines: "".into(),
                binary: false,
                change_type: gitbutler_diff::ChangeType::Modified,
            }],
        )]
        .into_iter()
        .collect();
        source_commit_diff
    }

    #[test]
    fn lines_overlap_test() {
        assert!(!lines_overlap(1, 2, 3, 4));
        assert!(lines_overlap(1, 4, 2, 3));
        assert!(lines_overlap(2, 3, 1, 4));
        assert!(!lines_overlap(3, 4, 1, 2));

        assert!(lines_overlap(1, 2, 2, 3));
        assert!(lines_overlap(1, 3, 2, 3));
        assert!(lines_overlap(2, 3, 1, 2));

        assert!(!lines_overlap(1, 1, 2, 2));
        assert!(lines_overlap(1, 1, 1, 1));
        assert!(lines_overlap(1, 1, 1, 2));
        assert!(lines_overlap(1, 2, 2, 2));
    }

    #[test]
    fn check_source_lock_test_not_locked_same_file() {
        let path: &str = "foo.txt";

        let source_branch_non_comitted_files = create_virtual_branch_files(path, 1, 2);
        let source_commit_diff = create_source_commit_diff(path, 3, 1);

        assert!(!check_source_lock(
            &source_branch_non_comitted_files,
            &source_commit_diff
        ));
    }

    #[test]
    fn check_source_lock_test_not_locked_different_file() {
        let path_1: &str = "foo.txt";
        let path_2: &str = "bar.txt";

        let source_branch_non_comitted_files = create_virtual_branch_files(path_1, 1, 2);
        let source_commit_diff = create_source_commit_diff(path_2, 1, 1);

        assert!(!check_source_lock(
            &source_branch_non_comitted_files,
            &source_commit_diff
        ));
    }

    #[test]
    fn check_source_lock_test_locked_exact_lines() {
        let path: &str = "foo.txt";

        let source_branch_non_comitted_files = create_virtual_branch_files(path, 1, 2);
        let source_commit_diff = create_source_commit_diff(path, 1, 1);

        assert!(check_source_lock(
            &source_branch_non_comitted_files,
            &source_commit_diff
        ));
    }

    #[test]
    fn check_source_lock_test_locked_overlapping_files() {
        let path: &str = "foo.txt";

        let source_branch_non_comitted_files = create_virtual_branch_files(path, 1, 4);
        let source_commit_diff = create_source_commit_diff(path, 3, 4);

        assert!(check_source_lock(
            &source_branch_non_comitted_files,
            &source_commit_diff
        ));
    }

    #[test]
    fn check_source_lock_test_locked_containing_lines() {
        let path: &str = "foo.txt";

        let source_branch_non_comitted_files = create_virtual_branch_files(path, 1, 4);
        let source_commit_diff = create_source_commit_diff(path, 1, 2);

        assert!(check_source_lock(
            &source_branch_non_comitted_files,
            &source_commit_diff
        ));
    }
}
