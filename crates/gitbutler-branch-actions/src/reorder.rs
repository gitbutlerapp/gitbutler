use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::StackId;

use serde::Serialize;

use crate::VirtualBranchesExt;

pub fn reorder_stack(
    ctx: &CommandContext,
    branch_id: StackId,
    stack_order: StackOrder,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let state = ctx.project().virtual_branches();
    let stack = state.get_branch(branch_id)?;
    let repo = ctx.repository();
    Ok(())
}

/// Represents the order of series (branches) and changes (commits) in a stack.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackOrder {
    /// The series are ordered from newest to oldest (most recent stacks go first)
    series: Vec<SeriesOrder>,
}
/// Represents the order of changes (commits) in a series (branch).
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
pub struct SeriesOrder {
    /// Unique name of the series (branch). Must already exist in the stack.
    name: String,
    /// The changes are ordered from newest to oldest (most recent changes go first)
    /// The change ids must refer to commits that already exist in the stack.
    change_ids: Vec<String>,
}

impl SeriesOrder {
    /// Ensures that:
    /// - The series exists in the stack
    /// - The change ids refer to commits that already exist in the stack
    fn validate(&self, repo: &git2::Repository) -> Result<()> {
        Ok(())
    }
}
