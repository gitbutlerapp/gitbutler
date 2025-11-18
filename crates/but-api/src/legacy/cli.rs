//! In place of commands.rs

use anyhow::Result;
use but_action::cli::{InstallMode, do_install_cli, get_cli_path};
use but_api_macros::api_cmd_tauri;
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn install_cli() -> Result<()> {
    do_install_cli(InstallMode::AllowPrivilegeElevation)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String> {
    let cli_path = get_cli_path()?;
    Ok(cli_path.to_string_lossy().to_string())
}
