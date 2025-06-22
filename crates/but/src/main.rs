use anyhow::{Context, Ok, Result};

mod args;
use args::{Args, Subcommands, actions};
use but_settings::AppSettings;

use crate::args::Inspect;
mod command;
mod id;
mod log;
mod mcp;
mod mcp_internal;
mod metrics;
mod rub;
mod status;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();
    let app_settings = AppSettings::load_from_default_path_creating()?;

    if args.trace {
        trace::init()?;
    }

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    gitbutler_secret::secret::set_application_namespace(namespace);

    match &args.cmd {
        Subcommands::Mcp { internal } => {
            if *internal {
                mcp_internal::start().await
            } else {
                mcp::start(app_settings).await
            }
        }
        Subcommands::Actions(actions::Platform { cmd }) => match cmd {
            Some(actions::Subcommands::HandleChanges {
                description,
                handler,
            }) => {
                let handler = *handler;
                command::handle_changes(&args.current_dir, args.json, handler, description)
            }
            None => command::list_actions(&args.current_dir, args.json, 0, 10),
        },
        Subcommands::Log => log::commit_graph(&args.current_dir, args.json),
        Subcommands::Status => status::worktree(&args.current_dir, args.json),
        Subcommands::Rub { source, target } => {
            let result = rub::handle(&args.current_dir, args.json, source, target)
                .context("Rubbed the wrong way.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            Ok(())
        }
        args::Subcommands::BetaInspect(Inspect { cmd }) => match cmd {
            args::InspectSubcommands::Status => {
                command::inspect::status(&args.current_dir, args.json)
            }
            args::InspectSubcommands::Generate => {
                command::inspect::generate(&args.current_dir, args.json)
            }
        },
    }
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing::subscriber::set_global_default;
    use tracing_subscriber::Layer;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::layer::SubscriberExt;

    pub fn init() -> anyhow::Result<()> {
        let format_for_humans = tracing_subscriber::fmt::format()
            .with_file(true)
            .with_line_number(true)
            .with_target(false)
            .compact();

        let subscriber = tracing_subscriber::registry();

        set_global_default(
            subscriber.with(
                // subscriber that writes spans to stdout
                tracing_subscriber::fmt::layer()
                    .event_format(format_for_humans)
                    .with_ansi(true)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(LevelFilter::TRACE),
            ),
        )?;
        Ok(())
    }
}
