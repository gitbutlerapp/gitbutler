use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_ctx::Context;
use std::fmt::Write;

use crate::{id::UncommittedCliId, utils::OutputChannel};

use super::display::TreeChangeWithPatch;

pub(crate) enum Filter {
    Unassigned,
    Uncommitted(UncommittedCliId),
}

pub(crate) fn worktree(
    ctx: &mut Context,
    out: &mut OutputChannel,
    filter: Option<Filter>,
) -> anyhow::Result<()> {
    let result = but_api::legacy::diff::changes_in_worktree(ctx)?;
    writeln!(out, "{:?}", result)?;
    if let Some(filter) = filter {
        match filter {
            Filter::Unassigned => Ok(()),
            Filter::Uncommitted(_id) => Ok(()),
        }
    } else {
        Ok(())
    }
}

pub(crate) fn commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    id: gix::ObjectId,
    path: Option<BString>,
) -> anyhow::Result<()> {
    let result = but_api::diff::commit_details(ctx, id, ComputeLineStats::No)?;
    for change in result.diff_with_first_parent {
        if let Some(path) = &path {
            if &change.path != path {
                continue;
            }
        } else {
            let patch = but_api::legacy::diff::tree_change_diffs(ctx, change.clone().into())
                .ok()
                .flatten();
            writeln!(out, "{}", TreeChangeWithPatch::new(change.into(), patch))?;
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

        writeln!(out, "{}", TreeChangeWithPatch::new(change, patch))?;
    }
    Ok(())
}
