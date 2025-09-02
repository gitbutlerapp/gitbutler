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
    // Validate both commits exist in stacks before proceeding
    let source_stack = stack_id_by_commit_id(ctx, source)
        .map_err(|e| anyhow::anyhow!("Source commit {}: {}", &source.to_string()[..7], e))?;
    let destination_stack = stack_id_by_commit_id(ctx, destination).map_err(|e| {
        anyhow::anyhow!(
            "Destination commit {}: {}",
            &destination.to_string()[..7],
            e
        )
    })?;
    if source_stack != destination_stack {
        anyhow::bail!("Cannot squash commits from different stacks");
    }

    gitbutler_branch_actions::squash_commits(
        ctx,
        source_stack,
        vec![source.to_git2()],
        destination.to_git2(),
    )?;
    println!(
        "Squashed {} → {}",
        source.to_string()[..7].blue(),
        destination.to_string()[..7].blue()
    );
    Ok(())
}
