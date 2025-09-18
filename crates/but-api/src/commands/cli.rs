//! In place of commands.rs

use but_action::cli::{do_install_cli, get_cli_path};
use but_api_macros::api_cmd;
use tracing::instrument;

use crate::error::Error;

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn install_cli() -> Result<(), Error> {
    do_install_cli().map_err(Error::from)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String, Error> {
    let cli_path = get_cli_path()?;
    if !cli_path.exists() {
        return Err(anyhow::anyhow!(
            "CLI path does not exist: {}",
            cli_path.display()
        ))
        .map_err(|e| e.into());
    }
    Ok(cli_path.to_string_lossy().to_string())
}
