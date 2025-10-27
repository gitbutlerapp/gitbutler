use std::io;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::Shell;

use crate::args::Args;

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: Option<Shell>) -> Result<()> {
    let shell = shell.or_else(Shell::from_env).context(
        "Couldn't extract shell from `SHELL` environment variable - please specify it manually",
    )?;
    let mut cmd = Args::command();
    let bin_name = cmd.get_name().to_string();

    clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());

    Ok(())
}
