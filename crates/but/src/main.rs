use anyhow::Result;

mod args;
use args::Args;
mod mcp_internal;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    match &args.cmd {
        args::Subcommands::McpInternal => mcp_internal::start().await,
    }
}
