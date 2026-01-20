use anyhow::{Context as _, anyhow};
use but_api::json::Error;
use tauri::{AppHandle, Manager, Runtime};
use tracing::instrument;

/// Opens the logs folder in the system file manager
#[instrument(skip(handle), err(Debug))]
pub fn open_logs_folder<R: Runtime>(handle: &AppHandle<R>) -> Result<(), Error> {
    let dir = handle
        .path()
        .app_log_dir()
        .context("Failed to get app log directory")?;
    open_existing(&dir)
}

/// Opens the config/settings folder in the system file manager
#[instrument(skip(handle), err(Debug))]
pub fn open_config_folder<R: Runtime>(handle: &AppHandle<R>) -> Result<(), Error> {
    let dir = handle
        .path()
        .app_config_dir()
        .context("Failed to get app config directory")?;
    open_existing(&dir)
}

/// Opens the cache folder in the system file manager
#[instrument(err(Debug))]
pub fn open_cache_folder() -> Result<(), Error> {
    let dir = but_path::app_cache_dir()?;
    open_existing(&dir)
}

/// Open `dir` but refuse to do so if that would definitely fail as it's not a directory,
/// or it doesn't exist.
///
/// We can assume the directories exist.
fn open_existing(dir: &std::path::Path) -> Result<(), Error> {
    if !dir.exists() {
        return Err(anyhow!(
            "Cannot attempt to open non-existing directory: '{}'",
            dir.display()
        )
        .into());
    }
    if !dir.is_dir() {
        return Err(anyhow!(
            "Cannot attempt to open anything but a directory: '{}'",
            dir.display()
        )
        .into());
    }
    Ok(open::that(dir)
        .with_context(|| format!("Failed to open directory at '{}'", dir.display()))?)
}
