use anyhow::Result;
use gitbutler_core::projects::Project;

use clap::{arg, Command};
#[cfg(not(windows))]
use pager::Pager;

fn cli() -> Command {
    Command::new("gitbutler-cli")
        .about("A CLI tool for GitButler")
        .arg(arg!(-C <path> "Run as if gitbutler-cli was started in <path> instead of the current working directory."))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("snapshot")
                .about("List and restore snapshots.")
                .subcommand(Command::new("restore")
                .about("Restores the state of the working direcory as well as virtual branches to a given snapshot.")
                .arg(arg!(<SNAPSHOT_ID> "The snapshot to restore"))),
        )
}

fn main() -> Result<()> {
    #[cfg(not(windows))]
    Pager::new().setup();
    let matches = cli().get_matches();

    let cwd = std::env::current_dir()?.to_string_lossy().to_string();
    let repo_dir = matches.get_one::<String>("path").unwrap_or(&cwd);

    match matches.subcommand() {
        Some(("snapshot", sub_matches)) => match sub_matches.subcommand() {
            Some(("restore", sub_matches)) => {
                let snapshot_id = sub_matches
                    .get_one::<String>("SNAPSHOT_ID")
                    .expect("required");
                restore_snapshot(repo_dir, snapshot_id)?;
            }
            _ => {
                list_snapshots(repo_dir)?;
            }
        },
        _ => unreachable!(),
    }

    Ok(())
}

fn list_snapshots(repo_dir: &str) -> Result<()> {
    let project = project_from_path(repo_dir);
    let snapshots = project.list_snapshots(100, None)?;
    for snapshot in snapshots {
        let ts = chrono::DateTime::from_timestamp(snapshot.created_at.seconds(), 0);
        let details = snapshot.details;
        if let (Some(ts), Some(details)) = (ts, details) {
            println!("{} {} {}", ts, snapshot.commit_id, details.operation);
        }
    }
    Ok(())
}

fn restore_snapshot(repo_dir: &str, snapshot_id: &str) -> Result<()> {
    let project = project_from_path(repo_dir);
    project.restore_snapshot(snapshot_id.parse()?)?;
    Ok(())
}

fn project_from_path(repo_dir: &str) -> Project {
    Project {
        path: std::path::PathBuf::from(repo_dir),
        ..Default::default()
    }
}
