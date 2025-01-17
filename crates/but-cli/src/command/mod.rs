use gitbutler_project::Project;
use std::path::PathBuf;

pub fn project_from_path(path: PathBuf) -> anyhow::Result<Project> {
    Project::from_path(&path)
}

pub fn project_repo(path: PathBuf) -> anyhow::Result<gix::Repository> {
    let project = project_from_path(path)?;
    Ok(gix::open(project.worktree_path())?)
}

fn debug_print(this: impl std::fmt::Debug) -> anyhow::Result<()> {
    println!("{:#?}", this);
    Ok(())
}

pub mod status {
    use crate::command::{debug_print, project_repo};
    use std::path::PathBuf;

    pub fn doit(current_dir: PathBuf) -> anyhow::Result<()> {
        debug_print(but_core::worktree::changes(&project_repo(current_dir)?)?)
    }
}

pub mod stacks {
    use std::path::PathBuf;

    use crate::command::{debug_print, project_from_path};

    pub fn list(current_dir: PathBuf) -> anyhow::Result<()> {
        let project = project_from_path(current_dir)?;
        debug_print(but_workspace::stacks(&project.gb_dir()))
    }
}
