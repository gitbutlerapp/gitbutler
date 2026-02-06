//! Linux-specific install flow

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Result, anyhow, bail};

use crate::config::{Channel, InstallerConfig};
use crate::download::download_file;
use crate::install::{but_binary_path, validate_installed_binary};
use crate::release::validate_download_url;
use crate::release::{PlatformInfo, Release};
use crate::ui::info;
use crate::ui::warn;

pub(crate) fn download_and_install_app(
    config: &InstallerConfig,
    platform_info: &PlatformInfo,
    release: &Release,
    channel: Option<Channel>,
) -> Result<()> {
    let appimage_download_url = platform_info.url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "No download URL for platform {} in release {}",
            config.platform,
            release.version
        )
    })?;

    // Note: For now we find the CLI by convention on Linux, but we should update the API to point
    // to its location at some point.
    let filename = "but";
    let download_url = appimage_download_url
        .rsplit_once('/')
        .map(|(base, _)| format!("{}/{filename}", base))
        .ok_or_else(|| anyhow::anyhow!("Failed to construct but cli URL"))?;
    let download_url = download_url.as_str();

    validate_download_url(download_url)?;

    info(&format!("Download URL: {}", download_url));

    let temp_dir = tempfile::Builder::new().prefix("gitbutler-install.").tempdir()?;
    let tmp_filepath = temp_dir.path().join(filename);

    info(&format!("Downloading GitButler {}...", release.version));
    download_file(download_url, &tmp_filepath)?;
    info("Download completed successfully");

    // TODO verify signature

    // Install the app bundle
    install_app(&tmp_filepath, &config.home_dir, channel)?;

    Ok(())
}

fn install_app(but_path: &Path, home_dir: &Path, channel: Option<Channel>) -> Result<()> {
    let install_bin_path = but_binary_path(home_dir);
    let bin_dir = install_bin_path
        .parent()
        .ok_or_else(|| anyhow!("Failed to resolve bin dir path"))?;
    fs::create_dir_all(bin_dir)?;

    let but_backup = if install_bin_path.is_file() {
        let suffix: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64 ^ std::process::id() as u64)
            .unwrap_or(std::process::id() as u64);
        let mut backup_name = install_bin_path.as_os_str().to_owned();
        backup_name.push(format!(".{suffix:x}"));
        let but_backup = std::path::PathBuf::from(backup_name);

        info(&format!(
            "Found existing install at {}, backing up to {}",
            install_bin_path.to_string_lossy(),
            but_backup.to_string_lossy()
        ));

        fs::rename(&install_bin_path, &but_backup)?;
        Some(but_backup)
    } else {
        None
    };

    info(&format!(
        "Installing{} to {}...",
        channel
            .map(|c| format!(" channel {}", c.display_name()))
            .unwrap_or_default(),
        install_bin_path.to_string_lossy(),
    ));

    // NOTE: Must copy rather than rename. Rename assumes source and dest are on the same mount
    // point, but /tmp on Linux systems is very often an in-memory file system (tmpfs) and thus on
    // a different mount point than persistent files
    fs::copy(but_path, &install_bin_path)?;

    let mut perms = fs::metadata(&install_bin_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&install_bin_path, perms)?;

    if !validate_installed_binary(&install_bin_path) {
        warn("Final installation verification failed");

        if let Some(but_backup) = but_backup {
            warn("Attempting to restore backup");
            fs::rename(&but_backup, &install_bin_path)?;

            if validate_installed_binary(&install_bin_path) {
                info("Backup restored successfully, exiting ...");
                bail!("Installation failed but your previous installation was restored");
            } else {
                bail!("Installation failed and backup restoration also failed - 'but' command may not work");
            }
        } else {
            bail!("Installation failed and no backup available to restore");
        }
    } else if let Some(but_backup) = but_backup {
        info(&format!("Removing backup at {}", but_backup.to_string_lossy()));
        fs::remove_file(&but_backup)?;
    }

    Ok(())
}
