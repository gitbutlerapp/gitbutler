//! In place of commands.rs

use anyhow::Result;
use but_api_macros::but_api;
use but_skill::cli_link::{InstallMode, do_install_cli, get_cli_path};
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn install_cli() -> Result<()> {
    do_install_cli(InstallMode::AllowPrivilegeElevation)
}

#[but_api]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String> {
    let cli_path = get_cli_path()?;
    Ok(cli_path.to_string_lossy().to_string())
}
