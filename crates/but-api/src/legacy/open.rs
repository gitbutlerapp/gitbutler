//! In place of commands.rs
use std::env;

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use tracing::instrument;
use url::Url;

/// Environment variable names that need cleaning when spawning subprocesses
/// on Linux to avoid AppImage path contamination.
const APPIMAGE_ENV_VARS: &[&str] = &[
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
];

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

/// Open a URL with the appropriate handler, after validating the scheme.
/// Only allows known-safe URL schemes. NOT suitable for filesystem paths.
pub(crate) fn open_that(url: &str) -> anyhow::Result<()> {
    let target_url = Url::parse(url).with_context(|| format!("Invalid URL format: '{url}'"))?;
    if ![
        "http",
        "https",
        "mailto",
        "vscode",
        "vscodium",
        "vscode-insiders",
        "zed",
        "windsurf",
        "cursor",
        "trae",
    ]
    .contains(&target_url.scheme())
    {
        bail!("Invalid URL scheme: {}", target_url.scheme());
    }

    open_with_cleaned_env(url)
}

/// Open a target (URL or filesystem path) using the system handler,
/// with AppImage-safe environment variables on Linux.
fn open_with_cleaned_env(target: &str) -> anyhow::Result<()> {
    let mut cmd_errors = Vec::new();

    for mut cmd in open::commands(target) {
        let cleaned_vars = clean_env_vars(APPIMAGE_ENV_VARS);

        cmd.envs(cleaned_vars);
        cmd.current_dir(env::temp_dir());
        if cmd.status().is_ok() {
            return Ok(());
        } else {
            cmd_errors.push(anyhow::anyhow!("Failed to execute command {:?}", cmd));
        }
    }
    if !cmd_errors.is_empty() {
        bail!("Errors occurred: {:?}", cmd_errors);
    }
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<()> {
    open_that(&url)
}

#[but_api]
#[instrument(err(Debug))]
pub fn show_in_finder(path: String) -> Result<()> {
    // Cross-platform implementation to open file/directory in the default file manager
    // macOS: Opens in Finder (with -R flag to reveal the item)
    // Windows: Opens in File Explorer
    // Linux: Opens in the default file manager

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to show '{path}' in Finder"))?;
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to show '{path}' in Explorer"))?;
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, open filesystem paths directly using the system handler
        // (xdg-open / gio open). Do NOT use open_that() here — that function
        // validates URL schemes and rejects plain filesystem paths.
        if std::path::Path::new(&path).is_dir() {
            open_with_cleaned_env(&path)
                .with_context(|| format!("Failed to open directory '{path}' in file manager"))?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let parent_str = parent.to_string_lossy();
                open_with_cleaned_env(&parent_str)
                    .with_context(|| format!("Failed to open parent directory of '{path}' in file manager",))?;
            } else {
                open_with_cleaned_env(&path)
                    .with_context(|| format!("Failed to open '{path}' in file manager"))?;
            }
        }
    }

    Ok(())
}
