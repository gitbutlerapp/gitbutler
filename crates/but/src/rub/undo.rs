use but_workspace::StackId;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gix::ObjectId;

pub(crate) fn commit(ctx: &mut CommandContext, oid: &ObjectId) -> anyhow::Result<()> {
    gitbutler_branch_actions::undo_commit(ctx, stack_id_by_commit_id(ctx, oid)?, oid.to_git2())?;
    Ok(())
}

fn stack_id_by_commit_id(ctx: &CommandContext, oid: &ObjectId) -> anyhow::Result<StackId> {
    let stacks = crate::log::stacks(ctx)?
        .iter()
        .map(|s| crate::log::stack_details(ctx, s.id).map(|d| (s.id, d)))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if let Some((id, _)) = stacks.iter().find(|(_, stack)| {
        stack
            .branch_details
            .iter()
            .any(|branch| branch.commits.iter().any(|commit| commit.id == *oid))
    }) {
        println!("Uncommitted {}", oid.to_string()[..7].blue());
        return Ok(*id);
    }
    anyhow::bail!("No stack found for commit {}", oid.to_string())
}
