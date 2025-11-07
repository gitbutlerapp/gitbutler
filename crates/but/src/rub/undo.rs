use std::io::Write;

use but_workspace::StackId;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gix::ObjectId;

pub(crate) fn commit(ctx: &mut CommandContext, oid: &ObjectId) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    gitbutler_branch_actions::undo_commit(ctx, stack_id_by_commit_id(ctx, oid)?, oid.to_git2())?;
    writeln!(stdout, "Uncommitted {}", oid.to_string()[..7].blue()).ok();
    Ok(())
}

pub(crate) fn stack_id_by_commit_id(
    ctx: &CommandContext,
    oid: &ObjectId,
) -> anyhow::Result<StackId> {
    let stacks = crate::log::stacks(ctx)?
        .iter()
        .filter_map(|s| {
            s.id.map(|id| crate::log::stack_details(ctx, id).map(|d| (id, d)))
        })
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if let Some((id, _)) = stacks.iter().find(|(_, stack)| {
        stack
            .branch_details
            .iter()
            .any(|branch| branch.commits.iter().any(|commit| commit.id == *oid))
    }) {
        return Ok(*id);
    }
    anyhow::bail!("No stack found for commit {}", oid)
}
