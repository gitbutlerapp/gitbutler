use std::collections::HashSet;
use std::{collections::HashMap, path::PathBuf, vec};

use crate::file::list_virtual_commit_files;
use crate::integration::get_workspace_head;
use crate::BranchStatus;
use crate::{
    conflicts::RepoConflictsExt,
    file::{virtual_hunks_into_virtual_files, VirtualBranchFile},
    hunk::{file_hunks_from_diffs, VirtualBranchHunk},
    BranchManagerExt, VirtualBranchesExt,
};
use anyhow::{bail, Context, Result};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_diff::{diff_files_into_hunks, Hunk, HunkHash};
use gitbutler_hunk_dependency::{
    compute_hunk_locks, HunkDependencyOptions, HunkLock, InputCommit, InputDiff, InputFile,
    InputStack,
};
use gitbutler_operating_modes::assure_open_workspace_mode;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use gitbutler_stack::{BranchOwnershipClaims, OwnershipClaim, Stack, StackId};
use itertools::Itertools;
use tracing::instrument;

/// Represents the uncommitted status of the applied virtual branches in the workspace.
#[derive(Debug)]
pub struct VirtualBranchesStatus {
    /// A collection of branches and their associated uncommitted file changes.
    pub branches: Vec<(Stack, Vec<VirtualBranchFile>)>,
    /// A collection of files that were skipped during the diffing process (due to being very large and unprocessable).
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
}

pub fn get_applied_status(
    ctx: &CommandContext,
    perm: Option<&mut WorktreeWritePermission>,
) -> Result<VirtualBranchesStatus> {
    get_applied_status_cached(ctx, perm, None)
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
    worktree_changes: Option<gitbutler_diff::DiffByPathMap>,
) -> Result<VirtualBranchesStatus> {
    assure_open_workspace_mode(ctx).context("ng applied status requires open workspace mode")?;
    let workspace_head = get_workspace_head(ctx)?;
    let mut virtual_branches = ctx
        .project()
        .virtual_branches()
        .list_branches_in_workspace()?;
    let base_file_diffs = worktree_changes.map(Ok).unwrap_or_else(|| {
        gitbutler_diff::workdir(ctx.repository(), workspace_head.to_owned())
            .context("failed to diff workdir")
    })?;

    let mut skipped_files: Vec<gitbutler_diff::FileDiff> = Vec::new();
    for file_diff in base_file_diffs.values() {
        if file_diff.skipped {
            skipped_files.push(file_diff.clone());
        }
    }
    let mut base_diffs: HashMap<_, _> = diff_files_into_hunks(base_file_diffs).collect();

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

    let locks = compute_locks(
        ctx,
        &workspace_head,
        &default_target.sha,
        &base_diffs,
        &virtual_branches,
    )?;

    for branch in &mut virtual_branches {
        if let Err(e) = branch.initialize(ctx) {
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
    if !ctx.is_resolving() {
        for (vbranch, files) in &mut hunks_by_branch {
            vbranch.tree = gitbutler_diff::write::hunks_onto_oid(ctx, vbranch.head(), files)?;
            vb_state
                .set_branch(vbranch.clone())
                .context(format!("failed to write virtual branch {}", vbranch.name))?;
        }
    }
    let hunks_by_branch: Vec<(Stack, HashMap<PathBuf, Vec<VirtualBranchHunk>>)> = hunks_by_branch
        .iter()
        .map(|(branch, hunks)| {
            let hunks = file_hunks_from_diffs(&ctx.project().path, hunks.clone(), Some(&locks));
            (branch.clone(), hunks)
        })
        .collect();

    let files_by_branch: Vec<(Stack, Vec<VirtualBranchFile>)> = hunks_by_branch
        .iter()
        .map(|(branch, hunks)| {
            let files = virtual_hunks_into_virtual_files(ctx, hunks.clone());
            (branch.clone(), files)
        })
        .collect();

    Ok(VirtualBranchesStatus {
        branches: files_by_branch,
        skipped_files,
    })
}

fn compute_locks(
    ctx: &CommandContext,
    workspace_head: &git2::Oid,
    target_sha: &git2::Oid,
    base_diffs: &BranchStatus,
    stacks: &Vec<Stack>,
) -> Result<HashMap<HunkHash, Vec<HunkLock>>> {
    let repo = ctx.repository();
    let base_commit = repo.find_commit(*target_sha)?;
    let workspace_commit = repo.find_commit(*workspace_head)?;

    let diff = &ctx.repository().diff_tree_to_tree(
        Some(&base_commit.tree()?),
        Some(&workspace_commit.tree()?),
        None,
    )?;

    let files_touched_by_commits = diff
        .deltas()
        .filter_map(|d| d.new_file().path())
        .map(|c| c.to_path_buf())
        .unique()
        .sorted()
        .collect::<HashSet<_>>();
    let files_touched_by_diffs = base_diffs.keys().cloned().collect::<HashSet<_>>();

    let touched_by_both = files_touched_by_commits
        .intersection(&files_touched_by_diffs)
        .cloned()
        .collect_vec();

    let mut stacks_input: Vec<InputStack> = vec![];
    for stack in stacks {
        let mut commits_input: Vec<InputCommit> = vec![];
        let commit_ids = repo.l(stack.head(), LogUntil::Commit(*target_sha), false)?;
        for commit_id in commit_ids {
            let mut files_input: Vec<InputFile> = vec![];
            let commit = repo.find_commit(commit_id)?;
            let files = list_virtual_commit_files(ctx, &commit, false)?;
            for file in files {
                if touched_by_both.contains(&file.path) {
                    let value = InputFile {
                        path: file.path,
                        diffs: file
                            .hunks
                            .iter()
                            .map(|hunk| InputDiff {
                                old_start: hunk.old_start,
                                old_lines: hunk.old_lines,
                                new_start: hunk.start,
                                new_lines: hunk.end - hunk.start,
                            })
                            .collect::<Vec<_>>(),
                    };
                    files_input.push(value);
                }
            }
            commits_input.push(InputCommit {
                commit_id,
                files: files_input,
            });
        }
        stacks_input.push(InputStack {
            stack_id: stack.id,
            commits: commits_input,
        });
    }

    compute_hunk_locks(HunkDependencyOptions {
        workdir: base_diffs,
        stacks: stacks_input,
    })
}
