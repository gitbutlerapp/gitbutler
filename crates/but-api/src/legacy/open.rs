//! In place of commands.rs
use std::env;

#[allow(unused_imports)]
use anyhow::anyhow;
use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use tracing::instrument;
use url::Url;

pub(crate) fn open_that(target_url: &Url) -> anyhow::Result<()> {
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
        "file",
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

    let commands = if target_url.scheme() == "file" {
        open::commands(
            target_url
                .to_file_path()
                .ok()
                .with_context(|| format!("Couldn't turn {target_url} into a file path"))?,
        )
    } else {
        open::commands(target_url.as_str())
    };

    for mut cmd in commands {
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

#[but_api]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<()> {
    let url = Url::parse(&url).with_context(|| format!("Invalid path format: '{url}'"))?;
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

    #[cfg(target_os = "macos")]
    {
        /// Helper to run a command and check its exit status
        /// Used for macOS terminals that are launched via `open` or direct commands.
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
        /// Check if a macOS application is installed using `open -Ra`.
        fn ensure_app_installed(app_name: &str, terminal_id: &str, path: &str) -> Result<()> {
            let status = std::process::Command::new("open")
                .arg("-Ra")
                .arg(app_name)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .context("Failed to run 'open -Ra' to check application availability")?;
            if !status.success() {
                return Err(anyhow::anyhow!(
                    "command: open_in_terminal\n\
                     params: terminal=\"{terminal_id}\", path=\"{path}\"\n\n\
                     '{app_name}' was not found."
                )
                .context(but_error::Code::DefaultTerminalNotFound));
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
                let cli_found = Command::new("which")
                    .arg("wezterm")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !cli_found {
                    return Err(anyhow::anyhow!(
                        "command: open_in_terminal\n\
                         params: terminal=\"{terminal_id}\", path=\"{path}\"\n\n\
                         'wezterm' CLI was not found on PATH."
                    )
                    .context(but_error::Code::DefaultTerminalNotFound));
                }
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

        // Check if the terminal binary exists in PATH before attempting to launch.
        let binary_found = Command::new("where")
            .arg(&terminal_id)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !binary_found {
            return Err(anyhow::anyhow!(
                "command: open_in_terminal\n\
                 params: terminal=\"{terminal_id}\", path=\"{cleaned_path}\"\n\n\
                 '{terminal_id}' was not found."
            )
            .context(but_error::Code::DefaultTerminalNotFound));
        }

        // CREATE_NEW_CONSOLE: Creates a new console for the process (0x00000010)
        // This allows the terminal to run independently without blocking our thread
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;

        match terminal_id.as_str() {
            "wt" => {
                let mut cmd = Command::new("wt");
                cmd.arg("-d").arg(cleaned_path);
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                // Windows Terminal detaches automatically, just check if it launches
                cmd.spawn().with_context(|| {
                    format!(
                        "command: open_in_terminal\n\
                         params: terminal=\"{terminal_id}\", path=\"{cleaned_path}\"\n\n\
                         Failed to launch Windows Terminal at '{cleaned_path}'"
                    )
                })?;
            }
            "powershell" => {
                let mut cmd = Command::new("powershell");
                // Set the working directory directly instead of using cd command
                cmd.current_dir(cleaned_path);
                cmd.arg("-NoExit"); // Keep the window open
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn().with_context(|| {
                    format!(
                        "command: open_in_terminal\n\
                         params: terminal=\"{terminal_id}\", path=\"{cleaned_path}\"\n\n\
                         Failed to launch PowerShell at '{cleaned_path}'"
                    )
                })?;
            }
            "cmd" => {
                let mut cmd = Command::new("cmd");
                // Set the working directory directly - OS handles path format
                cmd.current_dir(cleaned_path);
                cmd.arg("/K"); // Keep the window open
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn().with_context(|| {
                    format!(
                        "command: open_in_terminal\n\
                         params: terminal=\"{terminal_id}\", path=\"{cleaned_path}\"\n\n\
                         Failed to launch Command Prompt at '{cleaned_path}'"
                    )
                })?;
            }
            _ => bail!("Unknown terminal: {}", terminal_id),
        };
    }

    #[cfg(target_os = "linux")]
    {
        // Resolve the actual binary name (some terminals use a different binary than their ID)
        let binary = match terminal_id.as_str() {
            "warp" => "warp-terminal",
            other => other,
        };

        // Check if the terminal binary exists in PATH before attempting to launch.
        // This lets us give a clear error directing users to Settings, rather than
        // a vague launch failure (which could be confused with path issues).
        let binary_found = Command::new("which")
            .arg(binary)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !binary_found {
            return Err(anyhow::anyhow!(
                "command: open_in_terminal\n\
                 params: terminal=\"{terminal_id}\", path=\"{path}\"\n\n\
                 '{binary}' was not found."
            )
            .context(but_error::Code::DefaultTerminalNotFound));
        }

        // Use spawn() (fire-and-forget) rather than output() for Linux terminals.
        // Most terminal emulators return immediately after spawning a window, so output()
        // doesn't provide meaningful post-launch diagnostics — only spawn failures matter.
        match terminal_id.as_str() {
            // Terminals that inherit parent process CWD (no explicit flags needed).
            // Note: `binary` is used instead of the terminal ID because some terminals
            // have a different binary name (e.g. "warp" launches "warp-terminal").
            "gnome-terminal" | "konsole" | "xfce4-terminal" | "alacritty" | "ghostty" | "warp" => {
                let mut cmd = Command::new(binary);
                cmd.current_dir(&path);
                cmd.spawn()
                    .with_context(|| format!("Failed to launch {binary} at '{path}'"))?;
            }
            // Hyper accepts path as argument (Electron app doesn't use parent CWD)
            "hyper" => {
                let mut cmd = Command::new("hyper");
                cmd.arg(&path);
                cmd.spawn()
                    .with_context(|| format!("Failed to launch hyper at '{path}'"))?;
            }
            // WezTerm does not respect parent CWD and requires explicit --cwd
            "wezterm" => {
                let mut cmd = Command::new("wezterm");
                cmd.arg("start").arg("--cwd").arg(&path);
                cmd.spawn()
                    .with_context(|| format!("Failed to launch wezterm at '{path}'"))?;
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
            open_that(&Url::from_file_path(&path).map_err(|_| anyhow!("Failed to parse URL"))?)
                .with_context(|| format!("Failed to open directory '{path}' in file manager"))?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&path).parent() {
                open_that(&Url::from_file_path(parent).map_err(|_| anyhow!("Failed to parse URL"))?)
                    .with_context(|| format!("Failed to open parent directory of '{path}' in file manager",))?;
            } else {
                open_that(&Url::from_file_path(&path).map_err(|_| anyhow!("Failed to parse URL"))?)
                    .with_context(|| format!("Failed to open '{path}' in file manager"))?;
            }
        }
    }

    Ok(())
}
