use bstr::ByteSlice;
use but_api::workspace::WorkspaceIntegrateUpstreamOutcome;
use but_core::{DryRun, sync::RepoExclusive};
use but_ctx::Context;
use but_workspace::{
    RefInfo,
    branch::Stack,
    ref_info::{Lane, LocalCommitRelation, Segment},
    ui::PushStatus,
    worktrees::{WorktreeBase, WorktreeInfo},
};

use crate::args::PullUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BranchStatus {
    Clear,
    Integrated,
    Conflicted,
    Empty,
}

impl BranchStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            BranchStatus::Clear | BranchStatus::Empty => "updatable",
            BranchStatus::Integrated => "integrated",
            BranchStatus::Conflicted => "conflicted_rebasable",
        }
    }

    pub(crate) fn needs_update(self) -> bool {
        matches!(self, BranchStatus::Integrated | BranchStatus::Conflicted)
    }
}

#[derive(Debug)]
pub(crate) struct BranchStatusInfo {
    pub(crate) name: String,
    pub(crate) status: BranchStatus,
}

pub(crate) struct IntegrationPreview {
    pub(crate) current: RefInfo,
    pub(crate) outcome: WorkspaceIntegrateUpstreamOutcome,
    pub(crate) statuses: Vec<BranchStatusInfo>,
}

pub(crate) fn dry_run_integration(
    ctx: &Context,
    update: &[PullUpdate],
) -> anyhow::Result<IntegrationPreview> {
    let mut ctx = ctx.to_sync().into_thread_local();
    let mut guard = ctx.exclusive_worktree_access();
    dry_run_integration_with_perm(&mut ctx, update, guard.write_permission())
}

pub(crate) fn dry_run_integration_with_perm(
    ctx: &mut Context,
    update: &[PullUpdate],
    perm: &mut RepoExclusive,
) -> anyhow::Result<IntegrationPreview> {
    let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
    let updates =
        but_api::workspace::rebase_lane_bottoms(selected_lanes(&current_head_info, update));
    let preview = but_api::workspace::workspace_integrate_upstream_with_perm(
        ctx,
        updates,
        DryRun::Yes,
        perm,
    )?;
    let statuses = classify(&current_head_info, update, &preview.workspace_state);
    Ok(IntegrationPreview {
        current: current_head_info,
        outcome: preview,
        statuses,
    })
}

/// The lanes resting on the target that `update` selects for rebasing.
pub(crate) fn selected_lanes<'a>(head_info: &'a RefInfo, update: &[PullUpdate]) -> Vec<Lane<'a>> {
    let stacks = head_info
        .stacks
        .iter()
        .filter(|_| update.contains(&PullUpdate::Workspace))
        .map(Stack::lane);
    let worktrees = head_info
        .worktrees
        .iter()
        .filter(|worktree| {
            update.contains(&PullUpdate::Worktrees) && rebasable_base(worktree).is_some()
        })
        .map(WorktreeInfo::lane);
    stacks.chain(worktrees).collect()
}

/// What `worktree` rests on, if that is the target and it has a branch or commits to move there.
fn rebasable_base(worktree: &WorktreeInfo) -> Option<gix::ObjectId> {
    match worktree.base {
        Some(WorktreeBase::Outside(base))
            if worktree.ref_name.is_some() || worktree.commits().next().is_some() =>
        {
            Some(base)
        }
        _ => None,
    }
}

/// The lanes `update` selects along with every lane stacked on one of them, as rebasing a lane
/// carries those along.
fn rebased_lanes<'a>(head_info: &'a RefInfo, update: &[PullUpdate]) -> Vec<Lane<'a>> {
    let selected = selected_lanes(head_info, update);
    head_info
        .lanes()
        .filter(|lane| {
            let bottom = head_info
                .lanes_beneath(*lane)
                .last()
                .map_or(*lane, |(bottom, _)| *bottom);
            selected.contains(&bottom)
        })
        .collect()
}

/// Whether a worktree selected by `update` rests on anything but `target_tip`.
pub(crate) fn has_worktree_behind(
    head_info: &RefInfo,
    update: &[PullUpdate],
    target_tip: gix::ObjectId,
) -> bool {
    update.contains(&PullUpdate::Worktrees)
        && head_info
            .worktrees
            .iter()
            .any(|worktree| rebasable_base(worktree).is_some_and(|base| base != target_tip))
}

pub(crate) fn classify(
    current: &RefInfo,
    update: &[PullUpdate],
    preview: &but_api::WorkspaceState,
) -> Vec<BranchStatusInfo> {
    let preview_conflicts = preview.conflicts_by_reference();

    rebased_lanes(current, update)
        .into_iter()
        .flat_map(|lane| lane.segments)
        .map(|segment| classify_branch(segment, &preview_conflicts))
        .collect()
}

pub(crate) fn has_cleanup_candidate(head_info: &RefInfo, update: &[PullUpdate]) -> bool {
    rebased_lanes(head_info, update)
        .into_iter()
        .flat_map(|lane| lane.segments)
        .any(|segment| {
            matches!(segment.push_status, PushStatus::Integrated)
                || segment
                    .commits
                    .iter()
                    .any(|commit| matches!(commit.relation, LocalCommitRelation::Integrated(_)))
                || (segment.commits.is_empty() && segment.remote_tracking_ref_name.is_some())
        })
}

fn classify_branch(
    segment: &Segment,
    preview_conflicts: &std::collections::HashMap<Vec<u8>, bool>,
) -> BranchStatusInfo {
    let name = branch_display_name(segment);
    let Some(ref_info) = &segment.ref_info else {
        return BranchStatusInfo {
            name,
            status: BranchStatus::Clear,
        };
    };

    let Some(&has_conflicts) = preview_conflicts.get(ref_info.ref_name.as_bstr().as_bytes()) else {
        return BranchStatusInfo {
            name,
            status: BranchStatus::Integrated,
        };
    };

    let status = if segment.commits.is_empty() {
        BranchStatus::Empty
    } else if has_conflicts {
        BranchStatus::Conflicted
    } else {
        BranchStatus::Clear
    };
    BranchStatusInfo { name, status }
}

fn branch_display_name(segment: &Segment) -> String {
    segment
        .ref_info
        .as_ref()
        .map(|ref_info| ref_info.ref_name.shorten().to_string())
        .unwrap_or_else(|| "Unnamed segment".to_string())
}
