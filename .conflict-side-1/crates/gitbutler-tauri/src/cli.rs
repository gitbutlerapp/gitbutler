use but_api::{commands::cli, NoParams};
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn install_cli() -> Result<(), Error> {
    cli::install_cli(NoParams {})
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn cli_path() -> Result<String, Error> {
    cli::cli_path(NoParams {})
}
