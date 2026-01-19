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

/// Opens a terminal application at the specified directory path.
///
/// # Parameters
/// - `terminal_id`: Identifier for the terminal application to open. Use `"auto"` to select
///   the platform default (Terminal.app on macOS, PowerShell on Windows, GNOME Terminal on Linux).
/// - `path`: The directory path where the terminal should open.
///
/// # Supported Terminals
///
/// **All Platforms:**
/// - `auto` - Platform default terminal
///
/// **macOS:**
/// - `terminal` - Terminal.app
/// - `iterm2` - iTerm2
/// - `ghostty` - Ghostty
/// - `warp` - Warp
/// - `alacritty-mac` - Alacritty
/// - `wezterm-mac` - WezTerm
/// - `hyper` - Hyper
///
/// **Windows:**
/// - `wt` - Windows Terminal
/// - `powershell` - PowerShell
/// - `cmd` - Command Prompt
///
/// **Linux:**
/// - `gnome-terminal` - GNOME Terminal
/// - `konsole` - KDE Konsole
/// - `xfce4-terminal` - XFCE Terminal
/// - `alacritty-linux` - Alacritty
/// - `wezterm-linux` - WezTerm
///
/// # Errors
/// Returns an error if:
/// - The terminal application is not installed or not found in PATH
/// - The specified path does not exist or is not accessible
/// - The terminal_id is not recognized for the current platform
/// - The terminal process exits with a non-zero status code
#[but_api]
#[instrument(err(Debug))]
pub fn open_in_terminal(terminal_id: String, path: String) -> Result<()> {
    use std::process::Command;

    // Handle 'auto' by selecting the platform default terminal
    let terminal_id = if terminal_id == "auto" {
        #[cfg(target_os = "macos")]
        {
            "terminal".to_string()
        }
        #[cfg(target_os = "windows")]
        {
            "powershell".to_string()
        }
        #[cfg(target_os = "linux")]
        {
            "gnome-terminal".to_string()
        }
    } else {
        terminal_id
    };

    /// Helper to run a command and check its exit status
    fn run_terminal_command(mut cmd: Command, terminal_name: &str, path: &str) -> Result<()> {
        let status = cmd
            .status()
            .with_context(|| format!("Failed to launch {terminal_name} at '{path}'"))?;

        if !status.success() {
            bail!(
                "{terminal_name} exited with non-zero status: {}",
                status
                    .code()
                    .map_or("unknown".to_string(), |c| c.to_string())
            );
        }
        Ok(())
    }

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
        let mut cmd = Command::new("open");
        cmd.arg("-a").arg(app_name).arg(&path);
        run_terminal_command(cmd, app_name, &path)?;
    }

    #[cfg(target_os = "windows")]
    {
        match terminal_id.as_str() {
            "wt" => {
                let mut cmd = Command::new("wt");
                cmd.arg("-d").arg(&path);
                run_terminal_command(cmd, "Windows Terminal", &path)?;
            }
            "powershell" => {
                let mut cmd = Command::new("powershell");
                // Escape single quotes by doubling them for PowerShell
                let escaped_path = path.replace('\'', "''");
                cmd.arg("-NoExit")
                    .arg("-Command")
                    .arg(format!("cd '{}'", escaped_path));
                run_terminal_command(cmd, "PowerShell", &path)?;
            }
            "cmd" => {
                let mut cmd = Command::new("cmd");
                // Escape double quotes by doubling them for CMD
                let escaped_path = path.replace('"', "\"\"");
                cmd.arg("/K").arg(format!("cd /d \"{}\"", escaped_path));
                run_terminal_command(cmd, "Command Prompt", &path)?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    #[cfg(target_os = "linux")]
    {
        match terminal_id.as_str() {
            "gnome-terminal" => {
                let mut cmd = Command::new("gnome-terminal");
                cmd.arg("--working-directory").arg(&path);
                run_terminal_command(cmd, "GNOME Terminal", &path)?;
            }
            "konsole" => {
                let mut cmd = Command::new("konsole");
                cmd.arg("--workdir").arg(&path);
                run_terminal_command(cmd, "Konsole", &path)?;
            }
            "xfce4-terminal" => {
                let mut cmd = Command::new("xfce4-terminal");
                cmd.arg("--working-directory").arg(&path);
                run_terminal_command(cmd, "XFCE Terminal", &path)?;
            }
            "alacritty-linux" => {
                let mut cmd = Command::new("alacritty");
                cmd.arg("--working-directory").arg(&path);
                run_terminal_command(cmd, "Alacritty", &path)?;
            }
            "wezterm-linux" => {
                let mut cmd = Command::new("wezterm");
                cmd.arg("start").arg("--cwd").arg(&path);
                run_terminal_command(cmd, "WezTerm", &path)?;
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
