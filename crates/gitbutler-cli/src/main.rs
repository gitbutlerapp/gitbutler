use std::path::PathBuf;

use anyhow::{Context, Result, bail};

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
            }) => command::project::add(
                data_dir(app_suffix, app_data_dir)?,
                path,
                switch_to_workspace,
            ),
            None => command::project::list(),
        },
    }
}
pub fn data_dir(
    app_suffix: Option<String>,
    app_data_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let path = if let Some(dir) = app_data_dir {
        std::fs::create_dir_all(&dir).context("Failed to assure the designated data-dir exists")?;
        dir
    } else {
        dirs_next::data_dir()
            .map(|dir| {
                dir.join(format!(
                    "com.gitbutler.app{}",
                    app_suffix
                        .map(|mut suffix| {
                            suffix.insert(0, '.');
                            suffix
                        })
                        .unwrap_or_default()
                ))
            })
            .context("no data-directory available on this platform")?
    };
    if !path.is_dir() {
        bail!("Path '{}' must be a valid directory", path.display());
    }
    Ok(path)
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

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
