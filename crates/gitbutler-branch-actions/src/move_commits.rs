use crate::{
    branch_trees::checkout_branch_trees, conflicts::RepoConflictsExt, status::get_applied_status,
    VirtualBranchesExt,
};
use anyhow::{anyhow, bail, Context, Result};
use gitbutler_branch::{BranchId, OwnershipClaim};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{rebase::cherry_rebase_group, LogUntil, RepositoryExt};
use std::collections::HashMap;

/// moves commit from the branch it's in to the top of the target branch
pub(crate) fn move_commit(
    ctx: &CommandContext,
    target_branch_id: BranchId,
    commit_id: git2::Oid,
    perm: &mut WorktreeWritePermission,
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
        .find(|(b, _)| b.head() == commit_id)
        .ok_or_else(|| anyhow!("commit {commit_id} to be moved could not be found"))?;

    let source_branch_non_comitted_files = source_status;

    let source_commit = ctx
        .repository()
        .find_commit(commit_id)
        .context("failed to find commit")?;

    if source_commit.is_conflicted() {
        bail!("Can not move conflicted commits");
    }

    let source_branch_head_parent = source_commit
        .parent(0)
        .context("failed to get parent commit")?;
    let source_branch_head_tree = source_commit.tree().context("failed to get commit tree")?;
    let source_branch_head_parent_tree = source_branch_head_parent
        .tree()
        .context("failed to get parent tree")?;
    let branch_head_diff = gitbutler_diff::trees(
        ctx.repository(),
        &source_branch_head_parent_tree,
        &source_branch_head_tree,
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

    let branch_head_diff: HashMap<_, _> =
        gitbutler_diff::diff_files_into_hunks(branch_head_diff).collect();
    let is_source_locked = check_source_lock(source_branch_non_comitted_files, &branch_head_diff);

    let mut ancestor_commits = ctx.repository().log(
        source_branch_head_parent.id(),
        LogUntil::Commit(merge_base.id()),
    )?;
    ancestor_commits.push(merge_base);

    let ancestor_commits = ancestor_commits;

    let is_ancestor_locked =
        check_source_lock_to_ancestors(ctx.repository(), ancestor_commits, &branch_head_diff);

    if is_source_locked {
        bail!("the source branch contains hunks locked to the target commit")
    }

    if is_ancestor_locked {
        bail!("the source branch contains hunks locked to the target commit ancestors")
    }

    // move files ownerships from source branch to the destination branch

    let ownerships_to_transfer = branch_head_diff
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

    // reset the source branch to the parent commit
    // and update the destination branch head
    source_branch.set_head(source_branch_head_parent.id());
    vb_state.set_branch(source_branch.clone())?;

    destination_branch.set_head(new_destination_head_oid);
    vb_state.set_branch(destination_branch.clone())?;

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
            .map_or(false, |head_diff_hunks| {
                file.hunks.iter().any(|hunk| {
                    let hunk: gitbutler_diff::GitHunk = hunk.clone().into();
                    head_diff_hunks.iter().any(|head_hunk| {
                        lines_overlap(
                            head_hunk.new_start,
                            head_hunk.new_start + head_hunk.new_lines,
                            hunk.new_start,
                            hunk.new_start + hunk.new_lines,
                        )
                    })
                })
            })
    });
    is_source_locked
}

/// determines if the source commit is locked to any of its ancestors
fn check_source_lock_to_ancestors(
    repository: &git2::Repository,
    ancestor_commits: Vec<git2::Commit>,
    source_commit_diff: &HashMap<std::path::PathBuf, Vec<gitbutler_diff::GitHunk>>,
) -> bool {
    let mut previous: Option<git2::Commit> = None;

    for ancestor_commit in ancestor_commits {
        if previous.is_none() {
            previous = Some(ancestor_commit);
            continue;
        }

        let previous_commit = previous.take().unwrap();

        let old_tree = ancestor_commit.tree().unwrap();
        let new_tree = previous_commit.tree().unwrap();

        let diff = gitbutler_diff::trees(repository, &old_tree, &new_tree);

        if diff.is_err() {
            previous = Some(ancestor_commit);
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

        previous = Some(ancestor_commit);
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
