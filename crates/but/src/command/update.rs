use anyhow::Result;
#[cfg(unix)]
use but_installer::VersionRequest;
use but_settings::AppSettings;
use but_update::{AppName, CheckUpdateStatus, check_status};
use colored::Colorize;

use crate::{args::update, utils::OutputChannel};

pub fn handle(
    cmd: update::Subcommands,
    out: &mut OutputChannel,
    app_settings: &AppSettings,
) -> Result<()> {
    match cmd {
        update::Subcommands::Check => check_for_updates(out, app_settings),
        update::Subcommands::Suppress { days } => suppress_updates(out, days),
        update::Subcommands::Install { target } => install(out, target),
    }
}

fn check_for_updates(out: &mut OutputChannel, app_settings: &AppSettings) -> Result<()> {
    let mut cache = but_ctx::Context::app_cache();
    let status = check_status(AppName::Cli, app_settings, &mut cache)?;

    if let Some(status) = status {
        if let Some(writer) = out.for_human() {
            print_human_output(writer, &status)?;
        } else if let Some(out) = out.for_json() {
            out.write_value(&status)?;
        } else if let Some(writer) = out.for_shell()
            && !status.up_to_date
        {
            writeln!(writer, "{}", status.latest_version)?;
        }
    } else if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Another process is currently checking for updates",
            "→".yellow().bold()
        )?;
    }

    Ok(())
}

fn print_human_output(writer: &mut dyn std::fmt::Write, status: &CheckUpdateStatus) -> Result<()> {
    if status.up_to_date {
        writeln!(
            writer,
            "{} You're running the latest version ({})",
            "✓".green().bold(),
            status.latest_version.bold()
        )?;
    } else {
        let current_version = option_env!("VERSION").unwrap_or("0.0.0");
        writeln!(
            writer,
            "{} A new version is available: {} {} {}. Install it with 'but update install'",
            "→".yellow().bold(),
            current_version.dimmed(),
            "→".dimmed(),
            status.latest_version.green().bold()
        )?;

        if let Some(notes) = &status.release_notes {
            let trimmed = notes.trim();
            if !trimmed.is_empty() {
                writeln!(writer)?;
                writeln!(writer, "{trimmed}")?;
            }
        }

        if let Some(url) = &status.url {
            writeln!(writer)?;
            writeln!(writer, "Download: {}", url.cyan())?;
        }
    }

    Ok(())
}

fn suppress_updates(out: &mut OutputChannel, days: u32) -> Result<()> {
    // Convert days to hours (the API uses hours)
    // Note: days is already validated to be 1-30 by clap, so no overflow possible
    let hours = days * 24;

    // Call the suppress_update function
    let mut cache = but_ctx::Context::app_cache();
    but_update::suppress_update(&mut cache, hours)?;

    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Update notifications suppressed for {} {}",
            "✓".green().bold(),
            days,
            if days == 1 { "day" } else { "days" }
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "suppressed": true,
            "days": days,
            "hours": hours
        }))?;
    }

    Ok(())
}

fn install(out: &mut OutputChannel, target: Option<String>) -> Result<()> {
    // Installation requires interactive output and cannot be used with JSON mode
    // because the installer writes directly to stdout/stderr
    if out.for_json().is_some() {
        anyhow::bail!(
            "JSON output is not supported for 'but update install'.\n\n\
             The installation process requires interactive output.\n\
             For automated installations, use the standalone installer:\n\
             https://docs.gitbutler.com/installation"
        );
    }

    #[cfg(unix)]
    {
        install_unix(out, target)
    }
    #[cfg(windows)]
    {
        install_windows(out, target)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (out, target);
        anyhow::bail!("Update installation is not supported on this platform.\nPlease visit https://gitbutler.com/downloads")
    }
}

#[cfg(unix)]
fn install_unix(out: &mut OutputChannel, target: Option<String>) -> Result<()> {
    // Parse target to determine what to install
    let version_request = match target.as_deref() {
        Some("nightly") => VersionRequest::Nightly,
        Some("release") => VersionRequest::Release,
        Some(version_str) => {
            // Specific version - validate and create
            // Wrap validation errors with CLI-specific context
            VersionRequest::from_string(Some(version_str.to_string())).map_err(|e| {
                anyhow::anyhow!(
                    "Invalid version '{version_str}': {e}\n\nValid targets:\n  nightly          Install latest nightly build\n  release          Install latest stable release\n  <version>        Install specific version (e.g., 0.18.7)"
                )
            })?
        }
        None => {
            // Auto-detect from current channel
            let current_channel = but_path::AppChannel::new();
            match current_channel {
                but_path::AppChannel::Nightly => VersionRequest::Nightly,
                but_path::AppChannel::Release => VersionRequest::Release,
                but_path::AppChannel::Dev => VersionRequest::Release, // Dev installs release
            }
        }
    };

    // Call installer directly (handles all user-facing output)
    // Don't print usage info since user is already using the CLI
    but_installer::run_installation_with_version(version_request, false)?;

    // Show change log link
    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} {}",
            "→".cyan().bold(),
            "View release notes: https://gitbutler.com/releases".bold()
        )?;
        writeln!(writer)?;
    }

    invalidate_update_cache(out);

    Ok(())
}

#[cfg(windows)]
fn install_windows(out: &mut OutputChannel, target: Option<String>) -> Result<()> {
    let exe_path = std::env::current_exe()?;

    // Detect if installed via npm by checking if the executable is under a node_modules directory.
    if let Some(npm_pkg_name) = detect_npm_package(&exe_path) {
        return install_via_npm(out, &npm_pkg_name, target);
    }

    // Not an npm installation - provide download instructions
    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Automatic updates are not yet supported for this installation type on Windows.",
            "→".yellow().bold()
        )?;
        writeln!(writer)?;
        writeln!(
            writer,
            "Please download the latest version from: {}",
            "https://gitbutler.com/downloads".cyan()
        )?;
    }

    Ok(())
}

/// Detect if the executable is installed via npm and return the package name.
///
/// npm global installations on Windows place executables at paths like:
///   `C:\Users\<user>\AppData\Roaming\npm\node_modules\<package>\...`
/// or shims at:
///   `C:\Users\<user>\AppData\Roaming\npm\<name>.cmd`
///
/// We look for `node_modules` in the path to find the package name.
#[cfg(windows)]
fn detect_npm_package(exe_path: &std::path::Path) -> Option<String> {
    let components: Vec<_> = exe_path.components().collect();

    for (i, component) in components.iter().enumerate() {
        if let std::path::Component::Normal(name) = component {
            if name.to_string_lossy() == "node_modules" {
                // The next component should be the package name (or scope)
                if let Some(std::path::Component::Normal(pkg)) = components.get(i + 1) {
                    let pkg_name = pkg.to_string_lossy().to_string();
                    if pkg_name.starts_with('@') {
                        // Scoped package: @scope/name
                        if let Some(std::path::Component::Normal(name)) = components.get(i + 2) {
                            return Some(format!("{}/{}", pkg_name, name.to_string_lossy()));
                        }
                    }
                    return Some(pkg_name);
                }
            }
        }
    }

    // Also check for npm shim pattern: <npm_dir>/<name>.cmd alongside <npm_dir>/node_modules/<name>
    if let Some(parent) = exe_path.parent() {
        let node_modules = parent.join("node_modules");
        if node_modules.is_dir() {
            // Check if we look like an npm shim (e.g., but.cmd, but.exe next to node_modules/)
            if let Some(stem) = exe_path.file_stem() {
                let candidate = node_modules.join(stem.to_string_lossy().as_ref());
                if candidate.is_dir() {
                    return Some(stem.to_string_lossy().to_string());
                }
            }
        }
    }

    // Check the `npm_package_name` env var as a fallback (set during npm lifecycle scripts)
    std::env::var("npm_package_name").ok().filter(|s| !s.is_empty())
}

/// Perform an npm-based update by spawning a detached process.
///
/// On Windows, the EBUSY error occurs because npm cannot rename the package directory
/// while the current process (running from that directory) is still active. The solution
/// is to spawn a detached updater script and immediately exit, allowing npm to proceed
/// without file locks.
#[cfg(windows)]
fn install_via_npm(
    out: &mut OutputChannel,
    npm_pkg_name: &str,
    target: Option<String>,
) -> Result<()> {
    let version_suffix = match target.as_deref() {
        Some(version) => format!("@{version}"),
        None => "@latest".to_string(),
    };

    let install_spec = format!("{npm_pkg_name}{version_suffix}");

    if let Some(writer) = out.for_human() {
        writeln!(
            writer,
            "{} Detected npm installation (package: {})",
            "→".cyan().bold(),
            npm_pkg_name.bold()
        )?;
        writeln!(
            writer,
            "{} Spawning background updater to avoid EBUSY file lock errors...",
            "→".cyan().bold(),
        )?;
    }

    // Create a temporary batch script that:
    // 1. Waits for this process to exit (via a short delay)
    // 2. Runs npm install -g <package>
    // 3. Reports the result
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("but-update.cmd");
    let log_path = temp_dir.join("but-update.log");

    let script_content = format!(
        "@echo off\r\n\
         echo Waiting for the CLI process to exit...\r\n\
         timeout /t 3 /nobreak >nul 2>&1\r\n\
         echo Updating {install_spec}...\r\n\
         npm install -g {install_spec} > \"{log}\" 2>&1\r\n\
         if %errorlevel% equ 0 (\r\n\
             echo.\r\n\
             echo Update completed successfully.\r\n\
         ) else (\r\n\
             echo.\r\n\
             echo Update failed. Check the log at: {log}\r\n\
             echo You can also try updating manually:\r\n\
             echo   1. Close all terminals running 'but'\r\n\
             echo   2. Run: npm install -g {install_spec}\r\n\
         )\r\n\
         pause\r\n",
        log = log_path.display(),
    );

    std::fs::write(&script_path, &script_content)?;

    // Spawn the script in a new, visible console window so the user can see progress.
    // The CREATE_NEW_CONSOLE flag ensures the script runs independently.
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;

    std::process::Command::new("cmd.exe")
        .args(["/c", "start", "\"GitButler Update\"", "/wait"])
        .arg(&script_path)
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()?;

    if let Some(writer) = out.for_human() {
        writeln!(writer)?;
        writeln!(
            writer,
            "{} Update is running in a separate window.",
            "✓".green().bold()
        )?;
        writeln!(
            writer,
            "  The update log will be saved to: {}",
            log_path.display()
        )?;
        writeln!(writer)?;
        writeln!(
            writer,
            "  If the update fails, close all terminals running 'but' and run:"
        )?;
        writeln!(writer, "    npm install -g {install_spec}")?;
    }

    invalidate_update_cache(out);

    Ok(())
}

fn invalidate_update_cache(out: &mut OutputChannel) {
    let mut cache = but_ctx::Context::app_cache();
    if let Err(err) = cache.update_check_mut().and_then(|handle| handle.delete()) {
        tracing::warn!(?err, "Failed to invalidate update check cache");
        if let Some(writer) = out.for_human() {
            writeln!(
                writer,
                "Failed to invalidate update check cache - skipping invalidation: {err:?}",
            )
            .ok();
        }
    }
}
