use std::path::Path;

use anyhow::{Context as _, anyhow};
use but_api::json::Error;
use but_path::AppChannel;
use tracing::instrument;

/// Opens the logs folder in the system file manager
#[instrument(err(Debug))]
pub fn open_logs_folder() -> Result<(), Error> {
    open_existing(but_path::app_log_dir()?)
}

/// Opens the config/settings folder in the system file manager
#[instrument(err(Debug))]
pub fn open_config_folder() -> Result<(), Error> {
    open_existing(but_path::app_config_dir()?)
}

/// Opens the cache folder in the system file manager
#[instrument(err(Debug))]
pub fn open_cache_folder() -> Result<(), Error> {
    open_existing(but_path::app_cache_dir()?)
}

/// Open `dir` but refuse to do so if that would definitely fail as it's not a directory,
/// or it doesn't exist.
///
/// We can assume the directories exist.
fn open_existing(dir: impl AsRef<Path>) -> Result<(), Error> {
    let dir = dir.as_ref();
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

    let is_macos_stable_build =
        cfg!(target_os = "macos") && matches!(AppChannel::new(), AppChannel::Release);
    // On macOS stable builds, it would try to open `com.gitbutler.app` and treat it as application,
    // which would fail. Instead, we reveal, which selects the directory in the finder and users
    // can right-click it to see the package contents. Better than nothing.
    // Maybe we can rename the application ID at some point.
    if is_macos_stable_build {
        opener::reveal(dir).map_err(anyhow::Error::from)
    } else {
        open::that(dir).map_err(anyhow::Error::from)
    }
    .with_context(|| format!("Failed to open directory at '{}'", dir.display()))
    .map_err(Into::into)
}
