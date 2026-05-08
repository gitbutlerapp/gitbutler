//! Compute upstream integration statuses using `but-graph` projections.
//!
//! This replaces the legacy path through `gitbutler-branch-actions` which
//! used `stack_details_v3` and the old `UpstreamIntegrationContext`.

use std::collections::HashMap;

use super::upstream_integration::{
    BranchStatus, NameAndStatus, StackSelector, StackStatus, StackStatuses, UpstreamTreeStatus,
};
use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::RepositoryExt as _;
use but_core::commit::Headers;
use but_graph::projection::{StackCommitFlags, Workspace};
use but_rebase::RebaseStep;
use but_serde::BStringForFrontend;
use gix::merge::tree::TreatAsUnresolved;

/// Compute upstream integration statuses for all stacks in the workspace.
///
/// Uses `but-graph` projections directly, avoiding the legacy `stack_details_v3` path.
pub fn workspace_upstream_integration_statuses(
    ctx: &mut but_ctx::Context,
    target_commit_id: Option<gix::ObjectId>,
    review_map: &HashMap<String, but_forge::ForgeReview>,
) -> Result<StackStatuses> {
    let _guard = ctx.exclusive_worktree_access();

    // Build the but-graph workspace projection.
    let repo = ctx.clone_repo_for_merging()?;
    let meta = ctx.meta()?;
    let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
    let workspace = graph.into_workspace()?;

    upstream_integration_statuses_inner(&repo, &workspace, target_commit_id, review_map)
}

/// Core status computation that operates on a workspace projection directly.
///
/// This is the implementation behind [`workspace_upstream_integration_statuses()`],
/// separated so it can be tested without a [`but_ctx::Context`].
pub fn upstream_integration_statuses_inner(
    repo: &gix::Repository,
    workspace: &Workspace,
    target_commit_id: Option<gix::ObjectId>,
    review_map: &HashMap<String, but_forge::ForgeReview>,
) -> Result<StackStatuses> {
    // Determine old and new target commit IDs.
    let old_target_id = workspace
        .metadata
        .as_ref()
        .and_then(|m| m.target_commit_id)
        .or_else(|| workspace.target_commit.as_ref().map(|t| t.commit_id))
        .context("failed to get target base oid")?;

    let target_ref_name = workspace
        .target_ref
        .as_ref()
        .map(|tr| tr.ref_name.clone())
        .context("workspace has no target ref")?;

    let new_target = match target_commit_id {
        Some(oid) => oid,
        None => {
            repo.find_reference(target_ref_name.as_ref())?
                .peel_to_commit()?
                .id
        }
    };

    if new_target == old_target_id {
        return Ok(StackStatuses::UpToDate);
    }

    // Compute worktree conflicts.
    let worktree_conflicts = compute_worktree_conflicts(repo, workspace, new_target)?;

    // Compute per-stack statuses using the graph projection.
    let repo_in_memory = repo.clone().with_object_memory();
    let statuses = workspace
        .stacks
        .iter()
        .map(|stack| get_stack_status_from_graph(&repo_in_memory, new_target, stack, review_map))
        .collect::<Result<Vec<_>>>()?;

    Ok(StackStatuses::UpdatesRequired {
        worktree_conflicts,
        statuses,
    })
}

/// Compute worktree conflicts by three-way merging the working directory tree
/// against the new target.
fn compute_worktree_conflicts(
    repo: &gix::Repository,
    workspace: &Workspace,
    new_target: gix::ObjectId,
) -> Result<Vec<BStringForFrontend>> {
    let stack_tips: Vec<gix::ObjectId> = workspace
        .stacks
        .iter()
        .filter_map(|s| s.tip())
        .chain(std::iter::once(new_target))
        .collect();

    let merge_base_tree = repo
        .merge_base_octopus(stack_tips)?
        .object()?
        .into_commit()
        .tree_id()?;

    #[expect(deprecated, reason = "calls repo.create_wd_tree")]
    let workdir_tree = repo.create_wd_tree(0)?;

    let target_tree = repo.find_commit(new_target)?.tree_id()?;

    let (merge_options, _) = repo.merge_options_no_rewrites_fail_fast()?;

    let committed_conflicts = repo
        .merge_trees(
            merge_base_tree,
            repo.head()?.peel_to_commit()?.tree_id()?,
            target_tree,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(TreatAsUnresolved::git()))
        .map(|c| c.ours.location().to_owned())
        .collect::<Vec<_>>();

    let worktree_conflicts = repo
        .merge_trees(
            merge_base_tree,
            workdir_tree,
            target_tree,
            repo.default_merge_labels(),
            merge_options,
        )?
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(TreatAsUnresolved::git()))
        .filter(|c| !committed_conflicts.iter().any(|cc| cc == c.ours.location()))
        .map(|c| c.ours.location().into())
        .collect::<Vec<BStringForFrontend>>();

    Ok(worktree_conflicts)
}

/// Determine the status of a single stack using `but-graph` projection data.
///
/// Segments are iterated bottom-to-top (the graph stores them top-to-bottom).
/// For each segment (branch), we check whether all commits are integrated
/// (via commit flags or forge review), then do a trial rebase of
/// non-integrated commits to detect conflicts.
fn get_stack_status_from_graph(
    repo: &gix::Repository,
    new_target_commit_id: gix::ObjectId,
    stack: &but_graph::projection::Stack,
    review_map: &HashMap<String, but_forge::ForgeReview>,
) -> Result<StackStatus> {
    let stack_id = stack.id;
    let mut last_head = new_target_commit_id;
    let mut branch_statuses: Vec<NameAndStatus> = vec![];
    let mut bottom_selector: Option<StackSelector> = None;

    // Segments are ordered top-to-bottom; iterate bottom-to-top.
    for segment in stack.segments.iter().rev() {
        let branch_name = segment
            .ref_name()
            .map(|r| r.shorten().to_str_lossy().to_string())
            .unwrap_or_default();

        // Filter to non-integrated commits in this segment.
        // Segments store commits top-to-bottom (newest first).
        let all_commits = &segment.commits;

        if all_commits.is_empty()
            || all_commits
                .iter()
                .all(|c| c.flags.contains(StackCommitFlags::EarlyEnd))
        {
            // Empty segment — use its reference as bottom selector if needed.
            if bottom_selector.is_none()
                && let Some(ref_name) = segment.ref_name()
            {
                bottom_selector = Some(StackSelector::Reference(ref_name.to_owned()));
            }
            branch_statuses.push(NameAndStatus {
                name: branch_name,
                status: BranchStatus::Empty,
            });
            continue;
        }

        let branch_head = &all_commits[0]; // top-most commit
        let branch_head_string = branch_head.id.to_string();

        // Check integration via forge review.
        let is_integrated_via_review = review_map
            .get(&branch_name)
            .is_some_and(|review| review.is_merged_at_commit(&branch_head_string));

        // Check integration via commit flags (all commits integrated).
        let is_integrated_via_commits = all_commits
            .iter()
            .all(|c| c.flags.contains(StackCommitFlags::Integrated));

        if is_integrated_via_commits || is_integrated_via_review {
            if bottom_selector.is_none() {
                // Use the bottom-most commit as selector.
                if let Some(bottom) = all_commits.last() {
                    bottom_selector = Some(StackSelector::Commit(bottom.id));
                }
            }
            branch_statuses.push(NameAndStatus {
                name: branch_name,
                status: BranchStatus::Integrated,
            });
            continue;
        }

        // Non-integrated branch — find the bottom-most non-integrated commit.
        if bottom_selector.is_none() {
            let bottom_non_integrated = all_commits
                .iter()
                .rev()
                .find(|c| !c.flags.contains(StackCommitFlags::Integrated));
            if let Some(commit) = bottom_non_integrated {
                bottom_selector = Some(StackSelector::Commit(commit.id));
            }
        }

        // Trial rebase: pick non-integrated commits bottom-to-top onto last_head.
        let non_integrated_ids: Vec<gix::ObjectId> = all_commits
            .iter()
            .rev()
            .filter(|c| !c.flags.contains(StackCommitFlags::Integrated))
            .map(|c| c.id)
            .collect();

        let steps: Vec<RebaseStep> = non_integrated_ids
            .iter()
            .map(|id| RebaseStep::Pick {
                commit_id: *id,
                new_message: None,
            })
            .collect();

        let mut rebase = but_rebase::Rebase::new(repo, Some(last_head), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        let output = rebase.rebase()?;

        let any_conflicted = output.commit_mapping.iter().any(|(_base, _old, new)| {
            repo.find_commit(*new).ok().is_some_and(|c| {
                c.decode().ok().is_some_and(|decoded| {
                    let headers = Headers::try_from_commit_headers(|| decoded.extra_headers());
                    but_core::commit::is_conflicted(decoded.message, headers.as_ref())
                })
            })
        });

        last_head = output.top_commit;

        branch_statuses.push(NameAndStatus {
            name: branch_name,
            status: if any_conflicted {
                BranchStatus::Conflicted { rebasable: false }
            } else {
                BranchStatus::SafelyUpdatable
            },
        });
    }

    StackStatus::create(
        stack_id,
        UpstreamTreeStatus::Empty,
        branch_statuses,
        bottom_selector,
    )
}
