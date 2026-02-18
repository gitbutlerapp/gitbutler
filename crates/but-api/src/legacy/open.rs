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
///   It's a string as it's passed from the frontend, but ideally we'd manage to keep the original bytes.
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
/// - On all platforms, only spawn failures are detected; the terminal's later exit status is not checked
#[but_api]
#[instrument(err(Debug))]
pub fn open_in_terminal(terminal_id: String, path: String) -> Result<()> {
    use std::process::Command;

    /// Spawn a terminal process and reap it on a background thread.
    ///
    /// Terminal launches are fire-and-forget from the caller perspective, but we still
    /// wait on the child process asynchronously to avoid leaving zombie processes behind.
    fn spawn_and_reap(mut cmd: Command, terminal_name: &str, path: &str) -> Result<()> {
        tracing::info!(?cmd, "terminal command");
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to launch {terminal_name} at '{path}'"))?;

        let terminal_name = terminal_name.to_owned();
        let thread_terminal_name = terminal_name.clone();
        std::thread::Builder::new()
            .name(format!("reap-{terminal_name}"))
            .stack_size(512 * 1024)
            .spawn(move || match child.wait() {
                Ok(stat) => if !stat.success() {
                    tracing::warn!(terminal = %thread_terminal_name, status = ?stat, "Terminal process exited with error");
                },
                Err(err) => {
                    tracing::warn!(terminal = %thread_terminal_name, error = %err, "Failed to reap terminal process")
                }
            })
            .ok();

        Ok(())
    }

    if cfg!(target_os = "macos") {
        use std::process::Stdio;

        /// Helper to run a command and check its exit status
        /// Used for macOS terminals that are launched via `open` or direct commands.
        /// These typically return immediately (async launch), so we only check if the launch succeeded.
        fn run_terminal_command(mut cmd: Command, terminal_name: &str, path: &str) -> Result<()> {
            use bstr::ByteSlice;

            tracing::info!(?cmd, "terminal command");
            let output = cmd
                .output()
                .with_context(|| format!("Failed to launch {terminal_name} at '{path}'"))?;

            if output.status.success() {
                return Ok(());
            }

            let stderr = output.stderr.to_str_lossy();
            let stderr = stderr.trim();
            let status_code = output.status.code().map_or("unknown".to_string(), |c| c.to_string());
            if stderr.is_empty() {
                bail!("{terminal_name} exited with non-zero status: {status_code}",);
            } else {
                bail!("Failed to open {terminal_name} ({status_code}): {stderr}");
            }
        }

        /// Check if a macOS application is installed using `open -Ra`.
        fn ensure_app_installed(app_name: &str) -> Result<()> {
            let status = Command::new("open")
                .arg("-Ra")
                .arg(app_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("Failed to run 'open -Ra' to check application availability")?;
            if !status.success() {
                return Err(
                    anyhow::anyhow!("'{app_name}' was not found.").context(but_error::Code::DefaultTerminalNotFound)
                );
            }
            Ok(())
        }

        let open_with_path = |app_name: &str, alt_app_name: Option<&str>| {
            ensure_app_installed(app_name)?;
            let mut cmd = Command::new("open");
            cmd.arg("-a").arg(app_name).arg(&path);
            run_terminal_command(cmd, alt_app_name.unwrap_or(app_name), &path)
        };

        match terminal_id.as_str() {
            // These terminals support `open -a <app> <path>` as folder handlers
            "terminal" => open_with_path("Terminal", None)?,
            "iterm2" => open_with_path("iTerm", Some("iTerm2"))?,
            "warp" => open_with_path("Warp", None)?,
            "ghostty" => open_with_path("Ghostty", None)?,
            "hyper" => open_with_path("Hyper", None)?,
            "alacritty-mac" => {
                ensure_app_installed("Alacritty")?;
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
                let cli_found = which::which("wezterm").is_ok();
                if !cli_found {
                    return Err(anyhow::anyhow!("'wezterm' CLI was not found on PATH.")
                        .context(but_error::Code::DefaultTerminalNotFound));
                }
                let mut cmd = Command::new("wezterm");
                cmd.arg("start").arg("--cwd").arg(&path);
                run_terminal_command(cmd, "WezTerm", &path)?;
            }
            _ => bail!("Unknown terminal: {terminal_id}"),
        };
    } else if cfg!(target_os = "linux") {
        // Resolve the actual binary name (some terminals use a different binary than their ID)
        let binary = match terminal_id.as_str() {
            "warp" => "warp-terminal",
            other => other,
        };

        // Check if the terminal binary exists in PATH before attempting to launch.
        // This lets us give a clear error directing users to Settings, rather than
        // a vague launch failure (which could be confused with path issues).
        let binary_found = which::which(binary).is_ok();
        if !binary_found {
            return Err(anyhow::anyhow!("'{binary}' was not found.").context(but_error::Code::DefaultTerminalNotFound));
        }

        match terminal_id.as_str() {
            // Terminals that inherit parent process CWD (no explicit flags needed).
            // Note: `binary` is used instead of the terminal ID because some terminals
            // have a different binary name (e.g. "warp" launches "warp-terminal").
            "gnome-terminal" | "konsole" | "xfce4-terminal" | "alacritty" | "ghostty" | "warp" => {
                let mut cmd = Command::new(binary);
                cmd.current_dir(&path);
                spawn_and_reap(cmd, binary, &path)?;
            }
            // Hyper accepts path as argument (Electron app doesn't use parent CWD)
            "hyper" => {
                let mut cmd = Command::new("hyper");
                cmd.arg(&path);
                spawn_and_reap(cmd, "hyper", &path)?;
            }
            // WezTerm does not respect parent CWD and requires explicit --cwd
            "wezterm" => {
                let mut cmd = Command::new("wezterm");
                cmd.args(["start", "--cwd"]).arg(&path);
                spawn_and_reap(cmd, "wezterm", &path)?;
            }
            _ => bail!("Unknown terminal: {terminal_id}"),
        };
    } else if cfg!(windows) {
        #[cfg(windows)]
        fn create_new_console(cmd: &mut Command) -> &mut Command {
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_CONSOLE: Creates a new console for the process (0x00000010)
            // This allows the terminal to run independently without blocking our thread
            const CREATE_NEW_CONSOLE: u32 = 0x00000010;
            cmd.creation_flags(CREATE_NEW_CONSOLE)
        }
        #[cfg(not(windows))]
        fn create_new_console(cmd: &mut Command) -> &mut Command {
            cmd
        }

        use std::path::Path;

        // Validate path exists and canonicalize it to proper Windows format
        let path_buf = Path::new(&path);
        if !path_buf.exists() {
            bail!("Path does not exist: {path}");
        }
        if !path_buf.is_dir() {
            bail!("Path is not a directory: {path}");
        }

        // Canonicalize to get the absolute, properly formatted Windows path
        // This converts forward slashes to backslashes and resolves . and ..
        let canonical_path = gix::path::realpath(path_buf)
            .with_context(|| format!("Failed to canonicalize path: {path}"))?
            .to_str()
            .context("BUG: input path is String, should be able to convert back to it")?
            .to_owned();
        let canonical_path = &canonical_path;

        // Check if the terminal binary exists in PATH before attempting to launch.
        let binary_found = which::which(&terminal_id).is_ok();
        if !binary_found {
            return Err(
                anyhow::anyhow!("'{terminal_id}' was not found.").context(but_error::Code::DefaultTerminalNotFound)
            );
        }

        match terminal_id.as_str() {
            "wt" => {
                let mut cmd = Command::new("wt");
                cmd.arg("-d").arg(canonical_path);
                create_new_console(&mut cmd);
                spawn_and_reap(cmd, "Windows Terminal", canonical_path)?;
            }
            "powershell" => {
                // Set the working directory directly instead of using cd command
                let mut cmd = Command::new("powershell");
                cmd.current_dir(canonical_path)
                    // Keep the window open
                    .arg("-NoExit");
                create_new_console(&mut cmd);
                spawn_and_reap(cmd, "PowerShell", canonical_path)?;
            }
            "cmd" => {
                // Set the working directory directly - OS handles path format
                let mut cmd = Command::new("cmd");
                cmd.current_dir(canonical_path)
                    // Keep the window open
                    .arg("/K");
                create_new_console(&mut cmd);
                spawn_and_reap(cmd, "Command Prompt", canonical_path)?;
            }
            _ => bail!("Unknown terminal: {terminal_id}"),
        };
    } else {
        bail!("Unsupported platform");
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
