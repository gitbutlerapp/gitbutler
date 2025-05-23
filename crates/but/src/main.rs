use anyhow::Result;

mod args;
use args::Args;
mod mcp;

fn main() -> Result<()> {
    let args: Args = clap::Parser::parse();

    match &args.cmd {
        args::Subcommands::Mcp => mcp::start(),
    }
}
