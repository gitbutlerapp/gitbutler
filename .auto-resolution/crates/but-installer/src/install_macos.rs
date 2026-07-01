//! macOS-specific installation logic

use std::{
    fs::{self, File},
    io,
    io::Read,
    os::unix::fs as unix_fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::GzDecoder;
use tar::Archive;

use crate::{
    config::{Channel, InstallerConfig},
    download::download_file,
    install::{validate_installed_binary, verify_signature},
    release::{PlatformInfo, Release, validate_download_url},
    ui::{info, success, warn},
};

pub fn download_and_install_app(
    config: &InstallerConfig,
    platform_info: &PlatformInfo,
    release: &Release,
    channel: Option<Channel>,
) -> Result<()> {
    let download_url = platform_info.url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "No download URL for platform {} in release {}",
            config.platform,
            release.version
        )
    })?;

    validate_download_url(download_url)?;
    info(&format!("Download URL: {download_url}"));

    let temp_dir = tempfile::Builder::new()
        .prefix("gitbutler-install.")
        .tempdir()?;

    let filename = download_url
        .split('/')
        .next_back()
        .ok_or_else(|| anyhow::anyhow!("Failed to extract filename from download URL"))?;
    let tarball_path = temp_dir.path().join(filename);

    info(&format!("Downloading GitButler {}...", release.version));
    download_file(download_url, &tarball_path)?;

    validate_tarball(&tarball_path)?;
    success("Download completed successfully");

    verify_signature(&tarball_path, &platform_info.signature, temp_dir.path())?;

    info("Extracting archive...");
    let app_dir = extract_tarball(&tarball_path, temp_dir.path())?;
    success("Archive extracted successfully");

    verify_app_structure(&app_dir)?;

    install_app(&app_dir, &config.home_dir, channel)?;

    Ok(())
}

pub(crate) fn install_app(app_dir: &Path, home_dir: &Path, channel: Option<Channel>) -> Result<()> {
    let app_basename = app_dir
        .file_name()
        .ok_or_else(|| anyhow!("Failed to get app bundle name"))?;

    let install_app = home_dir.join("Applications").join(app_basename);
    let install_app_backup = home_dir
        .join("Applications")
        .join(format!("{}.backup", app_basename.to_string_lossy()));
    let install_app_new = home_dir
        .join("Applications")
        .join(format!("{}.new", app_basename.to_string_lossy()));

    info(&format!(
        "Installing{} to {}...",
        channel
            .map(|c| format!(" channel {}", c.display_name()))
            .unwrap_or_default(),
        install_app.display()
    ));

    // Clean up any leftover temp files from previous failed installations
    let _ = fs::remove_dir_all(&install_app_new);
    let _ = fs::remove_dir_all(&install_app_backup);

    // Create Applications directory if it doesn't exist
    fs::create_dir_all(home_dir.join("Applications"))?;

    // Install to temporary location first
    info("Installing to temporary location...");
    copy_dir_all(app_dir, &install_app_new)?;

    // Remove macOS quarantine attribute
    if let Err(e) = remove_quarantine_recursive(&install_app_new) {
        warn(&format!(
            "Could not remove quarantine attribute: {e} - macOS may show security warnings"
        ));
        info("If macOS blocks the app, go to System Settings > Privacy & Security and allow it");
    }

    // Create bin directory
    let bin_dir = home_dir.join(".local/bin");
    fs::create_dir_all(&bin_dir)?;

    // Check for existing 'but' and detect channel switching
    let but_symlink = bin_dir.join("but");
    let but_new = bin_dir.join("but.new");

    if but_symlink.exists() && !but_symlink.is_symlink() {
        // 'but' exists but is not a symlink
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = format!("{now}");
        let but_backup = bin_dir.join(format!("but.backup.{timestamp}"));
        warn(&format!(
            "A 'but' binary already exists at {} (not a symlink)",
            but_symlink.display()
        ));
        warn(&format!(
            "Moving it to {} to preserve your existing file",
            but_backup.display()
        ));
        fs::rename(&but_symlink, &but_backup)?;
        info(&format!(
            "Your original 'but' has been saved to: {}",
            but_backup.display()
        ));
    } else if but_symlink.is_symlink() {
        let existing_target = fs::read_link(&but_symlink)?;
        let existing_target_str = existing_target.to_string_lossy();

        // Detect channel switching
        let previous_channel = if existing_target_str.contains("Nightly") {
            Some(Channel::Nightly)
        } else if existing_target_str.contains("/GitButler.app/") {
            Some(Channel::Release)
        } else {
            None
        };

        if previous_channel.is_none() {
            warn(&format!(
                "Found existing 'but' symlink pointing to: {existing_target_str}"
            ));
            warn("This will be replaced with GitButler's 'but' command");
            info(
                "Note: Your custom symlink setup will be overwritten. The original target is not a GitButler installation.",
            );
        }
    }

    // Create temporary symlink to test the new installation
    let new_app_macos_dir = install_app_new.join("Contents/MacOS/gitbutler-tauri");
    let _ = fs::remove_file(&but_new);

    unix_fs::symlink(&new_app_macos_dir, &but_new)?;

    // Verify the new installation works
    let verify_status = Command::new(&but_new)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if !verify_status.map(|s| s.success()).unwrap_or(false) {
        fs::remove_dir_all(&install_app_new)?;
        fs::remove_file(&but_new)?;
        bail!(
            "New installation verification failed - 'but' binary cannot run (may be corrupted or blocked by macOS)"
        );
    }

    // New installation is valid - now do the atomic swap
    info("Swapping new installation into place...");

    // Helper to clean up stale installation artifacts
    let cleanup_artifacts = || {
        let _ = fs::remove_dir_all(&install_app_new);
        let _ = fs::remove_file(&but_new);
    };

    // Backup existing installation if it exists
    let had_backup = install_app.exists();
    if had_backup && let Err(e) = fs::rename(&install_app, &install_app_backup) {
        // Failed to backup - clean up artifacts before failing
        cleanup_artifacts();
        return Err(e.into());
    }

    // Move new installation into place with rollback on failure
    if let Err(e) = fs::rename(&install_app_new, &install_app) {
        // Clean up stale artifacts before handling the error
        cleanup_artifacts();

        // Critical failure - restore backup if we created one
        if had_backup && install_app_backup.exists() {
            warn("Failed to move new installation into place - restoring backup");
            if let Err(restore_err) = fs::rename(&install_app_backup, &install_app) {
                bail!(
                    "Failed to install new version: {}. Also failed to restore backup: {}. Your app may be at {}",
                    e,
                    restore_err,
                    install_app_backup.display()
                );
            }
            bail!(
                "Failed to install new version: {e}. Previous installation restored successfully."
            );
        }
        // No backup to restore, just fail
        bail!("Failed to move new installation into place: {e}");
    }

    // Update the symlink to point to the new installation
    let final_target = install_app.join("Contents/MacOS/gitbutler-tauri");
    let _ = fs::remove_file(&but_symlink);

    // Try to create symlink and verify - if either fails, rollback
    let symlink_result = unix_fs::symlink(&final_target, &but_symlink);
    let _ = fs::remove_file(&but_new);

    if let Err(e) = symlink_result {
        // Symlink creation failed - rollback to backup
        warn(&format!(
            "Failed to create symlink: {e} - attempting to restore backup"
        ));
        if install_app_backup.exists() {
            if let Err(remove_err) = fs::remove_dir_all(&install_app) {
                bail!(
                    "Failed to create symlink and failed to remove new installation during rollback: {}. Backup at: {}",
                    remove_err,
                    install_app_backup.display()
                );
            }
            fs::rename(&install_app_backup, &install_app)?;

            let restored_target = install_app.join("Contents/MacOS/gitbutler-tauri");
            let _ = fs::remove_file(&but_symlink);
            let _ = unix_fs::symlink(&restored_target, &but_symlink);

            bail!("Failed to create symlink: {e}. Previous installation was restored.");
        } else {
            bail!("Failed to create symlink: {e}. No backup available to restore.");
        }
    }

    if !validate_installed_binary(&but_symlink) {
        // Try to restore backup
        warn("Final installation verification failed - attempting to restore backup");
        if install_app_backup.exists() {
            fs::remove_dir_all(&install_app)?;
            fs::rename(&install_app_backup, &install_app)?;

            let restored_target = install_app.join("Contents/MacOS/gitbutler-tauri");
            let _ = fs::remove_file(&but_symlink);
            unix_fs::symlink(&restored_target, &but_symlink)?;

            if validate_installed_binary(&but_symlink) {
                success("Backup was restored successfully");
                bail!("Installation failed but your previous installation was restored");
            } else {
                bail!(
                    "Installation failed and backup restoration also failed - 'but' command may not work"
                );
            }
        } else {
            bail!("Installation failed and no backup available to restore");
        }
    }

    success(&format!(
        "{} installed successfully",
        app_basename.to_string_lossy()
    ));
    // Remove backup on success
    let _ = fs::remove_dir_all(&install_app_backup);

    success("GitButler CLI (but) installed successfully");

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_symlink() {
            let target = fs::read_link(&src_path)?;
            unix_fs::symlink(&target, &dst_path)?;
        } else if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub(crate) fn extract_tarball(tarball: &Path, dest_dir: &Path) -> Result<PathBuf> {
    let file = File::open(tarball)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(dest_dir)
        .context("Failed to extract archive")?;

    // Find the extracted .app bundle
    let mut app_dir = None;
    for entry in fs::read_dir(dest_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("app") {
            app_dir = Some(path);
            break;
        }
    }

    app_dir.ok_or_else(|| anyhow!("No .app bundle found in extracted archive"))
}

pub(crate) fn verify_app_structure(app_dir: &Path) -> Result<()> {
    let binaries_dir = app_dir.join("Contents/MacOS");
    if !binaries_dir.is_dir() {
        bail!(
            "Extracted app bundle does not contain expected directory structure (Contents/MacOS)"
        );
    }

    let required_binaries = ["gitbutler-git-askpass", "gitbutler-tauri"];

    for binary in &required_binaries {
        let binary_path = binaries_dir.join(binary);
        if !binary_path.exists() {
            bail!("Missing required binary: {binary}");
        }
    }

    Ok(())
}

fn validate_tarball(path: &Path) -> Result<()> {
    // Check if file is not empty
    let metadata = fs::metadata(path).context("Failed to read tarball metadata")?;
    if metadata.len() == 0 {
        bail!("Downloaded file is empty");
    }

    // Check for gzip magic bytes (1f 8b)
    if !is_gzip_file(path)? {
        bail!("Downloaded file does not appear to be a valid gzip archive");
    }

    Ok(())
}

/// Check if a file has valid gzip magic bytes
fn is_gzip_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).context("Failed to open file")?;
    let mut magic_bytes = [0u8; 2];

    // If we can't read 2 bytes, it's definitely not a valid gzip file
    match file.read_exact(&mut magic_bytes) {
        Ok(_) => Ok(magic_bytes == [0x1f, 0x8b]),
        Err(_) => Ok(false),
    }
}

/// Recursively remove the macOS quarantine attribute from a directory
fn remove_quarantine_recursive(path: &Path) -> Result<()> {
    // Try to remove the quarantine attribute from this path
    // It's OK if it doesn't exist - we just want to ensure it's not there
    let _ = xattr::remove(path, "com.apple.quarantine");

    // If it's a directory, recurse into it
    if path.is_dir() {
        for entry in fs::read_dir(path).context("Failed to read directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            remove_quarantine_recursive(&entry.path())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_is_gzip_file_with_valid_gzip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.gz");

        // Write gzip magic bytes
        let mut file = File::create(&test_file).unwrap();
        file.write_all(&[0x1f, 0x8b, 0x08, 0x00]).unwrap();

        assert!(is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_invalid_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");

        // Write non-gzip content
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.gz");

        // Create empty file
        File::create(&test_file).unwrap();

        // Empty file can't be a valid gzip
        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_is_gzip_file_with_single_byte() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("single.gz");

        // Write only one byte
        let mut file = File::create(&test_file).unwrap();
        file.write_all(&[0x1f]).unwrap();

        // Not enough bytes to be valid gzip
        assert!(!is_gzip_file(&test_file).unwrap());
    }

    #[test]
    fn test_validate_tarball_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("empty.tar.gz");

        // Create empty file
        File::create(&test_file).unwrap();

        // Should fail because file is empty
        assert!(validate_tarball(&test_file).is_err());
    }

    #[test]
    fn test_validate_tarball_non_gzip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.tar.gz");

        // Write non-gzip content
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"Not a gzip file").unwrap();

        // Should fail because it's not gzip
        assert!(validate_tarball(&test_file).is_err());
    }
}
