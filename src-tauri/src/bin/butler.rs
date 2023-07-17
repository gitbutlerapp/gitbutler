use std::process::ExitCode;

use clap::Parser;

mod cli;

fn main() -> ExitCode {
    cli::Butler::parse().run()
}
