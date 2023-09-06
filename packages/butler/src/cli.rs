use std::process::ExitCode;

use crate::commands::{self, RunCommand};

use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Branches(commands::Branches),
    Clear(commands::Clear),
    Commit(commands::Commit),
    Flush(commands::Flush),
    Info(commands::Info),
    Move(commands::Move),
    New(commands::New),
    Remotes(commands::Remotes),
    Reset(commands::Reset),
    Setup(commands::Setup),
    Status(commands::Status),
}

impl Cli {
    pub fn run(self) -> ExitCode {
        let output = match self.command {
            Command::Branches(branches) => branches.run(),
            Command::Clear(clear) => clear.run(),
            Command::Commit(commit) => commit.run(),
            Command::Flush(flush) => flush.run(),
            Command::Info(info) => info.run(),
            Command::Move(mv) => mv.run(),
            Command::New(new) => new.run(),
            Command::Remotes(remotes) => remotes.run(),
            Command::Reset(reset) => reset.run(),
            Command::Setup(setup) => setup.run(),
            Command::Status(status) => status.run(),
        };

        match output {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}: {:#}", "error".red(), e);
                ExitCode::FAILURE
            }
        }
    }
}
