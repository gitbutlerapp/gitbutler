use anyhow::Context as _;
use but_api::json::Error;
use tauri::{AppHandle, Manager, Runtime};
use tracing::instrument;

/// Opens the logs folder in the system file manager
#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn open_logs_folder<R: Runtime>(handle: AppHandle<R>) -> Result<(), Error> {
    let paths = handle.path();
    let logs_dir = paths
        .app_log_dir()
        .context("Failed to get app log directory")?;

    // Ensure the directory exists
    std::fs::create_dir_all(&logs_dir).context("Failed to create logs directory")?;

    open::that(&logs_dir).context("Failed to open logs directory")?;
    Ok(())
}

/// Opens the config/settings folder in the system file manager
#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn open_config_folder<R: Runtime>(handle: AppHandle<R>) -> Result<(), Error> {
    let paths = handle.path();
    let config_dir = paths
        .app_config_dir()
        .context("Failed to get app config directory")?;

    // Ensure the directory exists
    std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

    open::that(&config_dir).context("Failed to open config directory")?;
    Ok(())
}
