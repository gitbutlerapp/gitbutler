use std::fmt::Write;

use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use gitbutler_stack::StackId;

use super::display::{DiffDisplay, TreeChangeWithPatch};
use crate::{IdMap, id::UncommittedCliId, utils::OutputChannel};

#[allow(clippy::large_enum_variant)]
pub(crate) enum Filter {
    Unassigned,
    Uncommitted(UncommittedCliId),
    Stack(StackId),
}

pub(crate) fn worktree(
    id_map: IdMap,
    out: &mut OutputChannel,
    filter: Option<Filter>,
) -> anyhow::Result<()> {
    let mut short_id_assignment_pairs: Vec<(&str, &HunkAssignment)> = id_map
        .uncommitted_hunks
        .iter()
        .filter(|(_, uncommitted_hunk)| {
            let a = &uncommitted_hunk.hunk_assignment;
            match &filter {
                None => true,
                Some(Filter::Unassigned) => a.stack_id.is_none(),
                Some(Filter::Uncommitted(id)) => {
                    if id.is_entire_file {
                        a.path_bytes == id.hunk_assignments.first().path_bytes
                    } else {
                        a.eq(id.hunk_assignments.first())
                    }
                }
                Some(Filter::Stack(stack_id)) => a.stack_id.as_ref() == Some(stack_id),
            }
        })
        .map(|(short_id, uncommitted_hunk)| (short_id.as_str(), &uncommitted_hunk.hunk_assignment))
        .collect();
    short_id_assignment_pairs.sort_by(|(_, a_assignment), (_, b_assignment)| {
        a_assignment
            .stack_id
            .cmp(&b_assignment.stack_id)
            .then_with(|| {
                a_assignment
                    .path_bytes
                    .cmp(&b_assignment.path_bytes)
                    .then_with(|| a_assignment.hunk_header.cmp(&b_assignment.hunk_header))
            })
    });

    if short_id_assignment_pairs.is_empty() {
        writeln!(out, "No diffs to show.")?;
    } else {
        for (short_id, assignment) in short_id_assignment_pairs {
            write!(out, "{}", assignment.print_diff(Some(short_id)))?;
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
