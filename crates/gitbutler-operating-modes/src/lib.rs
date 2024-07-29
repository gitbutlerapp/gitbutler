use anyhow::{bail, Context, Result};
use gitbutler_command_context::CommandContext;

/// Operating Modes:
/// Gitbutler currently has two main operating modes:
/// - `in workspace mode`: When the app is on the gitbutler/integration branch.
///     This is when normal operations can be performed.
/// - `outside workspace mode`: When the user has left the gitbutler/integration
///     branch to perform regular git commands.

const INTEGRATION_BRANCH_REF: &str = "refs/heads/gitbutler/integration";

pub fn in_open_workspace_mode(ctx: &CommandContext) -> Result<bool> {
    let head_ref = ctx.repository().head().context("failed to get head")?;
    let head_ref_name = head_ref.name().context("failed to get head name")?;

    Ok(head_ref_name == INTEGRATION_BRANCH_REF)
}

pub fn assure_open_workspace_mode(ctx: &CommandContext) -> Result<()> {
    if in_open_workspace_mode(ctx)? {
        Ok(())
    } else {
        bail!("Unexpected state: cannot perform operation on non-integration branch")
    }
}

pub fn in_outside_workspace_mode(ctx: &CommandContext) -> Result<bool> {
    in_open_workspace_mode(ctx).map(|open_mode| !open_mode)
}

pub fn assure_outside_workspace_mode(ctx: &CommandContext) -> Result<()> {
    if in_open_workspace_mode(ctx)? {
        Ok(())
    } else {
        bail!("Unexpected state: cannot perform operation on non-integration branch")
    }
}
