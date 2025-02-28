use std::path::PathBuf;

use anyhow::{Context, Result};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_reference::RemoteRefname;

use crate::command::debug_print;

pub fn list(ctrl: gitbutler_project::Controller) -> Result<()> {
    for project in ctrl.list()? {
        println!(
            "{id} {name} {path}",
            id = project.id,
            name = project.title,
            path = project.path.display()
        );
    }
    Ok(())
}

pub fn add(
    ctrl: gitbutler_project::Controller,
    path: PathBuf,
    refname: Option<RemoteRefname>,
) -> Result<()> {
    let path = gix::discover(path)?
        .work_dir()
        .context("Only non-bare repositories can be added")?
        .to_owned()
        .canonicalize()?;
    let project = ctrl.add(path)?;
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    if let Some(refname) = refname {
        gitbutler_branch_actions::set_base_branch(&ctx, &refname)?;
    };
    debug_print(project)
}

pub fn switch_to_workspace(project: Project, refname: RemoteRefname) -> Result<()> {
    let ctx = CommandContext::open(&project, AppSettings::default())?;
    debug_print(gitbutler_branch_actions::set_base_branch(&ctx, &refname)?)
}
