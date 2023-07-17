use std::process::ExitCode;

use clap::Parser;

mod commands;
mod app;
mod cli;

fn main() -> ExitCode {
    cli::Cli::parse().run()
}
