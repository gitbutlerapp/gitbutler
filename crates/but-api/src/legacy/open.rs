//! In place of commands.rs
use std::env;

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use tracing::instrument;
use url::Url;

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
        "trae",
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

#[but_api]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<()> {
    open_that(&url)
}

#[but_api]
#[instrument(err(Debug))]
pub fn open_in_terminal(terminal_id: String, path: String) -> Result<()> {
    use std::process::Command;

    #[cfg(target_os = "macos")]
    {
        let app_name = match terminal_id.as_str() {
            "terminal" => "Terminal",
            "iterm2" => "iTerm",
            "ghostty" => "Ghostty",
            "warp" => "Warp",
            "alacritty-mac" => "Alacritty",
            "wezterm-mac" => "WezTerm",
            "hyper" => "Hyper",
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
        Command::new("open")
            .arg("-a")
            .arg(app_name)
            .arg(&path)
            .status()
            .with_context(|| format!("Failed to open terminal '{app_name}' at '{path}'"))?;
    }

    #[cfg(target_os = "windows")]
    {
        match terminal_id.as_str() {
            "wt" => {
                Command::new("wt")
                    .arg("-d")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open Windows Terminal at '{path}'"))?;
            }
            "powershell" => {
                Command::new("powershell")
                    .arg("-NoExit")
                    .arg("-Command")
                    .arg(format!("cd '{}'", path))
                    .status()
                    .with_context(|| format!("Failed to open PowerShell at '{path}'"))?;
            }
            "cmd" => {
                Command::new("cmd")
                    .arg("/K")
                    .arg(format!("cd /d \"{}\"", path))
                    .status()
                    .with_context(|| format!("Failed to open Command Prompt at '{path}'"))?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    #[cfg(target_os = "linux")]
    {
        match terminal_id.as_str() {
            "gnome-terminal" => {
                Command::new("gnome-terminal")
                    .arg("--working-directory")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open GNOME Terminal at '{path}'"))?;
            }
            "konsole" => {
                Command::new("konsole")
                    .arg("--workdir")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open Konsole at '{path}'"))?;
            }
            "xfce4-terminal" => {
                Command::new("xfce4-terminal")
                    .arg("--working-directory")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open XFCE Terminal at '{path}'"))?;
            }
            "alacritty-linux" => {
                Command::new("alacritty")
                    .arg("--working-directory")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open Alacritty at '{path}'"))?;
            }
            "wezterm-linux" => {
                Command::new("wezterm")
                    .arg("start")
                    .arg("--cwd")
                    .arg(&path)
                    .status()
                    .with_context(|| format!("Failed to open WezTerm at '{path}'"))?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    Ok(())
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
        // For directories, open the directory directly
        if std::path::Path::new(&path).is_dir() {
            open_that(&path)
                .with_context(|| format!("Failed to open directory '{path}' in file manager"))?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let parent_str = parent.to_string_lossy();
                open_that(&parent_str).with_context(|| {
                    format!("Failed to open parent directory of '{path}' in file manager",)
                })?;
            } else {
                open_that(&path)
                    .with_context(|| format!("Failed to open '{path}' in file manager"))?;
            }
        }
    }

    Ok(())
}
