use anyhow::Result;

mod args;
use args::Args;

use crate::args::{project, vbranch};

mod command;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();
    gitbutler_project::configure_git2();

    if args.trace {
        trace::init()?;
    }
    let _op_span = tracing::info_span!("cli-op").entered();

    match args.cmd {
        args::Subcommands::IntegrateUpstream { mode } => {
            let project = command::prepare::project_from_path(args.current_dir)?;
            command::workspace::update(project, mode)
        }
        args::Subcommands::UnapplyOwnership {
            filepath,
            from_line,
            to_line,
        } => {
            let project = command::prepare::project_from_path(args.current_dir)?;
            command::ownership::unapply(project, filepath, from_line, to_line)
        }
        args::Subcommands::Branch(vbranch::Platform { cmd }) => {
            let project = command::prepare::project_from_path(args.current_dir)?;
            match cmd {
                Some(vbranch::SubCommands::ListCommitFiles { commit_id }) => {
                    command::vbranch::list_commit_files(project, commit_id)
                }
                Some(vbranch::SubCommands::SetBase {
                    short_tracking_branch_name,
                }) => command::vbranch::set_base(project, short_tracking_branch_name),
                Some(vbranch::SubCommands::List) => command::vbranch::list_all(project),
                Some(vbranch::SubCommands::Status) => command::vbranch::status(project),
                Some(vbranch::SubCommands::Unapply { name }) => {
                    command::vbranch::unapply(project, name)
                }
                Some(vbranch::SubCommands::Apply { name, branch }) => {
                    command::vbranch::apply(project, name, branch)
                }
                Some(vbranch::SubCommands::SetDefault { name }) => {
                    command::vbranch::set_default(project, name)
                }
                Some(vbranch::SubCommands::Commit { message, name }) => {
                    command::vbranch::commit(project, name, message)
                }
                Some(vbranch::SubCommands::Series { name, series_name }) => {
                    command::vbranch::series(project, name, series_name)
                }
                Some(vbranch::SubCommands::Create { set_default, name }) => {
                    command::vbranch::create(project, name, set_default)
                }
                Some(vbranch::SubCommands::Details { names }) => {
                    command::vbranch::details(project, names)
                }
                Some(vbranch::SubCommands::ListAll) => command::vbranch::list_all(project),
                None => command::vbranch::list(project),
            }
        }
        args::Subcommands::Project(project::Platform {
            app_data_dir,
            app_suffix,
            cmd,
        }) => match cmd {
            Some(project::SubCommands::SwitchToWorkspace { remote_ref_name }) => {
                let project = command::prepare::project_from_path(args.current_dir)?;
                command::project::switch_to_workspace(project, remote_ref_name)
            }
            Some(project::SubCommands::Add {
                switch_to_workspace,
                path,
            }) => {
                let ctrl = command::prepare::project_controller(app_suffix, app_data_dir)?;
                command::project::add(ctrl, path, switch_to_workspace)
            }
            None => {
                let ctrl = command::prepare::project_controller(app_suffix, app_data_dir)?;
                command::project::list(ctrl)
            }
        },
    }
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    pub fn init() -> anyhow::Result<()> {
        tracing_subscriber::registry()
            .with(
                tracing_forest::ForestLayer::from(
                    tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                )
                .with_filter(LevelFilter::DEBUG),
            )
            .init();
        Ok(())
    }
}
