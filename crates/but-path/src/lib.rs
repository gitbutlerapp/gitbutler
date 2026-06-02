//! A version of `tauri::AppHandle::path()` for use outside `tauri`.
//!
//! # E2E test override
//!
//! When the `E2E_TEST_APP_DATA_DIR` environment varaiable is set,
//! these helpers stop using the platform-specific GitButler locations
//! and instead use fixed subdirectories underneath that directory:
//!
//! - app data: `<E2E_TEST_APP_DATA_DIR>/com.gitbutler.app`
//! - logs: `<E2E_TEST_APP_DATA_DIR>/logs`
//! - config: `<E2E_TEST_APP_DATA_DIR>/gitbutler`
//! - cache: `<E2E_TEST_APP_DATA_DIR>/cache`
//!
//! This override intentionally ignores the compile-time or explicit [`AppChannel`]. Even
//! `*_for_channel(...)` helpers return the same E2E path for all channels so tests do not
//! accidentally spread state across channel-specific directories.
#[cfg(target_os = "linux")]
use std::process::Command;
use std::{env, path::PathBuf};

use anyhow::Context;

/// The directory to store application-wide data in, like logs, **one per channel**.
///
/// > ⚠️Keep in sync with `tauri::AppHandle::path().app_data_dir().`
///
/// When `E2E_TEST_APP_DATA_DIR` is set, returns
/// `<E2E_TEST_APP_DATA_DIR>/com.gitbutler.app` for every channel.
pub fn app_data_dir() -> anyhow::Result<PathBuf> {
    app_data_dir_for_channel(AppChannel::new())
}

/// Like [`app_data_dir()`], but explicitly for `channel`.
///
/// When `E2E_TEST_APP_DATA_DIR` is set, `channel` is ignored and the result is always
/// `<E2E_TEST_APP_DATA_DIR>/com.gitbutler.app`.
pub fn app_data_dir_for_channel(channel: AppChannel) -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("com.gitbutler.app"));
    }
    dirs::data_dir()
        .ok_or(anyhow::anyhow!("Could not get app data dir"))
        .map(|dir| dir.join(identifier_for_channel(channel)))
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
/// default directories above. This override always ignores the compile-time channel.
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
///
/// When `E2E_TEST_APP_DATA_DIR` is set, returns `<E2E_TEST_APP_DATA_DIR>/gitbutler`.
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
/// `{E2E_TEST_APP_DATA_DIR}/cache` to isolate test environments. This override ignores
/// the compile-time or explicit channel.
///
/// # Errors
///
/// Returns an error if the platform's cache directory cannot be determined.
pub fn app_cache_dir() -> anyhow::Result<PathBuf> {
    app_cache_dir_for_channel(AppChannel::new())
}

/// Like [`app_cache_dir()`], but explicitly for `channel`.
///
/// When `E2E_TEST_APP_DATA_DIR` is set, `channel` is ignored and the result is always
/// `<E2E_TEST_APP_DATA_DIR>/cache`.
pub fn app_cache_dir_for_channel(channel: AppChannel) -> anyhow::Result<PathBuf> {
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Ok(PathBuf::from(test_dir).join("cache"));
    }
    dirs::cache_dir()
        .ok_or(anyhow::anyhow!("Could not get app cache dir"))
        .map(|dir| dir.join(identifier_for_channel(channel)))
}

/// Returns the bundle identifier for the compile-time [`AppChannel`].
pub fn identifier() -> &'static str {
    identifier_for_channel(AppChannel::new())
}

/// Returns the bundle identifier used for `channel`.
pub const fn identifier_for_channel(channel: AppChannel) -> &'static str {
    match channel {
        AppChannel::Nightly => "com.gitbutler.app.nightly",
        AppChannel::Release => "com.gitbutler.app",
        AppChannel::Dev => "com.gitbutler.app.dev",
    }
}

/// A way to learn about the currently configured compile-time app-channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// Returns the app channel configured at compile time.
    ///
    /// Resolution prefers the `IDENTIFIER` environment variable embedded at build
    /// time. If it is unset, `CHANNEL` is mapped to the corresponding bundle
    /// identifier, with any missing or unknown value falling back to
    /// [`AppChannel::Dev`].
    ///
    /// Only the known nightly and release identifiers map to their dedicated
    /// variants; any other identifier resolves to [`AppChannel::Dev`].
    pub fn new() -> Self {
        match option_env!("IDENTIFIER").unwrap_or_else(|| {
            if let Some(channel) = option_env!("CHANNEL") {
                match channel {
                    "nightly" => identifier_for_channel(AppChannel::Nightly),
                    "release" => identifier_for_channel(AppChannel::Release),
                    _ => identifier_for_channel(AppChannel::Dev),
                }
            } else {
                identifier_for_channel(AppChannel::Dev)
            }
        }) {
            "com.gitbutler.app.nightly" => AppChannel::Nightly,
            "com.gitbutler.app" => AppChannel::Release,
            _ => AppChannel::Dev,
        }
    }

    /// Open the GitButler GUI application for `possibly_project_dir`.
    ///
    /// This uses the deeplink URL scheme registered for the specific channel.
    ///
    /// Note: On Linux, we don't have a good way of distinguishing between channels in the installed
    /// binaries, so we always just resolve `gitbutler-tauri`.
    pub fn open(
        &self,
        possibly_project_dir: &std::path::Path,
        new_window: bool,
    ) -> anyhow::Result<()> {
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
            "{}://open?path={}&t={}&new_window={}",
            scheme,
            possibly_project_dir.display(),
            timestamp,
            if new_window { 1 } else { 0 }
        );

        let cleaned_vars: Vec<(&str, String)> = clean_env_vars(&[
            "APPDIR",
            "GDK_PIXBUF_MODULE_FILE",
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
        ])
        .collect();

        #[cfg(target_os = "linux")]
        {
            // On Linux, we don't currently want to rely on the scheme being properly registered
            // with the Tauri app. The mechanism by which the scheme is registered relies on the
            // bundled Desktop entry, and different desktop environments have pretty wildly
            // different handling of such entries.
            //
            // Adding insult to injury, even if that desktop entry is resolved, it in turn just has
            // an exec line with the name `gitbutler-tauri` and the URL is provided as a command
            // line argument. Therefore, after the roundabout trip to the desktop entry via the
            // custom scheme, we _still_ just resolve `gitbutler-tauri` from PATH.
            //
            // Even more annoying is that the desktop entry does not have a placeholder for the
            // command line argument, causing some stricter environments such as KDE to just error
            // out, while some more lenient environments simply append the URL to the exec line.
            //
            // For these reasons, it's way more reliable and simpler to just try to call
            // `gitbutler-tauri` directly, completely circumventing any issues with scheme
            // registration.
            //
            // As the binary is always called `gitbutler-tauri`, there's currently no way to
            // distinguish between release, nightly and dev. We'll just have to try to launch
            // whatever we find. This can be fixed by giving the binaries different names, but as
            // so few users use nightly builds, it's just not worth the effort.
            let mut cmd = Command::new("gitbutler-tauri");
            cmd.arg(&url);
            cmd.current_dir(env::temp_dir());
            cmd.envs(cleaned_vars.clone());

            // Unset all io to not pollute the terminal with output.
            cmd.stdin(std::process::Stdio::null());
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            // We spawn this fire-and-forget style. The process will be re-parented to init when
            // the caller exits (and that caller is typically the `but` CLI). This allows you to
            // e.g. run `but gui` in a terminal, and then keep using that terminal.
            //
            // This is only necessary on cold start, i.e. when the GUI isn't already running, as
            // then this process becomes the GUI process. If the GUI is already running, this
            // process effectively just sends the deep link to the already running GUI and then
            // exits.
            cmd.spawn()?;
        };

        #[cfg(not(target_os = "linux"))]
        {
            let mut cmd_errors = Vec::new();
            for mut cmd in open::commands(&url) {
                cmd.envs(cleaned_vars.clone());
                cmd.current_dir(env::temp_dir());
                if cmd.status().is_ok() {
                    return Ok(());
                } else {
                    cmd_errors.push(anyhow::anyhow!("Failed to execute command {cmd:?}"));
                }
            }
            if !cmd_errors.is_empty() {
                anyhow::bail!("Errors occurred: {cmd_errors:?}");
            }
        }
        Ok(())
    }
}

impl std::str::FromStr for AppChannel {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "nightly" => Ok(AppChannel::Nightly),
            "release" | "production" | "prod" => Ok(AppChannel::Release),
            "dev" | "development" => Ok(AppChannel::Dev),
            _ => Err(anyhow::anyhow!(
                "Unknown app suffix '{value}', expected one of: dev, nightly, release"
            )),
        }
    }
}

fn clean_env_vars<'a, 'b>(
    var_names: &'a [&'b str],
) -> impl Iterator<Item = (&'b str, String)> + 'a {
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
