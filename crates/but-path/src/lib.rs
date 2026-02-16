//! A version of `tauri::AppHandle::path()` for use outside `tauri`.
use std::{env, path::PathBuf};

use anyhow::{Context, bail};

/// The directory to store application-wide data in, like logs, **one per channel**.
///
/// > ⚠️Keep in sync with `tauri::AppHandle::path().app_data_dir().`
pub fn app_data_dir() -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("com.gitbutler.app"));
    }
    dirs::data_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join(identifier()))
}

/// The directory to store logs in, **one per channel**.
///
/// > ⚠️Keep in sync with `tauri::AppHandle::path().app_log_dir().`
///
/// # Platform-specific locations
///
/// - **macOS**: `~/Library/Logs/<identifier()>`
/// - **Linux/Windows/other**: `<data_local_dir>/<identifier()>/logs`
///
/// # Testing behavior
///
/// When the `E2E_TEST_APP_DATA_DIR` environment variable is set (used by E2E tests),
/// this function returns `<E2E_TEST_APP_DATA_DIR>/logs` instead of the platform-specific
/// default directories above.
pub fn app_log_dir() -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("logs"));
    }
    if cfg!(target_os = "macos") {
        dirs::home_dir()
            .with_context(|| "Couldn't resolve home directory")
            .map(|dir| dir.join("Library/Logs").join(identifier()))
    } else {
        dirs::data_local_dir()
            .with_context(|| "Couldn't resolve local data directory")
            .map(|dir| dir.join(identifier()).join("logs"))
    }
}

/// The directory to store application-wide settings in, **shared for all channels**.
///
/// > ⚠️Keep in sync with `tauri::AppHandle::path().app_config_dir().`
pub fn app_config_dir() -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("gitbutler"));
    }
    dirs::config_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join("gitbutler"))
}

/// Returns the platform-specific cache directory for GitButler, **one per channel**.
///
/// > ⚠️Keep in sync with `tauri::AppHandle::path().app_cache_dir().`
///
/// The cache directory is used for non-essential data that can be regenerated
/// or re-downloaded, such as update check metadata. Unlike data stored in
/// [`app_data_dir`], cached data:
///
/// - Should not be backed up by the system
/// - Can be safely deleted to free up disk space
/// - Has no user-visible impact if cleared
///
/// # Platform-specific locations
///
/// - **macOS**: `~/Library/Caches/com.gitbutler.app{channel}/`
/// - **Linux**: `~/.cache/com.gitbutler.app{channel}/` (following XDG Base Directory Specification)
/// - **Windows**: `%LOCALAPPDATA%\com.gitbutler.app{channel}\`
///
/// # Testing
///
/// When the `E2E_TEST_APP_DATA_DIR` environment variable is set, returns
/// `{E2E_TEST_APP_DATA_DIR}/cache` to isolate test environments.
///
/// # Errors
///
/// Returns an error if the platform's cache directory cannot be determined.
pub fn app_cache_dir() -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("cache"));
    }
    dirs::cache_dir()
        .ok_or(anyhow::anyhow!("Could not get app cache dir"))
        .map(|dir| dir.join(identifier()))
}

pub fn identifier() -> &'static str {
    option_env!("IDENTIFIER").unwrap_or_else(|| {
        if let Some(channel) = option_env!("CHANNEL") {
            match channel {
                "nightly" => "com.gitbutler.app.nightly",
                "release" => "com.gitbutler.app",
                _ => "com.gitbutler.app.dev",
            }
        } else {
            "com.gitbutler.app.dev"
        }
    })
}

/// A way to learn about the currently configured compile-time app-channel.
#[derive(Debug)]
pub enum AppChannel {
    /// This is a nightly build.
    Nightly,
    /// This is a release build.
    Release,
    /// The fallback if nothing is specified: developer mode.
    Dev,
}

impl Default for AppChannel {
    fn default() -> Self {
        AppChannel::new()
    }
}

impl AppChannel {
    pub fn new() -> Self {
        match identifier() {
            "com.gitbutler.app.nightly" => AppChannel::Nightly,
            "com.gitbutler.app" => AppChannel::Release,
            _ => AppChannel::Dev,
        }
    }

    /// Open the GitButler GUI application for `possibly_project_dir`.
    ///
    /// This uses the deeplink URL scheme registered for the specific channel.
    pub fn open(&self, possibly_project_dir: &std::path::Path) -> anyhow::Result<()> {
        let scheme = match self {
            AppChannel::Nightly => "but-nightly",
            AppChannel::Release => "but",
            AppChannel::Dev => "but-dev",
        };

        // Add a timestamp to avoid processing the same URL multiple times due to caching.
        // Read more https://github.com/gitbutlerapp/gitbutler/pull/11234#issuecomment-3533202481
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let url = format!(
            "{}://open?path={}&t={}",
            scheme,
            possibly_project_dir.display(),
            timestamp
        );

        let mut cmd_errors = Vec::new();

        for mut cmd in open::commands(url) {
            let cleaned_vars = clean_env_vars(&[
                "APPDIR",
                "GDK_PIXBUF_MODULE_FILE",
                "GIO_EXTRA_MODULES",
                "GIO_EXTRA_MODULES",
                "GSETTINGS_SCHEMA_DIR",
                "GST_PLUGIN_SYSTEM_PATH",
                "GST_PLUGIN_SYSTEM_PATH_1_0",
                "GTK_DATA_PREFIX",
                "GTK_EXE_PREFIX",
                "GTK_IM_MODULE_FILE",
                "GTK_PATH",
                "LD_LIBRARY_PATH",
                "PATH",
                "PERLLIB",
                "PYTHONHOME",
                "PYTHONPATH",
                "QT_PLUGIN_PATH",
                "XDG_DATA_DIRS",
            ]);

            cmd.envs(cleaned_vars);
            cmd.current_dir(env::temp_dir());
            if cmd.status().is_ok() {
                return Ok(());
            } else {
                cmd_errors.push(anyhow::anyhow!("Failed to execute command {cmd:?}"));
            }
        }
        if !cmd_errors.is_empty() {
            bail!("Errors occurred: {cmd_errors:?}");
        }
        Ok(())
    }
}

fn clean_env_vars<'a, 'b>(var_names: &'a [&'b str]) -> impl Iterator<Item = (&'b str, String)> + 'a {
    var_names
        .iter()
        .filter_map(|name| env::var(name).map(|value| (*name, value)).ok())
        .map(|(name, value)| {
            (
                name,
                value
                    .split(':')
                    .filter(|path| !path.contains("appimage-run") && !path.contains("/tmp/.mount"))
                    .collect::<Vec<_>>()
                    .join(":"),
            )
        })
}
