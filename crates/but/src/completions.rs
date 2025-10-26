use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::args::Args;

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: Shell) -> Result<()> {
    let mut cmd = Args::command();
    let bin_name = cmd.get_name().to_string();

    generate(shell, &mut cmd, bin_name, &mut io::stdout());

    Ok(())
}
