//! GitButler installer library
//!
//! This library provides functionality for installing GitButler on macOS systems.
//! It handles downloading releases, verifying signatures, extracting archives,
//! and setting up the application and CLI tools.

mod config;
mod download;
mod http;
mod install;
mod release;
mod shell;
pub mod ui;

use anyhow::Result;
use config::{Channel, InstallerConfig};
// Re-export types for public API consumers
pub use config::{Version, VersionRequest};
use download::{download_file, validate_tarball, verify_signature};
use install::{extract_tarball, install_app, verify_app_structure};
use release::{fetch_release, validate_download_url};
use shell::setup_path;
use ui::{info, success};

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
/// * `print_usage` - Whether to print usage instructions after installation.
///   Set to `false` when called from CLI tools where usage is already known.
///
/// Returns an error if any step fails. The function will attempt to rollback
/// changes on installation failure.
pub fn run_installation_with_version(version_request: VersionRequest, print_usage: bool) -> Result<()> {
    let config = InstallerConfig::new_with_version(version_request)?;
    run_installation_impl(config, print_usage)
}

/// Runs the complete GitButler installation process.
///
/// Reads version from command-line arguments or GITBUTLER_VERSION environment variable.
/// If neither is provided, installs the latest release.
///
/// Returns an error if any step fails. The function will attempt to rollback
/// changes on installation failure.
pub fn run_installation() -> Result<()> {
    let config = InstallerConfig::new()?;
    run_installation_impl(config, true)
}

fn run_installation_impl(config: InstallerConfig, print_usage: bool) -> Result<()> {
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
    match &config.version_request {
        VersionRequest::Nightly => {
            info(&format!("Installing latest nightly version: {}", release.version));
        }
        VersionRequest::Specific(version) => {
            info(&format!("Installing version: {}", version));
        }
        VersionRequest::Release => {
            info(&format!("Latest version: {}", release.version));
        }
    }

    let platform_info = release
        .platforms
        .get(&config.platform)
        .ok_or_else(|| anyhow::anyhow!("Platform {} not found in release", config.platform))?;

    let download_url = platform_info.url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "No download URL for platform {} in release {}",
            config.platform,
            release.version
        )
    })?;

    // Validate download URL
    validate_download_url(download_url)?;

    info(&format!("Download URL: {}", download_url));

    // Create temporary directory
    let temp_dir = tempfile::Builder::new().prefix("gitbutler-install.").tempdir()?;

    // Download the tarball
    let filename = download_url
        .split('/')
        .next_back()
        .ok_or_else(|| anyhow::anyhow!("Failed to extract filename from download URL"))?;
    let tarball_path = temp_dir.path().join(filename);

    info(&format!("Downloading GitButler {}...", release.version));
    download_file(download_url, &tarball_path)?;

    // Validate download
    validate_tarball(&tarball_path)?;
    success("Download completed successfully");

    // Verify signature
    verify_signature(&tarball_path, &platform_info.signature, temp_dir.path())?;

    // Extract tarball
    info("Extracting archive...");
    let app_dir = extract_tarball(&tarball_path, temp_dir.path())?;
    success("Archive extracted successfully");

    // Verify app structure
    verify_app_structure(&app_dir)?;

    // Detect channel from app bundle name
    let app_basename = app_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get app bundle name"))?;
    let channel = detect_channel(app_basename);

    // Install the app bundle
    install_app(&app_dir, &config.home_dir, channel)?;

    // Setup PATH
    setup_path(&config.home_dir)?;

    ui::println_empty();
    success(&format!(
        "✓ GitButler CLI installation completed! ({} {})",
        channel.display_name(),
        release.version
    ));
    ui::println_empty();

    if print_usage {
        info("Usage:");
        ui::println("  but --help           Show available commands");
        ui::println("  but status           Show branch status");
        ui::println("  but commit           Create a commit");
        ui::println_empty();
        info("For more information, visit: https://docs.gitbutler.com");
    }

    Ok(())
}

/// Detect the channel from an app bundle name
fn detect_channel(app_name: &str) -> Channel {
    if app_name.contains("Nightly") {
        Channel::Nightly
    } else {
        Channel::Release
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_channel() {
        assert_eq!(detect_channel("GitButler.app"), Channel::Release);
        assert_eq!(detect_channel("GitButler Nightly.app"), Channel::Nightly);
        assert_eq!(detect_channel("GitButler_Nightly.app"), Channel::Nightly);
    }
}
