use std::path::PathBuf;

use anyhow::{Context as _, Result};
use but_ctx::Context;
use gitbutler_project::Project;
use gitbutler_reference::RemoteRefname;

use crate::command::debug_print;

pub fn list() -> Result<()> {
    for project in gitbutler_project::dangerously_list_projects_without_migration()? {
        println!(
            "{id} {name} {path}",
            id = project.id,
            name = project.title,
            path = project.worktree_dir()?.display()
        );
    }
    Ok(())
}

pub fn add(data_dir: PathBuf, path: PathBuf, refname: Option<RemoteRefname>) -> Result<()> {
    let path = gix::discover(path)?
        .workdir()
        .context("Only non-bare repositories can be added")?
        .to_owned()
        .canonicalize()?;
    let outcome = gitbutler_project::add_with_path(data_dir, path)?;
    let project = outcome.try_project()?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    if let Some(refname) = refname {
        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &refname,
            ctx.exclusive_worktree_access().write_permission(),
        )?;
    };
    debug_print(project)
}

pub fn switch_to_workspace(project: Project, refname: RemoteRefname) -> Result<()> {
    let ctx = Context::new_from_legacy_project(project.clone())?;
    debug_print(gitbutler_branch_actions::set_base_branch(
        &ctx,
        &refname,
        ctx.exclusive_worktree_access().write_permission(),
    )?)
}
