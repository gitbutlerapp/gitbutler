use std::path::PathBuf;

use anyhow::{Context, Result};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_reference::RemoteRefname;

use crate::command::debug_print;

pub fn add(data_dir: PathBuf, path: PathBuf, refname: Option<RemoteRefname>) -> Result<()> {
    let path = gix::discover(path)?
        .workdir()
        .context("Only non-bare repositories can be added")?
        .to_owned()
        .canonicalize()?;
    let project = gitbutler_project::add_with_path(data_dir, path)?;
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    if let Some(refname) = refname {
        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &refname,
            false,
            ctx.project().exclusive_worktree_access().write_permission(),
        )?;
    };
    debug_print(project)
}
