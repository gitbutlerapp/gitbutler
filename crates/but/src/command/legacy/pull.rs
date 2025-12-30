use but_ctx::Context;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::Integrated,
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
};

use crate::utils::OutputChannel;

pub async fn handle(ctx: &Context, out: &mut OutputChannel) -> anyhow::Result<()> {
    let status = but_api::legacy::virtual_branches::upstream_integration_statuses(
        ctx.legacy_project.id,
        None,
    )
    .await?;
    let resolutions = match status {
        UpToDate => {
            if let Some(out) = out.for_human() {
                writeln!(out, "✅ Everything is up to date")?;
            }
            None
        }
        UpdatesRequired {
            worktree_conflicts,
            statuses,
        } => {
            if !worktree_conflicts.is_empty() {
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "❗️ There are uncommitted changes in the worktree that may conflict with the updates. Please commit or stash them and try again."
                    )?;
                }
                None
            } else {
                if let Some(out) = out.for_human() {
                    writeln!(out, "🔄 Updating branches...")?;
                }
                let mut resolutions = vec![];
                for (maybe_stack_id, status) in statuses {
                    let Some(stack_id) = maybe_stack_id else {
                        if let Some(out) = out.for_human() {
                            writeln!(out, "No stack ID, assuming we're on single-branch mode...")?;
                        }
                        continue;
                    };
                    let approach = if status.branch_statuses.iter().all(|s| s.status == Integrated)
                        && status.tree_status
                            != gitbutler_branch_actions::upstream_integration::TreeStatus::Conflicted
                    {
                        ResolutionApproach::Delete
                    } else {
                        ResolutionApproach::Rebase
                    };
                    let resolution = Resolution {
                        stack_id,
                        approach,
                        delete_integrated_branches: true,
                    };
                    resolutions.push(resolution);
                }
                Some(resolutions)
            }
        }
    };

    if let Some(resolutions) = resolutions {
        but_api::legacy::virtual_branches::integrate_upstream(
            ctx.legacy_project.id,
            resolutions,
            None,
        )
        .await?;
    }
    Ok(())
}
