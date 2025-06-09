use anyhow::{Context, Ok, Result};

mod args;
use args::{Args, Subcommands, actions};
mod command;
mod id;
mod log;
mod mcp;
mod mcp_internal;
mod rub;
mod status;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    match &args.cmd {
        Subcommands::Mcp { internal } => {
            if *internal {
                mcp_internal::start(&args.current_dir).await
            } else {
                mcp::start().await
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
    }
}
