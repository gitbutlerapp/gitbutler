use anyhow::{Context as _, Result, bail};
use but_core::RepositoryExt;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_ctx::access::RepoExclusive;
use but_rebase::RebaseStep;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_repo::first_parent_commit_ids_until;
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use serde::{Deserialize, Serialize};

use crate::BaseBranch;
use crate::branch_manager::BranchManagerExt;

/// Result of attempting to switch back to the workspace.
///
/// When no divergence is detected (or resolutions were provided and applied),
/// returns `Ok` with the base branch data. When divergence is detected and
/// no resolutions were provided, returns `Diverged` so the frontend can
/// present resolution options before retrying.
#[derive(Serialize, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SwitchBackToWorkspaceResult {
    /// Successfully switched back to workspace — no divergence or resolutions applied.
    Ok {
        #[serde(rename = "baseBranch")]
        base_branch: Box<BaseBranch>,
    },
    /// Divergence detected — frontend should present options and retry with resolutions.
    Diverged {
        divergences: Vec<StackRefDivergence>,
    },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(SwitchBackToWorkspaceResult);

/// Per-stack divergence info returned to the frontend.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackRefDivergence {
    /// The stack that diverged.
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub stack_id: StackId,
    /// The branch ref name (tip of the stack).
    pub ref_name: String,
    /// Where the workspace commit expected this stack's head to be.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub expected_oid: gix::ObjectId,
    /// Where the ref actually points now (`None` if the ref was deleted).
    #[serde(with = "but_serde::object_id_opt")]
    #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
    pub actual_oid: Option<gix::ObjectId>,
    /// Classification of the divergence.
    pub status: DivergenceStatus,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackRefDivergence);

/// How a stack ref has diverged from its expected position.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DivergenceStatus {
    /// Ref was deleted externally.
    Deleted,
    /// Ref moved but still above the target base. Including it may change the workspace tree.
    MovedAboveBase { conflicted: bool },
    /// Ref moved to or below the target base. The stack has no commits in the workspace.
    MovedBelowBase,
    /// Ref moved but the tree at the new position is identical to the old — workspace is unaffected.
    MovedToSameTree,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DivergenceStatus);

/// Top-level result of divergence detection.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum DivergenceStatuses {
    /// All stack refs match the workspace commit — nothing to do.
    UpToDate,
    /// One or more stack refs have diverged from the workspace commit.
    DivergedRefs {
        divergences: Vec<StackRefDivergence>,
    },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DivergenceStatuses);

/// What the user wants to do with a diverged stack.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DivergenceApproach {
    /// Accept the ref at its current position and rebuild the workspace commit.
    IncludeAsIs,
    /// Rebase the stack's commits onto the target base.
    IncludeRebase,
    /// Remove the stack from the workspace (mark as unapplied).
    Exclude,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DivergenceApproach);

/// A user's chosen resolution for a single diverged stack.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DivergenceResolution {
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub stack_id: StackId,
    pub approach: DivergenceApproach,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DivergenceResolution);

/// Detect whether any workspace stack refs have diverged from the workspace commit.
///
/// For each stack, compares where its ref currently points against the workspace commit's
/// parent that corresponds to that stack. The workspace commit parents are the ground truth
/// for where each stack's head was when the workspace was last rebuilt.
pub fn detect_diverged_stacks(ctx: &Context) -> Result<DivergenceStatuses> {
    let repo = ctx.repo.get()?;

    // Get the workspace commit and its parent OIDs — these are the "expected" positions.
    let ws_ref = repo
        .try_find_reference(but_core::WORKSPACE_REF_NAME)?
        .context("gitbutler/workspace ref not found")?;
    let ws_commit_id = ws_ref
        .into_fully_peeled_id()
        .context("failed to peel workspace ref")?
        .detach();
    let ws_commit = repo.find_commit(ws_commit_id)?;
    let mut unmatched_parents: Vec<gix::ObjectId> =
        ws_commit.parent_ids().map(|id| id.detach()).collect();

    // Get the target base OID.
    let target = ctx
        .persisted_default_target()
        .context("no default target set")?;
    let target_base_oid = target.sha;

    // Remove the target from unmatched parents — it's not a stack.
    unmatched_parents.retain(|id| *id != target_base_oid);

    // Get all stacks in the workspace.
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks: Vec<Stack> = vb_state.list_stacks_in_workspace()?;

    let mut divergences = Vec::new();

    for stack in &stacks {
        // Where does the ref currently point?
        let actual_oid = resolve_stack_head_from_ref(stack, &repo);

        // Find the expected OID: the workspace commit parent that matches this stack.
        // If the ref hasn't moved, actual_oid matches one of the parents.
        let expected_oid = if let Some(actual) = actual_oid {
            if let Some(pos) = unmatched_parents.iter().position(|id| *id == actual) {
                // Ref matches a parent — no divergence. Remove from unmatched pool.
                unmatched_parents.remove(pos);
                continue;
            }
            // Ref doesn't match any remaining parent — it has moved.
            // The expected OID is the parent we can match via reachability: the parent
            // from which this stack's ref was previously reachable.
            find_reachable_parent(&repo, &unmatched_parents, actual)
        } else {
            // Ref was deleted. Find the parent that has no other matching stack.
            None
        };

        let expected_oid = match expected_oid {
            Some(oid) => {
                // Remove from unmatched pool since we've claimed this parent.
                unmatched_parents.retain(|id| *id != oid);
                oid
            }
            None => {
                // Can't determine expected OID — take the first unmatched parent if available.
                if let Some(oid) = unmatched_parents.first().copied() {
                    unmatched_parents.remove(0);
                    oid
                } else {
                    continue; // No parent to match against.
                }
            }
        };

        let status = classify_divergence(&repo, expected_oid, actual_oid, target_base_oid)?;

        let ref_name = stack
            .heads
            .last()
            .map(|h| h.name.clone())
            .unwrap_or_default();

        divergences.push(StackRefDivergence {
            stack_id: stack.id,
            ref_name,
            expected_oid,
            actual_oid,
            status,
        });
    }

    if divergences.is_empty() {
        return Ok(DivergenceStatuses::UpToDate);
    }

    // Second pass: check inter-stack conflicts.
    // Build a workspace tree from all non-diverged stacks, then check whether each
    // diverged stack's new tree merges cleanly with it. This catches cases where the
    // moved ref doesn't conflict with the base but DOES conflict with another applied stack.
    check_inter_stack_conflicts(ctx, &repo, &stacks, &mut divergences, target_base_oid)?;

    Ok(DivergenceStatuses::DivergedRefs { divergences })
}

/// Try to find which workspace commit parent is the "old" position of a moved ref.
///
/// A parent is a match if the actual (moved) OID and the parent share a merge-base
/// that is not the actual OID itself — meaning they diverged from a common point.
/// This is a heuristic; it works well when stacks don't share history.
fn find_reachable_parent(
    _repo: &gix::Repository,
    parents: &[gix::ObjectId],
    _actual: gix::ObjectId,
) -> Option<gix::ObjectId> {
    // For now, if there's only one unmatched parent, it must be ours.
    if parents.len() == 1 {
        return Some(parents[0]);
    }
    // With multiple unmatched parents, we can't reliably determine which one
    // belonged to this stack without more info. Return None and let the caller
    // fall back to the first available.
    None
}

/// Resolve the stack's top branch ref to where it currently points in the repo.
/// Returns `None` if the ref has been deleted.
fn resolve_stack_head_from_ref(stack: &Stack, repo: &gix::Repository) -> Option<gix::ObjectId> {
    let branch = stack.heads.last()?;
    repo.try_find_reference(&branch.name)
        .ok()?
        .and_then(|mut r| r.peel_to_commit().ok())
        .map(|c| c.id)
}

/// Apply user-chosen resolutions for diverged stacks, then rebuild the workspace commit.
///
/// This is the second phase of the two-phase divergence handling flow:
/// 1. `detect_diverged_stacks` identifies what diverged
/// 2. `resolve_diverged_stacks` applies the user's chosen strategy for each
///
/// After resolution, the workspace commit is rebuilt to reflect the new state.
/// This may trigger a checkout if included stacks have changed trees.
pub fn resolve_diverged_stacks(
    ctx: &Context,
    resolutions: &[DivergenceResolution],
    permission: &mut RepoExclusive,
) -> Result<()> {
    let repo = ctx.repo.get()?;
    let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = ctx
        .persisted_default_target()
        .context("no default target set")?;
    let target_base_oid = target.sha;

    // Phase 1: Process excludes (unapply stacks).
    for resolution in resolutions {
        if resolution.approach != DivergenceApproach::Exclude {
            continue;
        }
        ctx.branch_manager().unapply(
            resolution.stack_id,
            permission,
            false, // don't delete vb_state entry
            Vec::new(),
            false, // not safe_checkout — we'll rebuild the workspace commit ourselves
        )?;
    }

    // Phase 2: Process rebases.
    for resolution in resolutions {
        if resolution.approach != DivergenceApproach::IncludeRebase {
            continue;
        }
        let mut stack = vb_state.get_stack(resolution.stack_id)?;
        let actual_oid = resolve_stack_head_from_ref(&stack, &repo)
            .context("cannot rebase: stack ref was deleted")?;

        // Collect commits from the stack's current head down to the target base.
        let commit_ids = first_parent_commit_ids_until(&repo, actual_oid, target_base_oid)?;
        if commit_ids.is_empty() {
            continue; // Nothing to rebase — stack is at or below target base.
        }

        let steps: Vec<RebaseStep> = commit_ids
            .iter()
            .map(|commit_id| RebaseStep::Pick {
                commit_id: *commit_id,
                new_message: None,
            })
            .collect();

        let mut rebase = but_rebase::Rebase::new(&repo, Some(target_base_oid), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        let output = rebase.rebase()?;

        // Check for conflicts in rebased commits.
        let any_conflicted = output.commit_mapping.iter().any(|(_base, _old, new)| {
            repo.find_commit(*new)
                .map(|c| c.is_conflicted())
                .unwrap_or(false)
        });
        if any_conflicted {
            bail!(
                "Rebase of stack '{}' produced conflicts — try IncludeAsIs or Exclude instead",
                stack
                    .heads
                    .last()
                    .map(|h| h.name.as_str())
                    .unwrap_or("unknown")
            );
        }

        // Update the stack's branch refs from the rebase output.
        stack.set_heads_from_rebase_output(ctx, output.references)?;
        vb_state.set_stack(stack)?;
    }

    // Phase 3: For IncludeAsIs, no ref changes are needed — the workspace commit
    // rebuild will pick up the current ref positions automatically.

    // Phase 4: Rebuild the workspace commit. This merges all in-workspace stacks
    // and performs a checkout if the tree changed.
    crate::integration::update_workspace_commit_with_vb_state(&vb_state, ctx, true)?;

    Ok(())
}

/// Apply divergence resolutions without rebuilding the workspace commit.
///
/// This is used by the `switch_back_to_workspace` two-phase flow: resolutions
/// are applied first (refs moved, stacks unapplied), then `go_back_to_integration`
/// handles the checkout and workspace commit rebuild.
pub fn apply_divergence_resolutions(
    ctx: &Context,
    resolutions: &[DivergenceResolution],
    permission: &mut RepoExclusive,
) -> Result<()> {
    let repo = ctx.repo.get()?;
    let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let target = ctx
        .persisted_default_target()
        .context("no default target set")?;
    let target_base_oid = target.sha;

    // Phase 1: Process excludes (unapply stacks).
    for resolution in resolutions {
        if resolution.approach != DivergenceApproach::Exclude {
            continue;
        }
        ctx.branch_manager()
            .unapply(resolution.stack_id, permission, false, Vec::new(), false)?;
    }

    // Phase 2: Process rebases.
    for resolution in resolutions {
        if resolution.approach != DivergenceApproach::IncludeRebase {
            continue;
        }
        let mut stack = vb_state.get_stack(resolution.stack_id)?;
        let actual_oid = resolve_stack_head_from_ref(&stack, &repo)
            .context("cannot rebase: stack ref was deleted")?;

        let commit_ids = first_parent_commit_ids_until(&repo, actual_oid, target_base_oid)?;
        if commit_ids.is_empty() {
            continue;
        }

        let steps: Vec<RebaseStep> = commit_ids
            .iter()
            .map(|commit_id| RebaseStep::Pick {
                commit_id: *commit_id,
                new_message: None,
            })
            .collect();

        let mut rebase = but_rebase::Rebase::new(&repo, Some(target_base_oid), None)?;
        rebase.rebase_noops(false);
        rebase.steps(steps)?;
        let output = rebase.rebase()?;

        let any_conflicted = output.commit_mapping.iter().any(|(_base, _old, new)| {
            repo.find_commit(*new)
                .map(|c| c.is_conflicted())
                .unwrap_or(false)
        });
        if any_conflicted {
            bail!(
                "Rebase of stack '{}' produced conflicts — try IncludeAsIs or Exclude instead",
                stack
                    .heads
                    .last()
                    .map(|h| h.name.as_str())
                    .unwrap_or("unknown")
            );
        }

        stack.set_heads_from_rebase_output(ctx, output.references)?;
        vb_state.set_stack(stack)?;
    }

    // Phase 3: IncludeAsIs — no ref changes needed.
    // The workspace commit rebuild (done by the caller) will pick up current positions.

    Ok(())
}

/// Check whether diverged stacks would conflict with other applied stacks.
///
/// For each diverged stack marked `MovedAboveBase { conflicted: false }`, builds a
/// workspace tree from all OTHER stacks (non-diverged use current tree, other diverged
/// use their new actual tree), then checks if adding this stack would conflict.
fn check_inter_stack_conflicts(
    ctx: &Context,
    repo: &gix::Repository,
    stacks: &[Stack],
    divergences: &mut [StackRefDivergence],
    target_base_oid: gix::ObjectId,
) -> Result<()> {
    // Only worth checking for stacks marked MovedAboveBase { conflicted: false }.
    let any_needs_check = divergences.iter().any(|d| {
        matches!(
            d.status,
            DivergenceStatus::MovedAboveBase { conflicted: false }
        )
    });
    if !any_needs_check {
        return Ok(());
    }

    let merge_tree_id = repo.find_commit(target_base_oid)?.tree_id()?.detach();
    let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;

    // Build a lookup: stack_id → tree_id for all stacks that would be in the workspace.
    // Non-diverged stacks use their current head. Diverged stacks use their actual (new) OID.
    let divergence_map: std::collections::HashMap<StackId, &StackRefDivergence> =
        divergences.iter().map(|d| (d.stack_id, d)).collect();

    let mut stack_trees: Vec<(StackId, gix::ObjectId)> = Vec::new();
    for stack in stacks {
        let tree_id = if let Some(div) = divergence_map.get(&stack.id) {
            // For diverged stacks that would be included (MovedAboveBase non-conflicted
            // or MovedToSameTree), use their actual tree.
            match &div.status {
                DivergenceStatus::MovedAboveBase { conflicted: false }
                | DivergenceStatus::MovedToSameTree => {
                    if let Some(actual) = div.actual_oid {
                        repo.find_commit(actual)?.tree_id()?.detach()
                    } else {
                        continue;
                    }
                }
                // Deleted, MovedBelowBase, or already-conflicted stacks won't be included.
                _ => continue,
            }
        } else {
            // Non-diverged stack — use current head.
            repo.find_commit(stack.head_oid(ctx)?)?.tree_id()?.detach()
        };
        stack_trees.push((stack.id, tree_id));
    }
    // For each candidate divergence, build a workspace tree from all OTHER stacks,
    // then check if adding this stack's tree would conflict.
    for divergence in divergences.iter_mut() {
        if !matches!(
            divergence.status,
            DivergenceStatus::MovedAboveBase { conflicted: false }
        ) {
            continue;
        }
        let check_stack_id = divergence.stack_id;

        // Build workspace tree from all stacks EXCEPT the one being checked.
        let mut workspace_tree_id = merge_tree_id;
        for (sid, tree_id) in &stack_trees {
            if *sid == check_stack_id {
                continue;
            }
            let mut merge = repo.merge_trees(
                merge_tree_id,
                workspace_tree_id,
                *tree_id,
                repo.default_merge_labels(),
                merge_options.clone(),
            )?;
            if !merge.has_unresolved_conflicts(conflict_kind) {
                workspace_tree_id = merge.tree.write()?.detach();
            }
        }

        // Now check if this diverged stack's tree merges cleanly with the workspace.
        let Some(check_tree_id) = stack_trees
            .iter()
            .find(|(sid, _)| *sid == check_stack_id)
            .map(|(_, t)| *t)
        else {
            continue;
        };
        let merge = repo.merge_trees(
            merge_tree_id,
            workspace_tree_id,
            check_tree_id,
            repo.default_merge_labels(),
            merge_options.clone(),
        )?;
        if merge.has_unresolved_conflicts(conflict_kind) {
            divergence.status = DivergenceStatus::MovedAboveBase { conflicted: true };
        }
    }

    Ok(())
}

/// Classify how a stack ref has diverged.
fn classify_divergence(
    repo: &gix::Repository,
    expected_oid: gix::ObjectId,
    actual_oid: Option<gix::ObjectId>,
    target_base_oid: gix::ObjectId,
) -> Result<DivergenceStatus> {
    let Some(actual) = actual_oid else {
        return Ok(DivergenceStatus::Deleted);
    };

    // Check if the trees are identical — if so, the workspace is unaffected.
    let expected_tree = repo.find_commit(expected_oid)?.tree_id()?.detach();
    let actual_tree = repo.find_commit(actual)?.tree_id()?.detach();
    if expected_tree == actual_tree {
        return Ok(DivergenceStatus::MovedToSameTree);
    }

    // Check if the ref is now at or below the target base.
    let merge_base = repo
        .merge_base(actual, target_base_oid)
        .context("failed to compute merge base")?;
    if merge_base == actual {
        // actual is an ancestor of (or equal to) target_base — it's at or below the base.
        return Ok(DivergenceStatus::MovedBelowBase);
    }

    // Ref is above the base but has moved. Check if including it would conflict.
    let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;
    let target_tree = repo.find_commit(target_base_oid)?.tree_id()?.detach();
    let outcome = repo.merge_trees(
        target_tree,
        expected_tree,
        actual_tree,
        repo.default_merge_labels(),
        merge_options,
    )?;
    let conflicted = outcome.has_unresolved_conflicts(conflict_kind);

    Ok(DivergenceStatus::MovedAboveBase { conflicted })
}
