use anyhow::Result;

mod args;
use args::Args;

use crate::args::{project, snapshot, vbranch};

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    match args.cmd {
        args::Subcommands::Branch(vbranch::Platform { cmd }) => {
            let project = command::prepare::project_from_path(args.current_dir)?;
            match cmd {
                Some(vbranch::SubCommands::SetDefault { name }) => {
                    command::vbranch::set_default(project, name)
                }
                Some(vbranch::SubCommands::Commit { message, name }) => {
                    command::vbranch::commit(project, name, message)
                }
                Some(vbranch::SubCommands::Create { name }) => {
                    command::vbranch::create(project, name)
                }
                None => command::vbranch::list(project),
            }
        }
        args::Subcommands::Project(project::Platform {
            app_data_dir,
            app_suffix,
            cmd,
        }) => match cmd {
            Some(project::SubCommands::SwitchToIntegration { remote_ref_name }) => {
                let project = command::prepare::project_from_path(args.current_dir)?;
                command::project::switch_to_integration(project, remote_ref_name)
            }
            Some(project::SubCommands::Add {
                switch_to_integration,
                path,
            }) => {
                let ctrl = command::prepare::project_controller(app_suffix, app_data_dir)?;
                command::project::add(ctrl, path, switch_to_integration)
            }
            None => {
                let ctrl = command::prepare::project_controller(app_suffix, app_data_dir)?;
                command::project::list(ctrl)
            }
        },
        args::Subcommands::Snapshot(snapshot::Platform { cmd }) => {
            let project = command::prepare::project_from_path(args.current_dir)?;
            match cmd {
                Some(snapshot::SubCommands::Restore { snapshot_id }) => {
                    command::snapshot::restore(project, snapshot_id)
                }
                None => command::snapshot::list(project),
            }
        }
    }
}
