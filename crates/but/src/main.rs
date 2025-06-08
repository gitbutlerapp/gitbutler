use anyhow::Result;

mod args;
use args::Args;
mod command;
mod mcp;
mod mcp_internal;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    match &args.cmd {
        args::Subcommands::Mcp { internal } => {
            if *internal {
                mcp_internal::start(&args.current_dir).await
            } else {
                mcp::start().await
            }
        }
        args::Subcommands::HandleChanges {
            change_description,
            simple,
        } => command::handle_changes(&args.current_dir, args.json, *simple, change_description),
        args::Subcommands::ListActions { offset, limit } => {
            command::list_actions(&args.current_dir, args.json, *offset, *limit)
        }
    }
}
