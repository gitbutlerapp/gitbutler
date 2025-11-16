use crate::utils::OutputChannel;
use but_core::ref_metadata::StackId;
use but_oxidize::ObjectIdExt;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gix::ObjectId;

pub(crate) fn commit(
    ctx: &mut CommandContext,
    oid: &ObjectId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    gitbutler_branch_actions::undo_commit(ctx, stack_id_by_commit_id(ctx, oid)?, oid.to_git2())?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Uncommitted {}", oid.to_string()[..7].blue())?;
    }
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
