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

    open_folder_in_file_manager(logs_dir.to_string_lossy().to_string())?;
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

    open_folder_in_file_manager(config_dir.to_string_lossy().to_string())?;
    Ok(())
}

/// Cross-platform implementation to open a folder in the default file manager
fn open_folder_in_file_manager(path: String) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to open '{}' in Finder", path))?;
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to open '{}' in Explorer", path))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Use xdg-open on Linux
        use std::process::Command;
        Command::new("xdg-open")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to open '{}' in file manager", path))?;
    }

    Ok(())
}
