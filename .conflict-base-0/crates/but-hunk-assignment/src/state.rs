/// The name of the file holding our state, useful for watching for changes.
use anyhow::Result;
use gitbutler_command_context::CommandContext;

use crate::HunkAssignment;

pub fn assignments(ctx: &mut CommandContext) -> Result<Vec<HunkAssignment>> {
    let assignments = ctx
        .db()?
        .hunk_assignments()
        .list_all()?
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<HunkAssignment>>>()?;
    Ok(assignments)
}

pub fn set_assignments(ctx: &mut CommandContext, assignments: Vec<HunkAssignment>) -> Result<()> {
    let assignments: Vec<but_db::HunkAssignment> = assignments
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<but_db::HunkAssignment>>>(
    )?;
    ctx.db()?.hunk_assignments().set_all(assignments)
}
