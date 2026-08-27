use bstr::ByteSlice;
use but_api::workspace::WorkspaceIntegrateUpstreamOutcome;
use but_core::{DryRun, sync::RepoExclusive};
use but_ctx::Context;
use but_workspace::{
    RefInfo,
    ref_info::{LocalCommitRelation, Segment},
    ui::PushStatus,
};

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

pub(crate) fn dry_run_integration(ctx: &Context) -> anyhow::Result<IntegrationPreview> {
    let mut ctx = ctx.to_sync().into_thread_local();
    let mut guard = ctx.exclusive_worktree_access();
    dry_run_integration_with_perm(&mut ctx, guard.write_permission())
}

pub(crate) fn dry_run_integration_with_perm(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<IntegrationPreview> {
    let current_head_info = but_api::legacy::workspace::head_info(ctx)?;
    let updates = but_api::workspace::rebase_stack_bottoms(&current_head_info);
    let preview = but_api::workspace::workspace_integrate_upstream_with_perm(
        ctx,
        updates,
        DryRun::Yes,
        perm,
    )?;
    let statuses = classify(&current_head_info, &preview.workspace_state);
    Ok(IntegrationPreview {
        current: current_head_info,
        outcome: preview,
        statuses,
    })
}

pub(crate) fn classify(
    current: &RefInfo,
    preview: &but_api::WorkspaceState,
) -> Vec<BranchStatusInfo> {
    let preview_conflicts = preview.conflicts_by_reference();

    current
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .map(|segment| classify_branch(segment, &preview_conflicts))
        .collect()
}

pub(crate) fn has_cleanup_candidate(head_info: &RefInfo) -> bool {
    head_info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
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
