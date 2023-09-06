use std::process::ExitCode;

use clap::Parser;

mod app;
mod cli;
mod commands;

fn main() -> ExitCode {
    cli::Cli::parse().run()
}
