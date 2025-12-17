use std::path::Path;

use but_ctx::Context;

use crate::command::legacy::mcp_internal::project;

pub fn project_status(project_dir: &Path) -> anyhow::Result<but_tools::workspace::ProjectStatus> {
    let repo = project::project_repo(project_dir)?;

    let project = project::project_from_path(project_dir)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;

    but_tools::workspace::get_project_status(&mut ctx, &repo, None)
}
