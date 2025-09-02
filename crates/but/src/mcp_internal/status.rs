use std::path::Path;

use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

pub fn project_status(project_dir: &Path) -> anyhow::Result<but_tools::workspace::ProjectStatus> {
    let repo = crate::mcp_internal::project::project_repo(project_dir)?;

    let project = super::project::project_from_path(project_dir)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    but_tools::workspace::get_project_status(&mut ctx, &repo, None)
}
