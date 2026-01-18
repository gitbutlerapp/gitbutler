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
/// - `terminal_id`: Identifier for the terminal application to open.
/// - `path`: The directory path where the terminal should open.
///
/// # Supported Terminals
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
/// - `alacritty` - Alacritty
/// - `ghostty` - Ghostty
/// - `warp` - Warp
/// - `hyper` - Hyper
/// - `wezterm` - WezTerm
///
/// # Errors
/// Returns an error if:
/// - The terminal application is not installed or not found in PATH
/// - The specified path does not exist or is not accessible
/// - The terminal_id is not recognized for the current platform
/// - The terminal fails to launch
///   - On macOS/Linux, this includes the terminal process exiting immediately with a non-zero status code
///   - On Windows, only spawn failures are detected; the terminal's later exit status is not checked
#[but_api]
#[instrument(err(Debug))]
pub fn open_in_terminal(terminal_id: String, path: String) -> Result<()> {
    use std::process::Command;

    /// Helper to run a command and check its exit status
    /// Used for macOS and Linux terminals that are launched via `open` or direct commands.
    /// These typically return immediately (async launch), so we only check if the launch succeeded.
    fn run_terminal_command(mut cmd: Command, terminal_name: &str, path: &str) -> Result<()> {
        tracing::info!(?cmd, "terminal command");
        let output = cmd
            .output()
            .with_context(|| format!("Failed to launch {terminal_name} at '{path}'"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if stderr.is_empty() {
                bail!(
                    "{terminal_name} exited with non-zero status: {}",
                    output.status.code().map_or("unknown".to_string(), |c| c.to_string())
                );
            } else {
                bail!("Failed to open {terminal_name}: {stderr}");
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        /// Check if a macOS application is installed using `open -Ra`.
        fn ensure_app_installed(app_name: &str) -> Result<()> {
            let status = std::process::Command::new("open")
                .arg("-Ra")
                .arg(app_name)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .context("Failed to run 'open -Ra' to check application availability")?;
            if !status.success() {
                bail!("{app_name} was not found in Applications folder");
            }
            Ok(())
        }

        match terminal_id.as_str() {
            // These terminals support `open -a <app> <path>` as folder handlers
            "terminal" => {
                ensure_app_installed("Terminal", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-a").arg("Terminal").arg(&path);
                run_terminal_command(cmd, "Terminal", &path)?;
            }
            "iterm2" => {
                ensure_app_installed("iTerm", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-a").arg("iTerm").arg(&path);
                run_terminal_command(cmd, "iTerm2", &path)?;
            }
            "warp" => {
                ensure_app_installed("Warp", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-a").arg("Warp").arg(&path);
                run_terminal_command(cmd, "Warp", &path)?;
            }
            "ghostty" => {
                ensure_app_installed("Ghostty", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-a").arg("Ghostty").arg(&path);
                run_terminal_command(cmd, "Ghostty", &path)?;
            }
            "alacritty-mac" => {
                ensure_app_installed("Alacritty", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-n")
                    .arg("-a")
                    .arg("Alacritty")
                    .arg("--args")
                    .arg("--working-directory")
                    .arg(&path);
                run_terminal_command(cmd, "Alacritty", &path)?;
            }
            // WezTerm does not support `open -a WezTerm <path>`. Their docs state you have to use their CLI.
            // https://wezterm.org/config/launch.html#specifying-the-current-working-directory
            "wezterm-mac" => {
                ensure_app_installed("WezTerm", &terminal_id, &path)?;
                let mut cmd = Command::new("wezterm");
                cmd.arg("start").arg("--cwd").arg(&path);
                run_terminal_command(cmd, "WezTerm", &path)?;
            }
            "hyper" => {
                ensure_app_installed("Hyper", &terminal_id, &path)?;
                let mut cmd = Command::new("open");
                cmd.arg("-a").arg("Hyper").arg(&path);
                run_terminal_command(cmd, "Hyper", &path)?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::path::Path;

        // Validate path exists and canonicalize it to proper Windows format
        let path_buf = Path::new(&path);
        if !path_buf.exists() {
            bail!("Path does not exist: {}", path);
        }
        if !path_buf.is_dir() {
            bail!("Path is not a directory: {}", path);
        }

        // Canonicalize to get the absolute, properly formatted Windows path
        // This converts forward slashes to backslashes and resolves . and ..
        let canonical_path = path_buf
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", path))?;

        // Strip the \\?\ prefix that canonicalize adds on Windows
        // CMD.exe and some terminals don't support UNC paths with this prefix
        let path_str = canonical_path.to_str().context("Path contains invalid UTF-8")?;
        let cleaned_path = path_str.strip_prefix(r"\\?\").unwrap_or(path_str);

        // CREATE_NEW_CONSOLE: Creates a new console for the process (0x00000010)
        // This allows the terminal to run independently without blocking our thread
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;

        match terminal_id.as_str() {
            "wt" => {
                let mut cmd = Command::new("wt");
                cmd.arg("-d").arg(cleaned_path);
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                // Windows Terminal detaches automatically, just check if it launches
                cmd.spawn()
                    .with_context(|| format!("Failed to launch Windows Terminal at '{}'", cleaned_path))?;
            }
            "powershell" => {
                let mut cmd = Command::new("powershell");
                // Set the working directory directly instead of using cd command
                cmd.current_dir(cleaned_path);
                cmd.arg("-NoExit"); // Keep the window open
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn()
                    .with_context(|| format!("Failed to launch PowerShell at '{}'", cleaned_path))?;
            }
            "cmd" => {
                let mut cmd = Command::new("cmd");
                // Set the working directory directly - OS handles path format
                cmd.current_dir(cleaned_path);
                cmd.arg("/K"); // Keep the window open
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn()
                    .with_context(|| format!("Failed to launch Command Prompt at '{}'", cleaned_path))?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    #[cfg(target_os = "linux")]
    {
        // Most Linux terminal emulators follow a convention: they open in the current working
        // directory of the parent process. This means we can simply set current_dir() on the
        // Command instead of passing explicit --working-directory or --workdir flags.
        match terminal_id.as_str() {
            // Terminals that inherit parent process CWD (no explicit flags needed)
            terminal_cmd @ ("gnome-terminal" | "konsole" | "xfce4-terminal" | "alacritty" | "ghostty") => {
                let mut cmd = Command::new(terminal_cmd);
                cmd.current_dir(&path);
                run_terminal_command(cmd, terminal_cmd, &path)?;
            }
            // Warp uses a different binary name on Linux
            "warp" => {
                let mut cmd = Command::new("warp-terminal");
                cmd.current_dir(&path);
                run_terminal_command(cmd, "warp-terminal", &path)?;
            }
            // Hyper accepts path as argument (Electron app doesn't use parent CWD)
            "hyper" => {
                let mut cmd = Command::new("hyper");
                cmd.arg(&path);
                run_terminal_command(cmd, "hyper", &path)?;
            }
            // WezTerm is an exception - it does not respect parent CWD and requires explicit --cwd
            "wezterm" => {
                let mut cmd = Command::new("wezterm");
                cmd.arg("start").arg("--cwd").arg(&path);
                run_terminal_command(cmd, "wezterm", &path)?;
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
            open_that(&path).with_context(|| format!("Failed to open directory '{path}' in file manager"))?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let parent_str = parent.to_string_lossy();
                open_that(&parent_str)
                    .with_context(|| format!("Failed to open parent directory of '{path}' in file manager",))?;
            } else {
                open_that(&path).with_context(|| format!("Failed to open '{path}' in file manager"))?;
            }
        }
    }

    Ok(())
}
