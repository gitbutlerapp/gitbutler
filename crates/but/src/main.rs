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
        args::Subcommands::McpInternal => mcp_internal::start(&args.current_dir).await,
        args::Subcommands::Mcp => mcp::start(&args.current_dir).await,
        args::Subcommands::HandleChanges { context, simple } => {
            command::handle_changes(&args.current_dir, args.json, *simple, context)
        }
    }
}
