use but_api::{commands::cli, App, NoParams};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn install_cli(app: State<'_, App>) -> Result<(), Error> {
    cli::install_cli(&app, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn cli_path(app: State<'_, App>) -> Result<String, Error> {
    cli::cli_path(&app, NoParams {})
}
