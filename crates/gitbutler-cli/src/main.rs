use anyhow::{Context, Result};
use std::path::PathBuf;

use gitbutler_project::Project;

mod args;
use crate::args::snapshot;
use args::Args;

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    let project = project_from_path(args.current_dir)?;
    match args.cmd {
        args::Subcommands::Snapshot(snapshot::Platform { cmd }) => match cmd {
            Some(snapshot::SubCommands::Restore { snapshot_id }) => {
                command::snapshot::restore(project, snapshot_id)
            }
            None => command::snapshot::list(project),
        },
    }
}

fn project_from_path(path: PathBuf) -> Result<Project> {
    let worktree_dir = gix::discover(path)?
        .work_dir()
        .context("Bare repositories aren't supported")?
        .to_owned();
    Ok(Project {
        path: worktree_dir,
        ..Default::default()
    })
}
