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
use shell::setup_path;
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
pub fn run_installation_with_version(version_request: VersionRequest, interactive: bool) -> Result<()> {
    let config = InstallerConfig::new_with_version(version_request)?;
    run_installation_impl(config, interactive)
}

/// Runs the complete GitButler installation process.
///
/// Reads version from command-line arguments or GITBUTLER_VERSION environment variable.
/// If neither is provided, installs the latest release.
///
/// This is the entry point for the standalone installer binary, so it runs in
/// interactive mode (runs onboarding and prints usage instructions).
///
/// Returns an error if any step fails. The function will attempt to rollback
/// changes on installation failure.
pub fn run_installation() -> Result<()> {
    let config = InstallerConfig::new()?;
    run_installation_impl(config, true)
}

fn run_installation_impl(config: InstallerConfig, interactive: bool) -> Result<()> {
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
            info(&format!("Installing latest nightly version: {}", release.version));
            Some(Channel::Nightly)
        }
        VersionRequest::Specific(version) => {
            info(&format!("Installing version: {version}"));
            Some(Channel::Release)
        }
        VersionRequest::Release => {
            info(&format!("Latest version: {}", release.version));
            None
        }
    };

    let platform_info = release
        .platforms
        .get(&config.platform)
        .ok_or_else(|| anyhow::anyhow!("Platform {} not found in release", config.platform))?;

    download_and_install_app(&config, platform_info, &release, channel)?;

    // Setup PATH
    setup_path(&config.home_dir)?;

    ui::println_empty();
    success(&format!(
        "✓ GitButler CLI installation completed! ({}{})",
        channel.map(|c| format!("{} ", c.display_name())).unwrap_or_default(),
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
