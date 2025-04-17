use std::{collections::HashMap, path::PathBuf, vec};

use crate::branch_manager::BranchManagerExt;
use crate::dependencies::compute_workspace_dependencies;
use crate::integration::get_workspace_head;
use crate::VirtualBranchesExt;
use crate::{
    file::{virtual_hunks_into_virtual_files, VirtualBranchFile},
    hunk::{file_hunks_from_diffs, VirtualBranchHunk},
};
use anyhow::{bail, Context, Result};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::{diff_files_into_hunks, Hunk};
use gitbutler_hunk_dependency::locks::HunkDependencyResult;
use gitbutler_operating_modes::assure_open_workspace_mode;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{BranchOwnershipClaims, OwnershipClaim, Stack, StackId};
use tracing::instrument;

/// Represents the uncommitted status of the applied virtual branches in the workspace.
#[derive(Debug)]
pub struct VirtualBranchesStatus {
    /// A collection of branches and their associated uncommitted file changes.
    pub branches: Vec<(Stack, Vec<VirtualBranchFile>)>,
    /// A collection of files that were skipped during the diffing process (due to being very large and unprocessable).
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
    /// The dependency result for the workspace.
    pub workspace_dependencies: HunkDependencyResult,
}

pub fn get_applied_status(
    ctx: &CommandContext,
    perm: Option<&mut WorktreeWritePermission>,
) -> Result<VirtualBranchesStatus> {
    let diffs = gitbutler_diff::workdir(ctx.repo(), get_workspace_head(ctx)?)?;
    get_applied_status_cached(ctx, perm, &diffs)
}

/// Returns branches and their associated file changes, in addition to a list
/// of skipped files.
/// `worktree_changes` are all changed files against the current `HEAD^{tree}` and index
/// against the current working tree directory, and it's used to avoid double-computing
/// this expensive information.
// TODO(kv): make this side effect free
#[instrument(level = tracing::Level::DEBUG, skip(ctx, perm, worktree_changes))]
pub fn get_applied_status_cached(
    ctx: &CommandContext,
    perm: Option<&mut WorktreeWritePermission>,
    worktree_changes: &gitbutler_diff::DiffByPathMap,
) -> Result<VirtualBranchesStatus> {
    assure_open_workspace_mode(ctx).context("ng applied status requires open workspace mode")?;
    let mut virtual_branches = ctx
        .project()
        .virtual_branches()
        .list_stacks_in_workspace()?;

    let mut skipped_files: Vec<gitbutler_diff::FileDiff> = Vec::new();
    for file_diff in worktree_changes.values() {
        if file_diff.skipped {
            skipped_files.push(file_diff.clone());
        }
    }
    let mut base_diffs: HashMap<_, _> = diff_files_into_hunks(worktree_changes).collect();

    // sort by order, so that the default branch is first (left in the ui)
    virtual_branches.sort_by(|a, b| a.order.cmp(&b.order));

    let branch_manager = ctx.branch_manager();

    if virtual_branches.is_empty() && !base_diffs.is_empty() {
        if let Some(perm) = perm {
            virtual_branches = vec![branch_manager
                .create_virtual_branch(&BranchCreateRequest::default(), perm)
                .context("failed to create default branch")?];
        } else {
            bail!("Would have to create virtual-branch but write permissions aren't available")
        }
    }

    let mut diffs_by_branch: HashMap<StackId, HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>> =
        virtual_branches
            .iter()
            .map(|branch| (branch.id, HashMap::new()))
            .collect();

    let vb_state = ctx.project().virtual_branches();
    let default_target = vb_state.get_default_target()?;

    let workspace_dependencies =
        compute_workspace_dependencies(ctx, &default_target.sha, &base_diffs, &virtual_branches)?;

    let diff_dependencies = &workspace_dependencies.diffs;

    for branch in &mut virtual_branches {
        // This should never be invoked. But if it is, dont try to  make the branch name unique
        if let Err(e) = branch.initialize(ctx, true) {
            tracing::warn!("failed to initialize stack: {:?}", e);
        }
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
                            if claimed_hunk.intersects(git_diff_hunk) {
                                let hash = Hunk::hash_diff(&git_diff_hunk.diff_lines);
                                if diff_dependencies.contains_key(&hash) {
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
                                    hunk_header: None,
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
            let locked_to = diff_dependencies.get(&hash);

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
    let repo = ctx.gix_repo()?;
    for (vbranch, files) in &mut hunks_by_branch {
        vbranch.set_tree(gitbutler_diff::write::hunks_onto_oid(
            ctx,
            vbranch.head(&repo)?.to_git2(),
            files,
        )?);
        vb_state
            .set_stack(vbranch.clone())
            .context(format!("failed to write virtual branch {}", vbranch.name))?;
    }

    let hunks_by_branch: Vec<(Stack, HashMap<PathBuf, Vec<VirtualBranchHunk>>)> = hunks_by_branch
        .iter()
        .map(|(branch, hunks)| {
            let hunks =
                file_hunks_from_diffs(&ctx.project().path, hunks.clone(), Some(diff_dependencies));
            (branch.clone(), hunks)
        })
        .collect();

    let files_by_branch: Vec<(Stack, Vec<VirtualBranchFile>)> = hunks_by_branch
        .iter()
        .map(|(branch, hunks)| {
            let files = virtual_hunks_into_virtual_files(hunks.clone());
            (branch.clone(), files)
        })
        .collect();

    Ok(VirtualBranchesStatus {
        branches: files_by_branch,
        skipped_files,
        workspace_dependencies,
    })
}
