use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use colored::Colorize;
use gix::ObjectId;

use super::undo::stack_id_by_commit_id;
use crate::utils::OutputChannel;

pub(crate) fn commits(
    ctx: &mut Context,
    source: &ObjectId,
    destination: &ObjectId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let source_stack = stack_id_by_commit_id(ctx, source)?;
    let destination_stack = stack_id_by_commit_id(ctx, destination)?;
    if source_stack != destination_stack {
        anyhow::bail!("Cannot squash commits from different stacks");
    }

    gitbutler_branch_actions::squash_commits(
        ctx,
        source_stack,
        vec![source.to_git2()],
        destination.to_git2(),
    )?;
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Squashed {} → {}",
            source.to_string()[..7].blue(),
            destination.to_string()[..7].blue()
        )?
    }
    Ok(())
}
