use crate::{
    commit::VirtualBranchCommit,
    dependencies::stack_dependencies_from_workspace,
    file::VirtualBranchFile,
    hunk::VirtualBranchHunk,
    integration::get_workspace_head,
    remote::branch_to_remote_branch,
    stack::stack_series,
    status::{get_applied_status, get_applied_status_cached},
    RemoteBranchData, VirtualBranchHunkRange, VirtualBranchHunkRangeMap, VirtualBranchesExt,
};
use anyhow::{anyhow, bail, Context, Result};
use bstr::{BString, ByteSlice};
use but_rebase::RebaseStep;
use but_workspace::stack_ext::StackExt;
use gitbutler_branch::BranchUpdateRequest;
use gitbutler_branch::{dedup, dedup_fmt};
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_diff::{trees, GitHunk, Hunk};
use gitbutler_hunk_dependency::RangeCalculationError;
use gitbutler_operating_modes::assure_open_workspace_mode;
use gitbutler_oxidize::{
    git2_signature_to_gix_signature, git2_to_gix_object_id, gix_to_git2_oid, GixRepositoryExt,
    ObjectIdExt, OidExt,
};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname};
use gitbutler_repo::{
    logging::{LogUntil, RepositoryExt as _},
    RepositoryExt,
};
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{
    reconcile_claims, BranchOwnershipClaims, Stack, StackId, Target, VirtualBranchesHandle,
};
use gitbutler_time::time::now_since_unix_epoch_ms;
use itertools::Itertools;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, vec};
use tracing::instrument;

// this struct is a mapping to the view `Branch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of the Branch struct in Rust
// which is our persisted data structure for virtual branches
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct VirtualBranch {
    pub id: StackId,
    pub name: String,
    pub notes: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
    pub requires_force: bool, // does this branch require a force push to the upstream?
    pub conflicted: bool, // is this branch currently in a conflicted state (only for the workspace)
    pub order: usize,     // the order in which this branch should be displayed in the UI
    pub upstream: Option<RemoteBranchData>, // the upstream branch where this branch pushes to, if any
    pub upstream_name: Option<String>, // the upstream branch where this branch will push to on next push
    pub base_current: bool, // is this vbranch based on the current base branch? if false, this needs to be manually merged with conflicts
    /// The hunks (as `[(file, [hunks])]`) which are uncommitted but assigned to this branch.
    /// This makes them committable.
    pub ownership: BranchOwnershipClaims,
    pub updated_at: u128,
    pub selected_for_changes: bool,
    pub allow_rebasing: bool,
    #[serde(with = "gitbutler_serde::oid")]
    pub head: git2::Oid,
    /// The merge base between the target branch and the virtual branch
    #[serde(with = "gitbutler_serde::oid")]
    pub merge_base: git2::Oid,
    /// The fork point between the target branch and the virtual branch
    #[serde(with = "gitbutler_serde::oid_opt", default)]
    pub fork_point: Option<git2::Oid>,
    pub refname: Refname,
    #[serde(with = "gitbutler_serde::oid")]
    pub tree: git2::Oid,
    /// New way to group commits into a multiple patch series
    /// Most recent entries are first in order
    pub series: Vec<Result<PatchSeries, serde_error::Error>>,
}

/// A grouping that combines multiple commits into a patch series
///
/// We deviate slightly from established language as we are transitioning from lanes representing
/// independent branches to representing independent stacks of dependent patch series (branches).
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSeries {
    pub name: String,
    pub description: Option<String>,
    pub upstream_reference: Option<String>,
    /// List of patches beloning to this series, from newest to oldest
    pub patches: Vec<VirtualBranchCommit>,
    /// List of patches that only exist on the upstream branch
    pub upstream_patches: Vec<VirtualBranchCommit>,
    /// The pull request associated with the branch, or None if a pull request has not been created.
    pub pr_number: Option<usize>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    pub archived: bool,
    pub review_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranches {
    pub branches: Vec<VirtualBranch>,
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
    pub dependency_errors: Vec<RangeCalculationError>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub remote: String,
    pub refname: Refname,
}

struct HunkToUnapply<'a> {
    file_path: PathBuf,
    hunk: GitHunk,
    hunk_lines: Option<&'a Vec<VirtualBranchHunkRange>>,
}

pub fn unapply_ownership(
    ctx: &CommandContext,
    ownership: &BranchOwnershipClaims,
    lines: Option<VirtualBranchHunkRangeMap>,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let vb_state = ctx.project().virtual_branches();

    let workspace_commit_id = get_workspace_head(ctx)?;

    let applied_statuses = get_applied_status(ctx, None)
        .context("failed to get status by branch")?
        .branches;

    let hunks_to_unapply = applied_statuses
        .iter()
        .map(|(_branch, branch_files)| -> Result<Vec<HunkToUnapply>> {
            let mut hunks_to_unapply: Vec<HunkToUnapply> = Vec::new();
            for file in branch_files {
                let ownership_hunks: Vec<&Hunk> = ownership
                    .claims
                    .iter()
                    .filter(|o| o.file_path == file.path)
                    .flat_map(|f| &f.hunks)
                    .collect();
                for hunk in &file.hunks {
                    let hunk_lines = lines.as_ref().and_then(|lines| lines.get(&hunk.id));
                    let hunk: GitHunk = hunk.clone().into();
                    if ownership_hunks.contains(&&Hunk::from(&hunk)) {
                        hunks_to_unapply.push(HunkToUnapply {
                            file_path: file.path.clone(),
                            hunk,
                            hunk_lines,
                        });
                    }
                }
            }

            hunks_to_unapply.sort_by(|a, b| a.hunk.old_start.cmp(&b.hunk.old_start));

            Ok(hunks_to_unapply)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let mut diff = HashMap::new();
    for h in hunks_to_unapply {
        let reversed_hunk = if let Some(hunk_lines) = h.hunk_lines {
            let hunk_lines = hunk_lines
                .iter()
                .map(|l| (l.old, l.new))
                .collect::<Vec<(Option<u32>, Option<u32>)>>();
            gitbutler_diff::reverse_hunk_lines(&h.hunk, hunk_lines)
        } else {
            gitbutler_diff::reverse_hunk(&h.hunk)
        };

        if let Some(reversed_hunk) = reversed_hunk {
            diff.entry(h.file_path)
                .or_insert_with(Vec::new)
                .push(reversed_hunk);
        } else {
            bail!("failed to reverse hunk")
        }
    }

    let repo = ctx.repo();

    let target_commit = repo
        .find_commit(workspace_commit_id)
        .context("failed to find target commit")?;

    let base_tree_id = git2_to_gix_object_id(target_commit.tree_id());
    let gix_repo = ctx.gix_repo_for_merging()?;
    let (merge_options_fail_fast, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    let final_tree_id = applied_statuses.into_iter().try_fold(
        git2_to_gix_object_id(target_commit.tree_id()),
        |final_tree_id, status| -> Result<_> {
            let files = status
                .1
                .into_iter()
                .map(|file| (file.path, file.hunks))
                .collect::<Vec<(PathBuf, Vec<VirtualBranchHunk>)>>();
            let branch_tree_id =
                gitbutler_diff::write::hunks_onto_oid(ctx, workspace_commit_id, files)?;
            let mut merge = gix_repo.merge_trees(
                base_tree_id,
                final_tree_id,
                git2_to_gix_object_id(branch_tree_id),
                gix_repo.default_merge_labels(),
                merge_options_fail_fast.clone(),
            )?;
            if merge.has_unresolved_conflicts(conflict_kind) {
                bail!("Tree has conflicts after merge")
            }
            Ok(merge.tree.write()?.detach())
        },
    )?;

    let final_tree = repo.find_tree(gix_to_git2_oid(final_tree_id))?;
    let final_tree_oid = gitbutler_diff::write::hunks_onto_tree(ctx, &final_tree, diff, true)?;
    let final_tree = repo
        .find_tree(final_tree_oid)
        .context("failed to find tree")?;

    repo.checkout_tree_builder(&final_tree)
        .force()
        .remove_untracked()
        .checkout()
        .context("failed to checkout tree")?;

    crate::integration::update_workspace_commit(&vb_state, ctx)?;

    Ok(())
}

// reset a file in the project to the index state
pub(crate) fn reset_files(
    ctx: &CommandContext,
    stack_id: StackId,
    files: &[PathBuf],
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let stack = ctx
        .project()
        .virtual_branches()
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?
        .into_iter()
        .find(|b| b.id == stack_id)
        .with_context(|| {
            format!("could not find applied branch with id {stack_id} to reset files from")
        })?;
    let claims: Vec<_> = stack
        .ownership
        .claims
        .into_iter()
        .filter(|claim| files.contains(&claim.file_path))
        .collect();

    unapply_ownership(ctx, &BranchOwnershipClaims { claims }, None, perm)?;
    Ok(())
}
fn find_base_tree<'a>(
    repo: &'a git2::Repository,
    branch_commit: &'a git2::Commit<'a>,
    target_commit: &'a git2::Commit<'a>,
) -> Result<git2::Tree<'a>> {
    // find merge base between target_commit and branch_commit
    let merge_base = repo
        .merge_base(target_commit.id(), branch_commit.id())
        .context("failed to find merge base")?;
    // turn oid into a commit
    let merge_base_commit = repo
        .find_commit(merge_base)
        .context("failed to find merge base commit")?;
    let base_tree = merge_base_commit
        .tree()
        .context("failed to get base tree object")?;
    Ok(base_tree)
}

#[derive(Debug)]
pub struct StackListResult {
    pub branches: Vec<VirtualBranch>,
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
    pub dependency_errors: Vec<RangeCalculationError>,
}

pub fn list_virtual_branches(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<StackListResult> {
    let diffs = gitbutler_diff::workdir(ctx.repo(), get_workspace_head(ctx)?)?;
    list_virtual_branches_cached(ctx, perm, &diffs)
}

/// `worktree_changes` are all changed files against the current `HEAD^{tree}` and index
/// against the current working tree directory, and it's used to avoid double-computing
/// this expensive information.
#[instrument(level = tracing::Level::DEBUG, skip(ctx, perm, worktree_changes))]
pub fn list_virtual_branches_cached(
    ctx: &CommandContext,
    // TODO(ST): this should really only shared access, but there is some internals
    //           that conditionally write things.
    perm: &mut WorktreeWritePermission,
    worktree_changes: &gitbutler_diff::DiffByPathMap,
) -> Result<StackListResult> {
    assure_open_workspace_mode(ctx)
        .context("Listing virtual branches requires open workspace mode")?;
    let mut branches: Vec<VirtualBranch> = Vec::new();

    let vb_state = ctx.project().virtual_branches();

    let default_target = vb_state
        .get_default_target()
        .context("failed to get default target")?;

    let status = get_applied_status_cached(ctx, Some(perm), worktree_changes)?;
    let max_selected_for_changes = status
        .branches
        .iter()
        .filter_map(|(branch, _)| branch.selected_for_changes)
        .max()
        .unwrap_or(-1);

    let branches_span =
        tracing::debug_span!("handle branches", num_branches = status.branches.len()).entered();
    let repo = ctx.repo();
    let gix_repo = ctx.gix_repo_for_merging_non_persisting()?;
    // We will perform virtual merges, no need to write them to the ODB.
    let cache = gix_repo.commit_graph_if_enabled()?;
    let mut graph = gix_repo.revision_graph(cache.as_ref());
    for (mut branch, mut files) in status.branches {
        let upstream_branch = match &branch.upstream {
            Some(upstream) => repo.maybe_find_branch_by_refname(&Refname::from(upstream))?,
            None => None,
        };

        // find all commits on head that are not on target.sha
        let commits = repo.log(
            branch.head(&gix_repo)?.to_git2(),
            LogUntil::Commit(default_target.sha),
            false,
        )?;
        let mut check_commit =
            IsCommitIntegrated::new(ctx, &default_target, &gix_repo, &mut graph)?;

        let merge_base = gix_repo
            .merge_base_with_graph(
                default_target.sha.to_gix(),
                branch.head(&gix_repo)?,
                check_commit.graph,
            )
            .context("failed to find merge base")?;
        let merge_base = gix_to_git2_oid(merge_base);
        let base_current = true;

        let raw_remotes = repo.remotes()?;
        let remotes: Vec<_> = raw_remotes.into_iter().flatten().collect();
        let upstream = upstream_branch
            .map(|upstream_branch| branch_to_remote_branch(ctx, &upstream_branch, &remotes))
            .transpose()?;

        let path_claim_positions: HashMap<&PathBuf, usize> = branch
            .ownership
            .claims
            .iter()
            .enumerate()
            .map(|(index, ownership_claim)| (&ownership_claim.file_path, index))
            .collect();

        files.sort_by(|a, b| {
            path_claim_positions
                .get(&a.path)
                .unwrap_or(&usize::MAX)
                .cmp(path_claim_positions.get(&b.path).unwrap_or(&usize::MAX))
        });
        let mut requires_force = is_requires_force(ctx, &branch, &gix_repo)?;

        let fork_point = commits
            .last()
            .and_then(|c| c.parent(0).ok())
            .map(|c| c.id());

        let refname = branch.refname()?.into();

        let stack_dependencies =
            stack_dependencies_from_workspace(&status.workspace_dependencies, branch.id);

        // TODO: Error out here once this API is stable
        let (series, force) = stack_series(
            ctx,
            &mut branch,
            &default_target,
            &mut check_commit,
            stack_dependencies,
        );

        if series
            .iter()
            .cloned()
            .filter_map(Result::ok)
            .any(|s| s.upstream_reference.is_some())
        {
            requires_force = force // derive force requirement from the series
        }

        let head = branch.head(&gix_repo)?;
        let tree = branch.tree(ctx)?;
        let branch = VirtualBranch {
            id: branch.id,
            name: branch.name,
            notes: branch.notes,
            active: true,
            files,
            order: branch.order,
            requires_force,
            upstream,
            upstream_name: branch
                .upstream
                .and_then(|r| Refname::from(r).branch().map(Into::into)),
            conflicted: false, // TODO: Get this from the index
            base_current,
            ownership: branch.ownership,
            updated_at: branch.updated_timestamp_ms,
            selected_for_changes: branch.selected_for_changes == Some(max_selected_for_changes),
            allow_rebasing: branch.allow_rebasing,
            head: head.to_git2(),
            merge_base,
            fork_point,
            refname,
            tree,
            series,
        };
        branches.push(branch);
    }
    drop(branches_span);

    let mut branches = branches_with_large_files_abridged(branches);
    branches.sort_by(|a, b| a.order.cmp(&b.order));

    Ok(StackListResult {
        branches,
        skipped_files: status.skipped_files,
        dependency_errors: status.workspace_dependencies.errors,
    })
}

/// The commit-data we can use for comparison to see which remote-commit was used to craete
/// a local commit from.
/// Note that trees can't be used for comparison as these are typically rebased.
#[derive(Debug, Hash, Eq, PartialEq)]
pub(crate) struct CommitData {
    message: BString,
    author: gix::actor::Signature,
}

impl TryFrom<&git2::Commit<'_>> for CommitData {
    type Error = anyhow::Error;

    fn try_from(commit: &git2::Commit<'_>) -> std::result::Result<Self, Self::Error> {
        Ok(CommitData {
            message: commit.message_raw_bytes().into(),
            author: git2_signature_to_gix_signature(commit.author()),
        })
    }
}

fn branches_with_large_files_abridged(mut branches: Vec<VirtualBranch>) -> Vec<VirtualBranch> {
    for branch in &mut branches {
        for file in &mut branch.files {
            // Diffs larger than 500kb are considered large
            if file.hunks.iter().any(|hunk| hunk.diff.len() > 500_000) {
                file.large = true;
                file.hunks.iter_mut().for_each(|hunk| {
                    hunk.diff.drain(..);
                });
            }
        }
    }
    branches
}

fn is_requires_force(ctx: &CommandContext, stack: &Stack, repo: &gix::Repository) -> Result<bool> {
    let upstream = if let Some(upstream) = &stack.upstream {
        upstream
    } else {
        return Ok(false);
    };

    let reference = match ctx.repo().refname_to_id(&upstream.to_string()) {
        Ok(reference) => reference,
        Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(false),
        Err(other) => return Err(other).context("failed to find upstream reference"),
    };

    let upstream_commit = ctx
        .repo()
        .find_commit(reference)
        .context("failed to find upstream commit")?;

    let merge_base = ctx
        .repo()
        .merge_base(upstream_commit.id(), stack.head(repo)?.to_git2())?;

    Ok(merge_base != upstream_commit.id())
}

pub fn update_branch(ctx: &CommandContext, branch_update: &BranchUpdateRequest) -> Result<Stack> {
    let vb_state = ctx.project().virtual_branches();
    let mut stack = vb_state.get_stack_in_workspace(branch_update.id)?;

    if let Some(ownership) = &branch_update.ownership {
        set_ownership(&vb_state, &mut stack, ownership).context("failed to set ownership")?;
    }

    if let Some(name) = &branch_update.name {
        let all_virtual_branches = vb_state
            .list_stacks_in_workspace()
            .context("failed to read virtual branches")?;

        ctx.delete_branch_reference(&stack)?;

        stack.name = dedup(
            &all_virtual_branches
                .iter()
                .filter(|b| b.id != branch_update.id)
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            name,
        );

        ctx.add_branch_reference(&stack)?;
    };

    if let Some(updated_upstream) = &branch_update.upstream {
        let default_target = vb_state.get_default_target()?;
        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(updated_upstream)?
        )
        .parse::<RemoteRefname>()
        .unwrap();
        stack.upstream = Some(remote_branch);
    };

    if let Some(notes) = branch_update.notes.clone() {
        stack.notes = notes;
    };

    if let Some(order) = branch_update.order {
        stack.order = order;
    };

    if let Some(selected_for_changes) = branch_update.selected_for_changes {
        stack.selected_for_changes = if selected_for_changes {
            for mut other_branch in vb_state
                .list_stacks_in_workspace()
                .context("failed to read virtual branches")?
                .into_iter()
                .filter(|b| b.id != stack.id)
            {
                other_branch.selected_for_changes = None;
                vb_state.set_stack(other_branch.clone())?;
            }
            Some(now_since_unix_epoch_ms())
        } else {
            None
        };
    };

    if let Some(allow_rebasing) = branch_update.allow_rebasing {
        stack.allow_rebasing = allow_rebasing;
    };

    vb_state.set_stack(stack.clone())?;
    Ok(stack)
}

pub(crate) fn ensure_selected_for_changes(vb_state: &VirtualBranchesHandle) -> Result<()> {
    let mut stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to list branches")?;

    if stacks.is_empty() {
        println!("no applied branches");
        return Ok(());
    }

    if stacks.iter().any(|b| b.selected_for_changes.is_some()) {
        println!("some branches already selected for changes");
        return Ok(());
    }

    stacks.sort_by_key(|branch| branch.order);

    stacks[0].selected_for_changes = Some(now_since_unix_epoch_ms());
    vb_state.set_stack(stacks[0].clone())?;
    Ok(())
}

pub(crate) fn set_ownership(
    vb_state: &VirtualBranchesHandle,
    target_branch: &mut Stack,
    ownership: &BranchOwnershipClaims,
) -> Result<()> {
    if target_branch.ownership.eq(ownership) {
        // nothing to update
        return Ok(());
    }

    let stacks = vb_state
        .list_stacks_in_workspace()
        .context("failed to read virtual branches")?;

    let mut claim_outcomes = reconcile_claims(stacks, target_branch, &ownership.claims)?;
    for claim_outcome in &mut claim_outcomes {
        if !claim_outcome.removed_claims.is_empty() {
            vb_state
                .set_stack(claim_outcome.updated_branch.clone())
                .context("failed to write ownership for branch".to_string())?;
        }
    }

    // Updates the claiming branch that was passed as mutable state with the new ownership claims
    // TODO: remove mutable reference to target_branch
    target_branch.ownership = ownership.clone();

    Ok(())
}

pub type BranchStatus = HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>;
pub type VirtualBranchHunksByPathMap = HashMap<PathBuf, Vec<VirtualBranchHunk>>;

// reset virtual branch to a specific commit
pub(crate) fn reset_branch(
    ctx: &CommandContext,
    stack_id: StackId,
    target_commit_id: git2::Oid,
) -> Result<()> {
    let vb_state = ctx.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;

    let gix_repo = ctx.gix_repo()?;
    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    if stack.head(&gix_repo)? == target_commit_id.to_gix() {
        // nothing to do
        return Ok(());
    }

    if default_target.sha != target_commit_id
        && !ctx
            .repo()
            .l(
                stack.head(&gix_repo)?.to_git2(),
                LogUntil::Commit(default_target.sha),
                false,
            )?
            .contains(&target_commit_id)
    {
        bail!("commit {target_commit_id} not in the branch");
    }

    // Compute the old workspace before resetting, so we can figure out
    // what hunks were released by this reset, and assign them to this branch.
    let old_head = get_workspace_head(ctx)?;

    stack.set_stack_head(&vb_state, &gix_repo, target_commit_id, None)?;

    let updated_head = get_workspace_head(ctx)?;
    let repo = ctx.repo();
    let diff = trees(
        repo,
        &repo
            .find_commit(updated_head)?
            .tree()
            .map_err(anyhow::Error::from)?,
        &repo
            .find_commit(old_head)?
            .tree()
            .map_err(anyhow::Error::from)?,
        true,
    )?;

    // Assign the new hunks to the branch we're working on.
    for (path, filediff) in diff {
        for hunk in filediff.hunks {
            let hash = Hunk::hash_diff(&hunk.diff_lines);
            stack.ownership.put(
                format!(
                    "{}:{}-{}-{:?}",
                    path.display(),
                    hunk.new_start,
                    hunk.new_start + hunk.new_lines,
                    &hash
                )
                .parse()?,
            );
        }
    }
    vb_state
        .set_stack(stack)
        .context("failed to write branch")?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit(
    ctx: &CommandContext,
    stack_id: StackId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
) -> Result<git2::Oid> {
    // get the files to commit
    let diffs = gitbutler_diff::workdir(ctx.repo(), get_workspace_head(ctx)?)?;
    let statuses = get_applied_status_cached(ctx, None, &diffs)
        .context("failed to get status by branch")?
        .branches;

    let (ref mut branch, files) = statuses
        .into_iter()
        .find(|(stack, _)| stack.id == stack_id)
        .with_context(|| format!("stack {stack_id} not found"))?;

    let gix_repo = ctx.gix_repo()?;

    let tree_oid = if let Some(ownership) = ownership {
        let files = files.into_iter().filter_map(|file| {
            let hunks = file
                .hunks
                .into_iter()
                .filter(|hunk| {
                    let hunk: GitHunk = hunk.clone().into();
                    ownership
                        .claims
                        .iter()
                        .find(|f| f.file_path.eq(&file.path))
                        .is_some_and(|f| {
                            f.hunks.iter().any(|h| {
                                h.start == hunk.new_start
                                    && h.end == hunk.new_start + hunk.new_lines
                            })
                        })
                })
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                None
            } else {
                Some((file.path, hunks))
            }
        });
        gitbutler_diff::write::hunks_onto_commit(ctx, branch.head(&gix_repo)?.to_git2(), files)?
    } else {
        let files = files
            .into_iter()
            .map(|file| (file.path, file.hunks))
            .collect::<Vec<(PathBuf, Vec<VirtualBranchHunk>)>>();
        gitbutler_diff::write::hunks_onto_commit(ctx, branch.head(&gix_repo)?.to_git2(), files)?
    };

    let git_repo = ctx.repo();
    let parent_commit = git_repo
        .find_commit(branch.head(&gix_repo)?.to_git2())
        .context(format!(
            "failed to find commit {:?}",
            branch.head(&gix_repo)
        ))?;
    let tree = git_repo
        .find_tree(tree_oid)
        .context(format!("failed to find tree {:?}", tree_oid))?;

    let commit_oid = ctx.commit(message, &tree, &[&parent_commit], None)?;

    let vb_state = ctx.project().virtual_branches();
    branch.set_stack_head(&vb_state, &gix_repo, commit_oid, Some(tree_oid))?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(commit_oid)
}

pub(crate) fn push(
    ctx: &CommandContext,
    stack_id: StackId,
    with_force: bool,
    askpass: Option<Option<StackId>>,
) -> Result<PushResult> {
    let vb_state = ctx.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;
    let upstream_remote = match default_target.push_remote_name {
        Some(remote) => remote.clone(),
        None => default_target.branch.remote().to_owned(),
    };

    let gix_repo = ctx.gix_repo()?;
    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let remote_branch = if let Some(upstream_branch) = &stack.upstream {
        upstream_branch.clone()
    } else {
        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(&stack.name)?
        )
        .parse::<RemoteRefname>()
        .context("failed to parse remote branch name")?;

        let remote_branches = ctx.repo().remote_branches()?;
        let existing_branches = remote_branches
            .iter()
            .map(RemoteRefname::branch)
            .map(str::to_lowercase) // git is weird about case sensitivity here, assume not case sensitive
            .collect::<Vec<_>>();

        remote_branch.with_branch(&dedup_fmt(
            &existing_branches
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            remote_branch.branch(),
            "-",
        ))
    };

    ctx.push(
        stack.head(&gix_repo)?.to_git2(),
        &remote_branch,
        with_force,
        None,
        askpass,
    )?;

    stack.upstream = Some(remote_branch.clone());
    stack.upstream_head = Some(stack.head(&gix_repo)?.to_git2());
    vb_state
        .set_stack(stack.clone())
        .context("failed to write target branch after push")?;
    ctx.fetch(remote_branch.remote(), askpass.map(|_| "modal".to_string()))?;

    Ok(PushResult {
        remote: upstream_remote,
        refname: gitbutler_reference::Refname::Remote(remote_branch),
    })
}

type MergeBaseCommitGraph<'repo, 'cache> = gix::revwalk::Graph<
    'repo,
    'cache,
    gix::revision::plumbing::graph::Commit<gix::revision::plumbing::merge_base::Flags>,
>;

pub(crate) struct IsCommitIntegrated<'repo, 'cache, 'graph> {
    gix_repo: &'repo gix::Repository,
    graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    target_commit_id: gix::ObjectId,
    upstream_tree_id: gix::ObjectId,
    upstream_commits: Vec<git2::Oid>,
    upstream_change_ids: Vec<String>,
}

impl<'repo, 'cache, 'graph> IsCommitIntegrated<'repo, 'cache, 'graph> {
    pub(crate) fn new(
        ctx: &'repo CommandContext,
        target: &Target,
        gix_repo: &'repo gix::Repository,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
    ) -> anyhow::Result<Self> {
        let remote_branch = ctx
            .repo()
            .maybe_find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("failed to get branch"))?;
        let remote_head = remote_branch.get().peel_to_commit()?;
        let upstream_tree_id = ctx.repo().find_commit(remote_head.id())?.tree_id();

        let upstream_commits =
            ctx.repo()
                .log(remote_head.id(), LogUntil::Commit(target.sha), true)?;
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|commit| commit.change_id())
            .sorted()
            .collect();
        let upstream_commits = upstream_commits
            .iter()
            .map(|commit| commit.id())
            .sorted()
            .collect();
        Ok(Self {
            gix_repo,
            graph,
            target_commit_id: git2_to_gix_object_id(target.sha),
            upstream_tree_id: git2_to_gix_object_id(upstream_tree_id),
            upstream_commits,
            upstream_change_ids,
        })
    }

    /// Used to construct [`IsCommitIntegrated`] without a [`CommandContext`]. If
    /// you have a `CommandContext` available, use [`Self::new`] instead.
    pub(crate) fn new_basic(
        gix_repo: &'repo gix::Repository,
        repo: &'repo git2::Repository,
        graph: &'graph mut MergeBaseCommitGraph<'repo, 'cache>,
        target_commit_id: gix::ObjectId,
        upstream_tree_id: gix::ObjectId,
        mut upstream_commits: Vec<git2::Oid>,
    ) -> Self {
        // Ensure upstream commits are sorted for binary search
        upstream_commits.sort();
        let upstream_change_ids = upstream_commits
            .iter()
            .filter_map(|oid| {
                let commit = repo.find_commit(*oid).ok()?;
                commit.change_id()
            })
            .sorted()
            .collect();
        Self {
            gix_repo,
            graph,
            target_commit_id,
            upstream_tree_id,
            upstream_commits,
            upstream_change_ids,
        }
    }
}

impl IsCommitIntegrated<'_, '_, '_> {
    pub(crate) fn is_integrated(&mut self, commit: &git2::Commit) -> Result<bool> {
        if self.target_commit_id == git2_to_gix_object_id(commit.id()) {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        if let Some(change_id) = commit.change_id() {
            if self.upstream_change_ids.binary_search(&change_id).is_ok() {
                return Ok(true);
            }
        }

        if self.upstream_commits.binary_search(&commit.id()).is_ok() {
            return Ok(true);
        }

        let merge_base_id = self.gix_repo.merge_base_with_graph(
            self.target_commit_id,
            git2_to_gix_object_id(commit.id()),
            self.graph,
        )?;
        if gix_to_git2_oid(merge_base_id).eq(&commit.id()) {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base_tree_id = self.gix_repo.find_commit(merge_base_id)?.tree_id()?;
        if merge_base_tree_id == self.upstream_tree_id {
            // if merge base is the same as upstream tree, then it's integrated
            return Ok(true);
        }

        // try to merge our tree into the upstream tree
        let (merge_options, conflict_kind) = self.gix_repo.merge_options_no_rewrites_fail_fast()?;
        let mut merge_output = self
            .gix_repo
            .merge_trees(
                merge_base_tree_id,
                git2_to_gix_object_id(commit.tree_id()),
                self.upstream_tree_id,
                Default::default(),
                merge_options,
            )
            .context("failed to merge trees")?;

        if merge_output.has_unresolved_conflicts(conflict_kind) {
            return Ok(false);
        }

        let merge_tree_id = merge_output.tree.write()?.detach();

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_id == self.upstream_tree_id)
    }
}

pub fn is_remote_branch_mergeable(
    ctx: &CommandContext,
    branch_name: &RemoteRefname,
) -> Result<bool> {
    let vb_state = ctx.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;
    let target_commit = ctx
        .repo()
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let branch = ctx
        .repo()
        .maybe_find_branch_by_refname(&branch_name.into())?
        .ok_or(anyhow!("branch not found"))?;
    let branch_oid = branch.get().target().context("detatched head")?;
    let branch_commit = ctx
        .repo()
        .find_commit(branch_oid)
        .context("failed to find branch commit")?;

    let base_tree = find_base_tree(ctx.repo(), &branch_commit, &target_commit)?;

    let wd_tree = ctx.repo().create_wd_tree(AUTO_TRACK_LIMIT_BYTES)?;

    let branch_tree = branch_commit.tree().context("failed to find branch tree")?;
    let gix_repo_in_memory = ctx.gix_repo_for_merging()?.with_object_memory();
    let (merge_options_fail_fast, conflict_kind) =
        gix_repo_in_memory.merge_options_no_rewrites_fail_fast()?;
    let mergeable = !gix_repo_in_memory
        .merge_trees(
            git2_to_gix_object_id(base_tree.id()),
            git2_to_gix_object_id(branch_tree.id()),
            git2_to_gix_object_id(wd_tree.id()),
            Default::default(),
            merge_options_fail_fast,
        )
        .context("failed to merge trees")?
        .has_unresolved_conflicts(conflict_kind);

    Ok(mergeable)
}

// this function takes a list of file ownership from a "from" commit and "moves"
// those changes to a "to" commit in a branch. This allows users to drag changes
// from one commit to another.
// if the "to" commit is below the "from" commit, the changes are simply added to the "to" commit
// and the rebase should be simple. if the "to" commit is above the "from" commit,
// the changes need to be removed from the "from" commit, everything rebased,
// then added to the "to" commit and everything above that rebased again.
//
// NB: It appears that this function is semi-broken when the "to" commit is above the "from" commit.
// Ths changes are indeed removed from the "from" commit, but they end up in the workspace and not the "to" commit.
// This was broken before the migration to the rebase engine.
// The way the trees of "diffs to keep" and "diffs to amend" are computed with gitbutler_diff::write::hunks_onto_commit is incredibly sketchy
pub(crate) fn move_commit_file(
    ctx: &CommandContext,
    stack_id: StackId,
    from_commit_id: git2::Oid,
    to_commit_id: git2::Oid,
    target_ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    let vb_state = ctx.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let gix_repo = ctx.gix_repo()?;
    let merge_base = stack.merge_base(ctx)?;

    // first, let's get the from commit data and it's parent data
    let from_commit = ctx
        .repo()
        .find_commit(from_commit_id)
        .context("failed to find commit")?;
    let from_tree = from_commit.tree().context("failed to find tree")?;
    let from_parent = from_commit.parent(0).context("failed to find parent")?;
    let from_parent_tree = from_parent.tree().context("failed to find parent tree")?;

    // ok, what is the entire patch introduced in the "from" commit?
    // we need to remove the parts of this patch that are in target_ownership (the parts we're moving)
    // and then apply the rest to the parent tree of the "from" commit to
    // create the new "from" commit without the changes we're moving
    let from_commit_diffs = gitbutler_diff::trees(ctx.repo(), &from_parent_tree, &from_tree, true)
        .context("failed to diff trees")?;

    // filter from_commit_diffs to HashMap<filepath, Vec<GitHunk>> only for hunks NOT in target_ownership
    // this is the patch parts we're keeping
    let diffs_to_keep = from_commit_diffs
        .iter()
        .filter_map(|(filepath, file_diff)| {
            let hunks = file_diff
                .hunks
                .iter()
                .filter(|hunk| {
                    !target_ownership.claims.iter().any(|file_ownership| {
                        file_ownership.file_path.eq(filepath)
                            && file_ownership.hunks.iter().any(|owned_hunk| {
                                owned_hunk.start == hunk.new_start
                                    && owned_hunk.end == hunk.new_start + hunk.new_lines
                            })
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                None
            } else {
                Some((filepath.clone(), hunks))
            }
        })
        .collect::<HashMap<_, _>>();

    // write our new tree and commit for the new "from" commit without the moved changes
    let new_from_tree_id =
        gitbutler_diff::write::hunks_onto_commit(ctx, from_parent.id(), &diffs_to_keep)?;
    let new_from_tree = &ctx
        .repo()
        .find_tree(new_from_tree_id)
        .with_context(|| "tree {new_from_tree_oid} not found")?;
    let new_from_commit_oid = ctx
        .repo()
        .commit_with_signature(
            None,
            &from_commit.author(),
            &from_commit.committer(),
            &from_commit.message_bstr().to_str_lossy(),
            new_from_tree,
            &[&from_parent],
            from_commit.gitbutler_headers(),
        )
        .context("commit failed")?;

    // rebase swapping the from_commit_oid with the new_from_commit_oid
    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
    // replace the "from" commit in the rebase steps with the new "from" commit which has the moved changes removed
    for step in steps.iter_mut() {
        if let RebaseStep::Pick { commit_id, .. } = step {
            if *commit_id == from_commit_id.to_gix() {
                *commit_id = new_from_commit_oid.to_gix();
            }
        }
    }
    let mut rebase = but_rebase::Rebase::new(&gix_repo, merge_base, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let outcome = rebase.rebase()?;
    // ensure that the stack here has been updated.
    stack.set_heads_from_rebase_output(ctx, outcome.references)?;

    // Discover the new id of the commit to amend `to_commit_id` from the output of the first rebas
    let to_commit_id = outcome
        .commit_mapping
        .iter()
        .find(|(_base, old, _new)| old == &to_commit_id.to_gix())
        .map(|(_base, _old, new)| new.to_git2())
        .ok_or_else(|| anyhow!("failed to find the to_ammend_commit after the initial rebase"))?;

    let to_commit = ctx
        .repo()
        .find_commit(to_commit_id)
        .context("failed to find commit")?;
    let to_commit_parents: Vec<_> = to_commit.parents().collect();

    // get a list of all the diffs across all the virtual branches
    let base_file_diffs = gitbutler_diff::workdir(ctx.repo(), default_target.sha)
        .context("failed to diff workdir")?;

    // filter base_file_diffs to HashMap<filepath, Vec<GitHunk>> only for hunks in target_ownership
    // this is essentially the group of patches that we're "moving"
    let diffs_to_amend = target_ownership
        .claims
        .iter()
        .filter_map(|file_ownership| {
            let hunks = base_file_diffs
                .get(&file_ownership.file_path)
                .map(|hunks| {
                    hunks
                        .hunks
                        .iter()
                        .filter(|hunk| {
                            file_ownership.hunks.iter().any(|owned_hunk| {
                                owned_hunk.start == hunk.new_start
                                    && owned_hunk.end == hunk.new_start + hunk.new_lines
                            })
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if hunks.is_empty() {
                None
            } else {
                Some((file_ownership.file_path.clone(), hunks))
            }
        })
        .collect::<HashMap<_, _>>();
    // apply diffs_to_amend to the commit tree
    // and write a new commit with the changes we're moving
    // let new_tree_oid =
    //     gitbutler_diff::write::hunks_onto_commit(ctx, to_commit_id, &diffs_to_amend)?;
    let new_tree_oid =
        gitbutler_diff::write::hunks_onto_tree(ctx, &to_commit.tree()?, &diffs_to_amend, true)?;

    let new_tree = ctx
        .repo()
        .find_tree(new_tree_oid)
        .context("failed to find new tree")?;
    let new_to_commit_oid = ctx
        .repo()
        .commit_with_signature(
            None,
            &to_commit.author(),
            &to_commit.committer(),
            &to_commit.message_bstr().to_str_lossy(),
            &new_tree,
            &to_commit_parents.iter().collect::<Vec<_>>(),
            to_commit.gitbutler_headers(),
        )
        .context("failed to create commit")?;

    dbg!(&new_to_commit_oid);

    // another rebase
    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
    // replace the "to" commit in the rebase steps with the new "to" commit which has the moved changes added
    for step in steps.iter_mut() {
        if let RebaseStep::Pick { commit_id, .. } = step {
            if *commit_id == to_commit_id.to_gix() {
                *commit_id = new_to_commit_oid.to_gix();
            }
        }
    }
    let mut rebase = but_rebase::Rebase::new(&gix_repo, merge_base, None)?;
    rebase.steps(steps)?;
    rebase.rebase_noops(false);
    let outcome = rebase.rebase()?;
    stack.set_heads_from_rebase_output(ctx, outcome.references)?;
    stack.set_stack_head(&vb_state, &gix_repo, outcome.top_commit.to_git2(), None)?;
    // todo: maybe update the workspace commit here?
    Ok(new_to_commit_oid)
}

// create and insert a blank commit (no tree change) either above or below a commit
// if offset is positive, insert below, if negative, insert above
// return the oid of the new head commit of the branch with the inserted blank commit
pub(crate) fn insert_blank_commit(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_oid: git2::Oid,
    offset: i32,
) -> Result<()> {
    let vb_state = ctx.project().virtual_branches();

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    // find the commit to offset from
    let mut commit = ctx
        .repo()
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    if offset > 0 {
        commit = commit.parent(0).context("failed to find parent")?;
    }

    let repo = ctx.repo();

    let commit_tree = repo.find_real_tree(&commit, Default::default()).unwrap();
    let blank_commit_oid = ctx.commit("", &commit_tree, &[&commit], Some(Default::default()))?;

    let merge_base = stack.merge_base(ctx)?;
    let repo = ctx.gix_repo()?;
    let steps = stack.as_rebase_steps(ctx, &repo)?;
    let mut updated_steps = vec![];
    for step in steps.iter() {
        updated_steps.push(step.clone());
        if let RebaseStep::Pick { commit_id, .. } = step {
            if commit_id == &commit.id().to_gix() {
                updated_steps.push(RebaseStep::Pick {
                    commit_id: blank_commit_oid.to_gix(),
                    new_message: None,
                });
            }
        }
    }
    // if the  commit is the merge_base, then put the blank commit at the beginning
    if commit.id().to_gix() == merge_base {
        updated_steps.insert(
            0,
            RebaseStep::Pick {
                commit_id: blank_commit_oid.to_gix(),
                new_message: None,
            },
        );
    }

    let mut rebase = but_rebase::Rebase::new(&repo, merge_base, None)?;
    rebase.steps(updated_steps)?;
    rebase.rebase_noops(false);
    let output = rebase.rebase()?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    stack.set_stack_head(&vb_state, &repo, output.top_commit.to_git2(), None)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    Ok(())
}

// changes a commit message for commit_oid, rebases everything above it, updates branch head if successful
pub(crate) fn update_commit_message(
    ctx: &CommandContext,
    stack_id: StackId,
    commit_id: git2::Oid,
    message: &str,
) -> Result<git2::Oid> {
    if message.is_empty() {
        bail!("commit message can not be empty");
    }
    let vb_state = ctx.project().virtual_branches();
    let default_target = vb_state.get_default_target()?;
    let gix_repo = ctx.gix_repo()?;

    let mut stack = vb_state.get_stack_in_workspace(stack_id)?;
    let branch_commit_oids = ctx.repo().l(
        stack.head(&gix_repo)?.to_git2(),
        LogUntil::Commit(default_target.sha),
        false,
    )?;

    if !branch_commit_oids.contains(&commit_id) {
        bail!("commit {commit_id} not in the branch");
    }

    let pushed_commit_oids = stack.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| {
            ctx.repo()
                .l(upstream_head, LogUntil::Commit(default_target.sha), false)
        },
    )?;

    if pushed_commit_oids.contains(&commit_id) && !stack.allow_rebasing {
        // updating the message of a pushed commit will cause a force push that is not allowed
        bail!("force push not allowed");
    }

    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
    // Update the commit message
    for step in steps.iter_mut() {
        if let RebaseStep::Pick {
            commit_id: id,
            new_message,
        } = step
        {
            if *id == commit_id.to_gix() {
                *new_message = Some(message.into());
            }
        }
    }
    let merge_base = stack.merge_base(ctx)?;
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;

    let new_head = output.top_commit.to_git2();
    stack.set_stack_head(&vb_state, &gix_repo, new_head, None)?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    crate::integration::update_workspace_commit(&vb_state, ctx)
        .context("failed to update gitbutler workspace")?;

    output
        .commit_mapping
        .iter()
        .find_map(|(_base, old, new)| (*old == commit_id.to_gix()).then_some(new.to_git2()))
        .ok_or(anyhow!(
            "Failed to find the updated commit id after rebasing"
        ))
}
