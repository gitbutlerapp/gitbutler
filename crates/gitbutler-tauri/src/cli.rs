use std::path::Path;

use crate::error::Error;
use anyhow::Context;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn install_cli() -> anyhow::Result<(), Error> {
    let cli_path = get_cli_path()?;
    if cfg!(windows) {
        return Err(anyhow::anyhow!(
            "CLI installation is not supported on Windows. Please install manually by placing '{}' in PATH.", cli_path.display()
        ).into());
    }

    let link_path = Path::new("/usr/local/bin/but");
    match std::fs::symlink_metadata(&link_path) {
        Ok(md) => {
            if !md.is_symlink() {
                return Err(anyhow::anyhow!(
                    "Refusing to install symlink onto existing non-symlink at '{}'",
                    link_path.display()
                )
                .into());
            }
            let current_link = std::fs::read_link(link_path)
                .context(format!(
                    "error reading existing link: {}",
                    link_path.display()
                ))
                .map_err(Error::from)?;
            if current_link == cli_path {
                return Ok(());
            }
            #[cfg(not(windows))]
            if std::fs::remove_file(&link_path)
                .and_then(|_| std::os::unix::fs::symlink(&cli_path, link_path))
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(not(windows))]
            if std::os::unix::fs::symlink(&cli_path, link_path).is_ok() {
                return Ok(());
            }
        }
        // Also: can happen if the `/usr/local/bin` dir doesn't exist, which then is unlikely to be in PATH anyway.
        Err(err) => return Err(anyhow::Error::from(err).into()),
    }

    let bin_dir = link_path.parent().context("Cant find bin dir")?;
    let status = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            &format!(
                "do shell script \" \
                    mkdir -p \'{}\' && \
                    ln -sf \'{}\' \'{}\' \
                \" with administrator privileges",
                bin_dir.to_string_lossy(),
                cli_path.to_string_lossy(),
                link_path.to_string_lossy(),
            ),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run osascript")?;

    if !status.success() {
        return Err(anyhow::anyhow!("error running osascript")).map_err(Error::from);
    }
    Ok(())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn cli_path() -> anyhow::Result<String, Error> {
    let cli_path = get_cli_path()?;
    if !cli_path.exists() {
        return Err(anyhow::anyhow!(
            "CLI path does not exist: {}",
            cli_path.display()
        ))
        .map_err(|e| e.into());
    }
    Ok(cli_path.to_string_lossy().to_string())
}

fn get_cli_path() -> anyhow::Result<std::path::PathBuf> {
    let cli_path = std::env::current_exe()?;
    Ok(cli_path.with_file_name(if cfg!(windows) { "but.exe" } else { "but" }))
}
