use anyhow::{bail, Context, Result};
use gitbutler_branch::{
    Branch, BranchCreateRequest, BranchId, BranchOwnershipClaims, OwnershipClaim,
};
use gitbutler_command_context::ProjectRepository;
use gitbutler_diff::{diff_files_into_hunks, GitHunk, Hunk, HunkHash};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::RepositoryExt;
use md5::Digest;
use std::{collections::HashMap, path::PathBuf, vec};

use crate::{
    conflicts::RepoConflictsExt, integration::get_workspace_head, write_tree, BranchManagerExt,
    HunkLock, VirtualBranchesExt,
};

pub struct VirtualBranchesStatus {
    pub branches: Vec<(Branch, HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>)>,
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
    pub locks: HashMap<Digest, Vec<HunkLock>>,
}

// Returns branches and their associated file changes, in addition to a list
// of skipped files.
// TODO(kv): make this side effect free
pub fn get_applied_status(
    project_repository: &ProjectRepository,
    perm: Option<&mut WorktreeWritePermission>,
) -> Result<VirtualBranchesStatus> {
    let integration_commit = get_workspace_head(project_repository)?;
    let mut virtual_branches = project_repository
        .project()
        .virtual_branches()
        .list_branches_in_workspace()?;
    let base_file_diffs =
        gitbutler_diff::workdir(project_repository.repo(), &integration_commit.to_owned())
            .context("failed to diff workdir")?;

    let mut skipped_files: Vec<gitbutler_diff::FileDiff> = Vec::new();
    for file_diff in base_file_diffs.values() {
        if file_diff.skipped {
            skipped_files.push(file_diff.clone());
        }
    }
    let mut base_diffs: HashMap<_, _> = diff_files_into_hunks(base_file_diffs).collect();

    // sort by order, so that the default branch is first (left in the ui)
    virtual_branches.sort_by(|a, b| a.order.cmp(&b.order));

    let branch_manager = project_repository.branch_manager();

    if virtual_branches.is_empty() && !base_diffs.is_empty() {
        if let Some(perm) = perm {
            virtual_branches = vec![branch_manager
                .create_virtual_branch(&BranchCreateRequest::default(), perm)
                .context("failed to create default branch")?];
        } else {
            bail!("Would have to create virtual-branch but write permissions aren't available")
        }
    }

    let mut diffs_by_branch: HashMap<BranchId, HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>> =
        virtual_branches
            .iter()
            .map(|branch| (branch.id, HashMap::new()))
            .collect();

    let locks = new_compute_locks(project_repository.repo(), &base_diffs, &virtual_branches)?;

    for branch in &mut virtual_branches {
        let old_claims = branch.ownership.claims.clone();
        let new_claims = old_claims
            .iter()
            .filter_map(|claim| {
                let git_diff_hunks = match base_diffs.get_mut(&claim.file_path) {
                    None => return None,
                    Some(hunks) => hunks,
                };

                let claimed_hunks: Vec<Hunk> = claim
                    .hunks
                    .iter()
                    .filter_map(|claimed_hunk| {
                        // if any of the current hunks intersects with the owned hunk, we want to keep it
                        for (i, git_diff_hunk) in git_diff_hunks.iter().enumerate() {
                            if claimed_hunk == &Hunk::from(git_diff_hunk)
                                || claimed_hunk.intersects(git_diff_hunk)
                            {
                                let hash = Hunk::hash_diff(&git_diff_hunk.diff_lines);
                                if locks.contains_key(&hash) {
                                    return None; // Defer allocation to unclaimed hunks processing
                                }
                                diffs_by_branch
                                    .entry(branch.id)
                                    .or_default()
                                    .entry(claim.file_path.clone())
                                    .or_default()
                                    .push(git_diff_hunk.clone());
                                let updated_hunk = Hunk {
                                    start: git_diff_hunk.new_start,
                                    end: git_diff_hunk.new_start + git_diff_hunk.new_lines,
                                    hash: Some(hash),
                                };
                                git_diff_hunks.remove(i);
                                return Some(updated_hunk);
                            }
                        }
                        None
                    })
                    .collect();

                if claimed_hunks.is_empty() {
                    // No need for an empty claim
                    None
                } else {
                    Some(OwnershipClaim {
                        file_path: claim.file_path.clone(),
                        hunks: claimed_hunks,
                    })
                }
            })
            .collect();

        branch.ownership = BranchOwnershipClaims { claims: new_claims };
    }

    let max_selected_for_changes = virtual_branches
        .iter()
        .filter_map(|b| b.selected_for_changes)
        .max()
        .unwrap_or(-1);
    let default_vbranch_pos = virtual_branches
        .iter()
        .position(|b| b.selected_for_changes == Some(max_selected_for_changes))
        .unwrap_or(0);

    // Everything claimed has been removed from `base_diffs`, here we just
    // process the remaining ones.
    for (filepath, hunks) in base_diffs {
        for hunk in hunks {
            let hash = Hunk::hash_diff(&hunk.diff_lines);
            let locked_to = locks.get(&hash);

            let vbranch_pos = if let Some(locks) = locked_to {
                let p = virtual_branches
                    .iter()
                    .position(|vb| vb.id == locks[0].branch_id);
                match p {
                    Some(p) => p,
                    _ => default_vbranch_pos,
                }
            } else {
                default_vbranch_pos
            };

            virtual_branches[vbranch_pos].ownership.put(OwnershipClaim {
                file_path: filepath.clone(),
                hunks: vec![Hunk::from(&hunk).with_hash(Hunk::hash_diff(&hunk.diff_lines))],
            });

            diffs_by_branch
                .entry(virtual_branches[vbranch_pos].id)
                .or_default()
                .entry(filepath.clone())
                .or_default()
                .push(hunk);
        }
    }

    let mut hunks_by_branch = diffs_by_branch
        .into_iter()
        .map(|(branch_id, hunks)| {
            (
                virtual_branches
                    .iter()
                    .find(|b| b.id.eq(&branch_id))
                    .unwrap()
                    .clone(),
                hunks,
            )
        })
        .collect::<Vec<_>>();

    // write updated state if not resolving
    if !project_repository.is_resolving() {
        let vb_state = project_repository.project().virtual_branches();
        for (vbranch, files) in &mut hunks_by_branch {
            vbranch.tree = write_tree(project_repository, &vbranch.head, files)?;
            vb_state
                .set_branch(vbranch.clone())
                .context(format!("failed to write virtual branch {}", vbranch.name))?;
        }
    }

    Ok(VirtualBranchesStatus {
        branches: hunks_by_branch,
        skipped_files,
        locks,
    })
}

fn new_compute_locks(
    repository: &git2::Repository,
    unstaged_hunks_by_path: &HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>,
    virtual_branches: &[Branch],
) -> Result<HashMap<HunkHash, Vec<HunkLock>>> {
    // If we cant find the integration commit and subsequently the target commit, we can't find any locks
    let target_tree = repository.target_commit()?.tree()?;

    let mut diff_opts = git2::DiffOptions::new();
    let opts = diff_opts
        .show_binary(true)
        .ignore_submodules(true)
        .context_lines(3);

    let branch_path_diffs = virtual_branches
        .iter()
        .filter_map(|branch| {
            let commit = repository.find_commit(branch.head).ok()?;
            let tree = commit.tree().ok()?;
            let diff = repository
                .diff_tree_to_tree(Some(&target_tree), Some(&tree), Some(opts))
                .ok()?;
            let hunks_by_filepath =
                gitbutler_diff::hunks_by_filepath(Some(repository), &diff).ok()?;

            Some((branch, hunks_by_filepath))
        })
        .collect::<Vec<_>>();

    let mut integration_hunks_by_path =
        HashMap::<PathBuf, Vec<(gitbutler_diff::GitHunk, &Branch)>>::new();

    for (branch, hunks_by_filepath) in branch_path_diffs {
        for (path, hunks) in hunks_by_filepath {
            integration_hunks_by_path.entry(path).or_default().extend(
                hunks
                    .hunks
                    .iter()
                    .map(|hunk| (hunk.clone(), branch))
                    .collect::<Vec<_>>(),
            );
        }
    }

    let locked_hunks = unstaged_hunks_by_path
        .iter()
        .filter_map(|(path, hunks)| {
            let integration_hunks = integration_hunks_by_path.get(path)?;

            let (unapplied_hunk, branches) = hunks.iter().find_map(|unapplied_hunk| {
                // Find all branches that have a hunk that intersects with the unapplied hunk
                let locked_to = integration_hunks
                    .iter()
                    .filter_map(|(integration_hunk, branch)| {
                        if GitHunk::integration_intersects_unapplied(
                            integration_hunk,
                            unapplied_hunk,
                        ) {
                            Some(*branch)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if locked_to.is_empty() {
                    None
                } else {
                    Some((unapplied_hunk, locked_to))
                }
            })?;

            let hash = Hunk::hash_diff(&unapplied_hunk.diff_lines);
            let locks = branches
                .iter()
                .map(|b| HunkLock {
                    branch_id: b.id,
                    commit_id: b.head,
                })
                .collect::<Vec<_>>();

            // For now we're returning an array of locks to align with the original type, even though this implementation doesn't give multiple locks for the same hunk
            Some((hash, locks))
        })
        .collect::<HashMap<_, _>>();

    Ok(locked_hunks)
}
