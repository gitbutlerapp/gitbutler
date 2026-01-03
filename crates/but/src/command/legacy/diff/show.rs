use std::fmt::Write;

use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_ctx::Context;
use but_hunk_assignment::WorktreeChanges;

use super::display::{DiffDisplay, TreeChangeWithPatch};
use crate::{IdMap, id::UncommittedCliId, utils::OutputChannel};

#[allow(clippy::large_enum_variant)]
pub(crate) enum Filter {
    Unassigned,
    Uncommitted(UncommittedCliId),
}

pub(crate) fn worktree(
    wt_changes: WorktreeChanges,
    id_map: IdMap,
    out: &mut OutputChannel,
    filter: Option<Filter>,
) -> anyhow::Result<()> {
    let assignments: Vec<_> = wt_changes
        .assignments
        .iter()
        .filter(|a| match &filter {
            None => true,
            Some(Filter::Unassigned) => a.stack_id.is_none(),
            Some(Filter::Uncommitted(id)) => {
                if id.is_entire_file {
                    a.path_bytes == id.hunk_assignments.first().path_bytes
                } else {
                    a == &id.hunk_assignments.first()
                }
            }
        })
        .collect();

    if assignments.is_empty() {
        writeln!(out, "No diffs to show.")?;
    } else {
        for assignment in assignments {
            let cli_id = id_map.resolve_uncommitted_hunk(assignment);
            write!(out, "{}", assignment.print_diff(cli_id.as_ref()))?;
        }
    }
    Ok(())
}

pub(crate) fn commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    id: gix::ObjectId,
    path: Option<BString>,
) -> anyhow::Result<()> {
    let result = but_api::diff::commit_details(ctx, id, ComputeLineStats::No)?;
    for change in result.diff_with_first_parent {
        if path.as_ref().is_none_or(|p| p == &change.path) {
            let patch = but_api::legacy::diff::tree_change_diffs(ctx, change.clone().into())
                .ok()
                .flatten();
            let diff = TreeChangeWithPatch::new(change.into(), patch);
            write!(out, "{}", diff.print_diff(None))?;
        }
    }
    Ok(())
}

pub(crate) fn branch(
    ctx: &Context,
    out: &mut OutputChannel,
    short_name: String,
) -> anyhow::Result<()> {
    let result = but_api::branch::branch_diff(ctx, short_name)?;
    for change in result.changes {
        let patch = but_api::legacy::diff::tree_change_diffs(ctx, change.clone())
            .ok()
            .flatten();

        let diff = TreeChangeWithPatch::new(change, patch);
        write!(out, "{}", diff.print_diff(None))?;
    }
    Ok(())
}
