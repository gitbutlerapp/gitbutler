use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};

mod args;
use args::Args;
use but_ctx::Context;

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
        args::Subcommands::Branch(vbranch::Platform { cmd }) => {
            let mut ctx = Context::discover(args.current_dir)?;
            match cmd {
                Some(vbranch::SubCommands::Apply { name, branch }) => {
                    command::vbranch::apply(&mut ctx, name, branch)
                }
                Some(vbranch::SubCommands::Commit { message, name }) => {
                    command::vbranch::commit(&mut ctx, name, message)
                }
                Some(vbranch::SubCommands::Series { name, series_name }) => {
                    command::vbranch::series(&ctx, name, series_name)
                }
                Some(vbranch::SubCommands::Create { name, .. }) => {
                    command::vbranch::create(&mut ctx, name)
                }
                None => command::vbranch::list(&ctx),
            }
        }
        args::Subcommands::Project(project::Platform {
            app_data_dir,
            app_suffix,
            cmd,
        }) => match cmd {
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
