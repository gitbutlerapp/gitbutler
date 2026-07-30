#![cfg(unix)]
//! GitButler installer library
//!
//! This library provides functionality for installing GitButler on Linux and macOS systems.
//! It handles downloading releases, verifying signatures, extracting archives,
//! and setting up the application and CLI tools.

mod config;
mod download;
mod http;
mod install;
mod release;
mod shell;
pub mod ui;

#[cfg(target_os = "linux")]
mod install_linux;

#[cfg(target_os = "macos")]
mod install_macos;

use anyhow::Result;
use config::{Channel, InstallerConfig};
// Re-export types for public API consumers
pub use config::{Version, VersionRequest};
use install::but_binary_path;
use release::fetch_release;
use shell::configure_shell;
use ui::{info, success};

#[cfg(target_os = "linux")]
use crate::install_linux::download_and_install_app;
#[cfg(target_os = "macos")]
use crate::install_macos::download_and_install_app;

/// Runs the complete GitButler installation process with a specific version.
///
/// This is the main entry point that orchestrates:
/// - Fetching release information from the API
/// - Downloading and verifying the application tarball
/// - Extracting and installing the app bundle
/// - Setting up the `but` CLI and shell configuration
///
/// # Arguments
/// * `version_request` - Version to install (Release, Nightly, or Specific version).
/// * `interactive` - Whether this is an interactive installation (e.g., from the install script).
///   When `true`, runs `but onboarding` and prints usage instructions after installation.
///   Set to `false` when called from CLI tools (like `but update`) where the user
///   is already familiar with the CLI.
///
/// Returns an error if any step fails. The function will attempt to rollback
/// changes on installation failure.
pub fn run_installation_with_version(
    version_request: VersionRequest,
    interactive: bool,
) -> Result<()> {
    let config = InstallerConfig::new_with_version(version_request)?;
    run_installation_impl(config, interactive)
}

/// Runs the complete GitButler installation process.
///
/// Reads version from command-line arguments or GITBUTLER_VERSION environment variable.
/// If neither is provided, installs the latest release.
///
/// This is the entry point for the standalone installer binary, and runs in interactive mode with
/// shell configuration and other onboarding if there is a terminal connected. Otherwise it runs
/// non-interactively.
///
/// Returns an error if any step fails. The function will attempt to rollback
/// changes on installation failure.
pub fn run_installation() -> Result<()> {
    let config = InstallerConfig::new()?;
    let interactive = ui::is_connected_to_terminal();
    let but_path = but_binary_path(&config.home_dir);
    run_installation_impl(config, interactive)?;
    if supports_agent_setup(&but_path) {
        if interactive {
            ui::println_empty();
            offer_agent_skill_setup(&but_path);
        } else {
            info("To install the GitButler skill for your coding agent, run: but agent setup");
        }
    }
    Ok(())
}

/// Returns true if the installed `but` has the built-in `agent setup` command.
///
/// Detected by `but --help` listing an `agent` subcommand. Do not probe by
/// running `but agent ...` itself: pinned older versions lack the command and
/// some of them would dispatch it to an unrelated `but-agent` executable on
/// PATH, whereas `--help` is built in on every version.
fn supports_agent_setup(but_path: &std::path::Path) -> bool {
    std::process::Command::new(but_path)
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|out| {
            out.status.success()
                && String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .any(|line| line.trim_start().starts_with("agent "))
        })
}

fn run_installation_impl(config: InstallerConfig, interactive: bool) -> Result<()> {
    if interactive && !ui::is_connected_to_terminal() {
        anyhow::bail!("interactive mode requested, but there is no terminal to prompt on");
    }

    info(&format!("Detected platform: {}", config.platform));

    // Fetch release information
    let message = match &config.version_request {
        VersionRequest::Nightly => "Fetching latest nightly release information...",
        VersionRequest::Specific(_) => "Fetching release information...",
        VersionRequest::Release => "Fetching latest release information...",
    };
    info(message);

    let release = fetch_release(&config)?;

    // Display version information
    let channel = match &config.version_request {
        VersionRequest::Nightly => {
            info(&format!(
                "Installing latest nightly version: {}",
                release.version
            ));
            Some(Channel::Nightly)
        }
        VersionRequest::Specific(version) => {
            info(&format!("Installing version: {version}"));
            None
        }
        VersionRequest::Release => {
            info(&format!(
                "Installing latest release version: {}",
                release.version
            ));
            Some(Channel::Release)
        }
    };

    let platform_info = release
        .platforms
        .get(&config.platform)
        .ok_or_else(|| anyhow::anyhow!("Platform {} not found in release", config.platform))?;

    download_and_install_app(&config, platform_info, &release, channel)?;

    if interactive {
        info("Checking shell configuration");
        configure_shell(&config.home_dir)?;
    }

    ui::println_empty();
    success(&format!(
        "✓ GitButler CLI installation completed! ({}{})",
        channel
            .map(|c| format!("{} ", c.display_name()))
            .unwrap_or_default(),
        release.version
    ));
    ui::println_empty();

    if interactive {
        // Run onboarding to show metrics info message (if first run).
        // Suppress stderr - older CLI versions don't have this command and would print an error.
        let but_path = but_binary_path(&config.home_dir);
        let _ = std::process::Command::new(&but_path)
            .arg("onboarding")
            .stderr(std::process::Stdio::null())
            .status();

        info("Usage:");
        ui::println("  but --help           Show available commands");
        ui::println("  but status           Show branch status");
        ui::println("  but commit           Create a commit");
        ui::println_empty();
        info("For more information, visit: https://docs.gitbutler.com");
    }

    Ok(())
}

/// Offers to run `but agent setup` after a successful interactive installation.
///
/// Best-effort: the installation has already succeeded at this point, so
/// declining, cancelling the wizard, or any failure never fails the install.
fn offer_agent_skill_setup(but_path: &std::path::Path) {
    if !ui::prompt_for_confirmation_default_yes(
        "Install the GitButler skill for your coding agent (Claude Code, Codex, ...)?",
    ) {
        info("You can install it later with: but agent setup");
        return;
    }

    // The setup wizard requires a terminal on stdin. Under `curl | sh` this
    // process inherits curl's pipe as stdin, so give the child a duplicate of
    // our stdout instead — in interactive mode that is the terminal device,
    // opened read-write by the terminal emulator. Deliberately not an opened
    // `/dev/tty`: on macOS, kqueue cannot poll that alias device, and the
    // wizard's event loop would hang before reading a single key.
    let child_stdin = {
        use std::os::fd::AsFd;
        std::io::stdout()
            .as_fd()
            .try_clone_to_owned()
            .map(std::process::Stdio::from)
            .unwrap_or_else(|_| std::process::Stdio::inherit())
    };

    let status = std::process::Command::new(but_path)
        .args(["agent", "setup"])
        .stdin(child_stdin)
        .status();

    if !status.map(|s| s.success()).unwrap_or(false) {
        info("Agent setup did not complete. You can run it again with: but agent setup");
    }
}
