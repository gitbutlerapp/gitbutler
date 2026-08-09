//! In place of commands.rs
use std::{env, ffi::OsStr};

use anyhow::{Context as _, Result, bail};
use but_api_macros::but_api;
use but_error::bail_precondition;
use std::path::Path;
use tracing::instrument;
use url::Url;

use crate::open::program::FILE_EXTENSION_WILDCARD;
use crate::open::{
    program::{
        OpenSpec, PROGRAMS, ProgramSpec, USER_DEFINED_PROGRAMS_FILENAME, UserDefinedProgramSpec,
        json::{Editor, Program},
        open_in_program_unchecked,
    },
    spawn::spawn_and_reap,
};

/// Terminal configuration and detection.
pub mod terminal;

/// Editor configuration.
pub mod program;

/// Spawn helpers.
pub(crate) mod spawn;

/// Opens a supported URL or file path with the platform's default handler.
///
/// Editor URLs are opened through a direct CLI invocation on WSL when possible;
/// all other supported schemes are delegated to the system opener after
/// cleaning environment variables that can point to transient AppImage mounts.
///
/// # Errors
///
/// Returns an error when the URL scheme is unsupported, a `file://` URL cannot
/// be converted to a path, or every available opener command fails to launch.
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
        "antigravity-ide",
        "file",
    ]
    .contains(&target_url.scheme())
    {
        bail!("Invalid path scheme: {}", target_url.scheme());
    }

    if open_editor_url_as_command_invocation_on_wsl(target_url) {
        return Ok(());
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

/// Opens supported editor URLs directly inside WSL.
///
/// The normal URL opener can fail to route `vscode://file/...`-style URLs back
/// to Linux editor CLIs when GitButler runs in WSL, so this attempts a direct
/// invocation first. Returns `true` only when a supported editor command was
/// executed successfully; unsupported URLs and failed launches fall back to the
/// generic opener.
fn open_editor_url_as_command_invocation_on_wsl(target_url: &Url) -> bool {
    use std::process::Command;

    if !is_wsl() {
        return false;
    }

    let Some((command, args)) = wsl_editor_invocation(target_url) else {
        return false;
    };

    let mut cmd = Command::new(command);
    cmd.args(args);
    tracing::info!(?cmd, "editor command");
    matches!(cmd.status(), Ok(status) if status.success())
}

fn is_wsl() -> bool {
    cfg!(target_os = "linux") && env::var_os("WSL_DISTRO_NAME").is_some()
        || env::var_os("WSL_INTEROP").is_some()
}

/// Builds the editor CLI command needed to open a WSL editor URL.
///
/// Returns the executable name and argument list for supported
/// `editor://file/...` URLs, including `--new-window` and `--goto` when the URL
/// requests them.
/// Returns `None` when the URL does not target a supported editor,
/// is not a file URL, or cannot be converted into a non-empty local path.
fn wsl_editor_invocation(target_url: &Url) -> Option<(&'static str, Vec<String>)> {
    if target_url.host_str() != Some("file") {
        return None;
    }
    let command = scheme_to_wsl_editor_command(target_url.scheme())?;
    let path = editor_url_path(target_url)?;
    if path.is_empty() {
        return None;
    }

    let mut args = Vec::new();
    if is_vscode_or_compatible(target_url.scheme()) {
        if target_url
            .query_pairs()
            .any(|(key, value)| key == "windowId" && value == "_blank")
        {
            args.push("--new-window".to_owned());
        }

        if path_has_position(&path) {
            args.push("--goto".to_owned());
        }
    }
    args.push(path);

    Some((command, args))
}

fn is_vscode_or_compatible(scheme: &str) -> bool {
    matches!(
        scheme,
        "vscode"
            | "vscode-insiders"
            | "vscodium"
            | "cursor"
            | "windsurf"
            | "trae"
            | "antigravity-ide"
    )
}

/// Maps editor URL schemes to the CLI binaries expected in the WSL PATH.
///
/// WSL editor URLs use stable protocol schemes such as `vscode://`, but the
/// Linux launch path needs the editor's command-line executable instead of the
/// URL scheme. Unsupported schemes are left for the generic URL opener.
fn scheme_to_wsl_editor_command(scheme: &str) -> Option<&'static str> {
    match scheme {
        "vscode" => Some("code"),
        "vscode-insiders" => Some("code-insiders"),
        "vscodium" => Some("codium"),
        "cursor" => Some("cursor"),
        "windsurf" => Some("windsurf"),
        "trae" => Some("trae"),
        "antigravity-ide" => Some("antigravity-ide"),
        _ => {
            tracing::warn!(%scheme, "missing WSL editor scheme mapping");
            None
        }
    }
}

fn editor_url_path(target_url: &Url) -> Option<String> {
    let file_url = Url::parse(&format!("file://{}", target_url.path())).ok()?;
    file_url
        .to_file_path()
        .ok()?
        .to_str()
        .map(ToOwned::to_owned)
}

fn path_has_position(path: &str) -> bool {
    let Some((_, line_or_column)) = path.rsplit_once(':') else {
        return false;
    };

    line_or_column.parse::<u32>().is_ok()
}

#[cfg(test)]
mod wsl_tests {
    use super::*;

    #[test]
    fn cursor_editor_url_becomes_goto_cli_invocation() {
        let url = Url::parse("cursor://file/home/example/project/src/main.rs:42:7").unwrap();

        assert_eq!(
            wsl_editor_invocation(&url),
            Some((
                "cursor",
                vec![
                    "--goto".to_owned(),
                    "/home/example/project/src/main.rs:42:7".to_owned(),
                ],
            ))
        );
    }

    #[test]
    fn vscode_project_url_becomes_new_window_cli_invocation() {
        let url = Url::parse("vscode://file/home/example/project?windowId=_blank").unwrap();

        assert_eq!(
            wsl_editor_invocation(&url),
            Some((
                "code",
                vec![
                    "--new-window".to_owned(),
                    "/home/example/project".to_owned()
                ]
            ))
        );
    }

    #[test]
    fn editor_url_paths_are_percent_decoded() {
        let url = Url::parse("vscode://file/home/example/My%20Project/src/lib.rs:3").unwrap();

        assert_eq!(
            wsl_editor_invocation(&url),
            Some((
                "code",
                vec![
                    "--goto".to_owned(),
                    "/home/example/My Project/src/lib.rs:3".to_owned(),
                ],
            ))
        );
    }

    #[test]
    fn unsupported_editor_scheme_falls_back_to_url_handler() {
        let url = Url::parse("zed://file/home/example/project/src/main.rs:42").unwrap();

        assert_eq!(wsl_editor_invocation(&url), None);
    }
}

/// Opens a `url` or file path with the platform's default handler.
///
/// The URL must use one of GitButler's supported schemes, including web,
/// mail, editor, and `file://` URLs.
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
/// - `kitty` - Kitty
///
/// **Windows:**
/// - `wt` - Windows Terminal
/// - `powershell` - PowerShell
/// - `cmd` - Command Prompt
///
/// **Linux:**
/// - `ptyxis` - Ptyxis
/// - `gnome-terminal` - GNOME Terminal
/// - `konsole` - KDE Konsole
/// - `xfce4-terminal` - XFCE Terminal
/// - `alacritty` - Alacritty
/// - `ghostty` - Ghostty
/// - `warp` - Warp
/// - `hyper` - Hyper
/// - `wezterm` - WezTerm
/// - `kitty` - Kitty
/// - `cosmic-term` - COSMIC Terminal
///
/// # Errors
/// Returns an error if:
/// - The terminal application is not installed or not found in PATH
/// - The specified path does not exist or is not accessible
/// - The terminal_id is not recognized for the current platform
/// - On all platforms, only spawn failures are detected; the terminal's later exit status is not checked
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn open_in_terminal(terminal_id: String, path: String) -> Result<()> {
    use std::process::Command;

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
            let status_code = output
                .status
                .code()
                .map_or("unknown".to_string(), |c| c.to_string());
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
                return Err(anyhow::anyhow!(
                    "'{app_name}' was not found - `open -Ra {app_name}` failed."
                )
                .context(but_error::Code::DefaultTerminalNotFound));
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
            "kitty" => open_with_path("kitty", Some("Kitty"))?,
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
        let binary = terminal::terminal_binary(&terminal_id);

        // Check if the terminal binary exists in PATH before attempting to launch.
        // This lets us give a clear error directing users to Settings, rather than
        // a vague launch failure (which could be confused with path issues).
        let binary_found = which::which(binary).is_ok();
        if !binary_found {
            return Err(anyhow::anyhow!(
                "'{binary}' was not found. Make sure it is installed and available on your PATH."
            )
            .context(but_error::Code::DefaultTerminalNotFound));
        }

        match terminal_id.as_str() {
            // Terminals that inherit parent process CWD (no explicit flags needed).
            // Note: `binary` is used instead of the terminal ID because some terminals
            // have a different binary name (e.g. "warp" launches "warp-terminal").
            "gnome-terminal" | "konsole" | "xfce4-terminal" | "alacritty" | "ghostty" | "warp"
            | "kitty" | "cosmic-term" => {
                let mut cmd = Command::new(binary);
                cmd.current_dir(&path);
                spawn_and_reap(cmd, binary, &path)?;
            }
            // Ptyxis requires --new-window argument
            "ptyxis" => {
                let mut cmd = Command::new(binary);
                cmd.arg("--new-window");
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
            return Err(anyhow::anyhow!("'{terminal_id}' was not found.")
                .context(but_error::Code::DefaultTerminalNotFound));
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

/// Reveals the file or directory at `path` in the platform's file manager.
///
/// On macOS this reveals the item in Finder, on Windows it selects the item in
/// Explorer, and on Linux it opens either the directory itself or the file's
/// parent directory.
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
            open_that(
                &Url::from_file_path(&path).map_err(|_| anyhow::anyhow!("Failed to parse URL"))?,
            )
            .with_context(|| format!("Failed to open directory '{path}' in file manager"))?;
        } else {
            // For files, try to open the parent directory
            if let Some(parent) = std::path::Path::new(&path).parent() {
                open_that(
                    &Url::from_file_path(parent)
                        .map_err(|_| anyhow::anyhow!("Failed to parse URL"))?,
                )
                .with_context(|| {
                    format!("Failed to open parent directory of '{path}' in file manager",)
                })?;
            } else {
                open_that(
                    &Url::from_file_path(&path)
                        .map_err(|_| anyhow::anyhow!("Failed to parse URL"))?,
                )
                .with_context(|| format!("Failed to open '{path}' in file manager"))?;
            }
        }
    }

    Ok(())
}

/// List all editors that can be opened from a GUI client.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_editors() -> anyhow::Result<Vec<Editor>> {
    Ok(list_program_specs()
        .iter()
        .filter(|p| p.is_gui_editor())
        .map(Into::into)
        .collect())
}

/// List all programs that can be opened from a GUI client.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn list_programs() -> anyhow::Result<Vec<Program>> {
    Ok(list_program_specs()
        .iter()
        .filter(|program| !program.requires_terminal())
        .map(Into::into)
        .collect())
}

/// List all user-defined programs.
///
/// This function never fails, it only logs errors as warnings if something goes wrong.
fn list_user_defined_program_specs() -> Vec<ProgramSpec> {
    let Ok(user_defined_programs_file) =
        but_path::app_config_dir().map(|dir| dir.join(USER_DEFINED_PROGRAMS_FILENAME))
    else {
        return vec![];
    };

    let content = match std::fs::read_to_string(&user_defined_programs_file) {
        Ok(content) => content,
        Err(err) => {
            tracing::info!(
                ?err,
                ?user_defined_programs_file,
                "Failed to read from user-defined programs file, probably does not exist"
            );
            return vec![];
        }
    };

    let user_defined_program_specs: Vec<UserDefinedProgramSpec> =
        match serde_json::from_str(&content) {
            Ok(specs) => specs,
            Err(err) => {
                tracing::warn!(?err, "Failed to decode user-defined programs file");
                vec![]
            }
        };

    user_defined_program_specs
        .into_iter()
        .filter_map(|spec| match spec.try_into_program_spec() {
            Ok(spec) => Some(spec),
            Err(err) => {
                tracing::warn!(?err, "Failed to decode user defined program specification");
                None
            }
        })
        .collect()
}

/// List all program specifications.
pub fn list_program_specs() -> Vec<ProgramSpec> {
    let mut specs = list_user_defined_program_specs();

    for builtin in PROGRAMS.iter() {
        if !specs
            .iter()
            .any(|user_defined| user_defined.id == builtin.id)
        {
            specs.push(builtin.clone());
        }
    }

    specs
}

/// List all programs that are suitable to use to open the file based on the file extension, or all
/// programs if no specific ones are found.
pub fn list_program_specs_for_file(path: &Path) -> Vec<ProgramSpec> {
    let all_specs = list_program_specs();
    let ext = path.extension().and_then(OsStr::to_str);

    let mut exact_extension_matches = vec![];
    let mut wildcard_extension_matches = vec![];

    for spec in &all_specs {
        if let Some(extensions) = &spec.extensions {
            if let Some(ext) = ext
                && extensions.iter().any(|v| v == ext)
            {
                exact_extension_matches.push(spec.clone());
            } else if extensions.iter().any(|v| v == FILE_EXTENSION_WILDCARD) {
                wildcard_extension_matches.push(spec.clone());
            }
        }
    }

    if !exact_extension_matches.is_empty() {
        let mut exact_matches_and_wildcards = exact_extension_matches;

        // still include wildcard matches even if there are exact matches
        exact_matches_and_wildcards.append(&mut wildcard_extension_matches);

        // maintain the sort from programs.json
        exact_matches_and_wildcards
            .sort_unstable_by_key(|program| all_specs.iter().position(|p| p.id == program.id));

        exact_matches_and_wildcards
    } else if !wildcard_extension_matches.is_empty() {
        wildcard_extension_matches
    } else {
        all_specs
    }
}

/// Open `path` within the given project's workdir using the program specified by `program_id`.
///
/// `path` is resolved relative to the repository workdir.
///
/// `line_nr` can be provided to open a file at a specific line.
///
/// [`list_programs`] provides the available `program_id`s.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn open_in_program(
    ctx: &mut but_ctx::Context,
    program_id: String,
    path: String,
    line_nr: Option<u32>,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let resolved_path = repo
        .workdir_path(path.as_str())
        .context("project must have a workdir")?;

    let program_specs = list_program_specs();
    if let Some(program) = program_specs
        .iter()
        .find(|program| program.id == program_id)
        && !program.requires_terminal()
    {
        let open_spec = match line_nr {
            Some(line_nr) => OpenSpec::FileAtLine(resolved_path, line_nr),
            None => OpenSpec::File(resolved_path),
        };
        open_in_program_unchecked(program, open_spec)
    } else {
        bail_precondition!("program_id '{program_id}' is not a GUI-compatible program");
    }
}
