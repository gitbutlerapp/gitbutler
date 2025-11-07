use std::io::Write;

use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gix::ObjectId;

use super::undo::stack_id_by_commit_id;

pub(crate) fn commits(
    ctx: &mut CommandContext,
    source: &ObjectId,
    destination: &ObjectId,
) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
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
    writeln!(
        stdout,
        "Squashed {} → {}",
        source.to_string()[..7].blue(),
        destination.to_string()[..7].blue()
    )
    .ok();
    Ok(())
}
