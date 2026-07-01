//! Linux-specific install flow

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    config::{Channel, InstallerConfig},
    download::{download_file, download_to_string},
    install::{but_binary_path, validate_installed_binary, verify_signature},
    release::{PlatformInfo, Release, validate_download_url},
    ui::{info, warn},
};

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

    // Note: For now we find the CLI and signature by convention on Linux, but we should update the
    // API (or create a new one) to contain this information.
    let filename = "but";
    let base_download_url = appimage_download_url
        .rsplit_once('/')
        .map(|(base, _)| base.to_string())
        .ok_or_else(|| anyhow::anyhow!("Failed to construct but cli URL"))?;
    let download_url = format!("{base_download_url}/{filename}");
    let signature_url = format!("{download_url}.sig");

    validate_download_url(&signature_url)?;
    validate_download_url(&download_url)?;
    info(&format!("Download URL: {download_url}"));

    let temp_dir = tempfile::Builder::new()
        .prefix("gitbutler-install.")
        .tempdir()?;
    let tmp_filepath = temp_dir.path().join(filename);

    info(&format!("Downloading GitButler {}...", release.version));
    download_file(&download_url, &tmp_filepath)?;
    info("Download completed successfully");

    let signature_b64 = download_to_string(&signature_url).with_context(|| {
        anyhow!("Failed to get signature for but, requested version may be too old")
    })?;
    verify_signature(&tmp_filepath, &signature_b64, temp_dir.path())?;

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
                bail!(
                    "Installation failed and backup restoration also failed - 'but' command may not work"
                );
            }
        } else {
            bail!("Installation failed and no backup available to restore");
        }
    } else if let Some(but_backup) = but_backup {
        info(&format!(
            "Removing backup at {}",
            but_backup.to_string_lossy()
        ));
        fs::remove_file(&but_backup)?;
    }

    Ok(())
}
