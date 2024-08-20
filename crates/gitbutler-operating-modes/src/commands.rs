use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::OperatingMode;

pub fn operating_mode(project: &Project) -> Result<OperatingMode> {
    let ctx = CommandContext::open(project)?;
    Ok(crate::operating_mode(&ctx))
}
