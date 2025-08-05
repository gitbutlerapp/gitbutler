//! In place of commands.rs
use anyhow::{Context, bail};
use serde::Deserialize;
use std::env;
use url::Url;

use crate::{App, error::Error};

pub(crate) fn open_that(path: &str) -> anyhow::Result<()> {
    let target_url = Url::parse(path).with_context(|| format!("Invalid path format: '{path}'"))?;
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
    ]
    .contains(&target_url.scheme())
    {
        bail!("Invalid path scheme: {}", target_url.scheme());
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
                        .filter(|path| {
                            !path.contains("appimage-run") && !path.contains("/tmp/.mount")
                        })
                        .collect::<Vec<_>>()
                        .join(":"),
                )
            })
    }

    let mut cmd_errors = Vec::new();

    for mut cmd in open::commands(path) {
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
            cmd_errors.push(anyhow::anyhow!("Failed to execute command {:?}", cmd));
        }
    }
    if !cmd_errors.is_empty() {
        bail!("Errors occurred: {:?}", cmd_errors);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenUrlParams {
    pub url: String,
}

pub fn open_url(_app: &App, params: OpenUrlParams) -> Result<(), Error> {
    Ok(open_that(&params.url)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowInFinderParams {
    pub path: String,
}

pub fn show_in_finder(_app: &App, params: ShowInFinderParams) -> Result<(), Error> {
    // Cross-platform implementation to open file/directory in the default file manager
    // macOS: Opens in Finder (with -R flag to reveal the item)
    // Windows: Opens in File Explorer
    // Linux: Opens in the default file manager

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("-R")
            .arg(&params.path)
            .status()
            .with_context(|| format!("Failed to show '{}' in Finder", params.path))?;
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg("/select,")
            .arg(&params.path)
            .status()
            .with_context(|| format!("Failed to show '{}' in Explorer", params.path))?;
    }

    #[cfg(target_os = "linux")]
    {
        // For directories, open the directory directly
        if std::path::Path::new(&params.path).is_dir() {
            open_that(&params.path).with_context(|| {
                format!("Failed to open directory '{}' in file manager", params.path)
            })?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&params.path).parent() {
                let parent_str = parent.to_string_lossy();
                open_that(&parent_str).with_context(|| {
                    format!(
                        "Failed to open parent directory of '{}' in file manager",
                        params.path
                    )
                })?;
            } else {
                open_that(&params.path)
                    .with_context(|| format!("Failed to open '{}' in file manager", params.path))?;
            }
        }
    }

    Ok(())
}

#[derive(Deserialize)]
pub struct OpenInTerminalParams {
    pub app_name: String,
    pub path: String,
}

pub fn open_in_terminal(_app: &App, params: OpenInTerminalParams) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("-a")
            .arg(&params.app_name)
            .arg(&params.path)
            .status()
            .with_context(|| format!("Failed to show '{}' in Finder", params.path))?;
    }

    Ok(())
}
