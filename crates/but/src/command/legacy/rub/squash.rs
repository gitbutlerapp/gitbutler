use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use colored::Colorize;
use gix::ObjectId;

use super::undo::stack_id_by_commit_id;
use crate::utils::OutputChannel;

pub(crate) fn commits(
    ctx: &mut Context,
    source: &ObjectId,
    destination: &ObjectId,
    custom_message: Option<&str>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let source_stack = stack_id_by_commit_id(ctx, source)?;
    let destination_stack = stack_id_by_commit_id(ctx, destination)?;
    if source_stack != destination_stack {
        anyhow::bail!("Cannot squash commits from different stacks");
    }

    let new_commit_oid = gitbutler_branch_actions::squash_commits(
        ctx,
        source_stack,
        vec![source.to_git2()],
        destination.to_git2(),
    )?;

    // If a custom message is provided, reword the resulting commit
    let final_commit_oid = if let Some(message) = custom_message {
        gitbutler_branch_actions::update_commit_message(ctx, source_stack, new_commit_oid, message)?
    } else {
        new_commit_oid
    };

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Squashed {} → {}",
            source.to_string()[..7].blue(),
            final_commit_oid.to_gix().to_string()[..7].blue()
        )?
    }
    Ok(())
}
