use but_api::commands::cli;

use but_api::error::Error;

#[tauri::command(async)]
pub fn install_cli() -> Result<(), Error> {
    cli::install_cli()
}

#[tauri::command(async)]
pub fn cli_path() -> Result<String, Error> {
    cli::cli_path()
}
