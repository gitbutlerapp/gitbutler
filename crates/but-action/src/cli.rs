use anyhow::{Context as _, anyhow, bail};

pub fn get_cli_path() -> anyhow::Result<std::path::PathBuf> {
    let cli_path = std::env::current_exe()?;
    Ok(if cfg!(feature = "builtin-but") {
        // This is expected to be `tauri`, which also is expected to have `but` capabilities.
        cli_path
    } else {
        cli_path.with_file_name(if cfg!(windows) { "but.exe" } else { "but" })
    })
}

const UNIX_LINK_PATH: &str = "/usr/local/bin/but";

pub enum InstallMode {
    AllowPrivilegeElevation,
    CurrentUserOnly,
}

pub fn do_install_cli(mode: InstallMode) -> anyhow::Result<()> {
    let cli_path = get_cli_path()?;
    if cfg!(windows) {
        bail!(
            "CLI installation is not supported on Windows. Please install manually by placing '{}' in PATH{maybe_new_name}.",
            cli_path.display(),
            maybe_new_name = if cfg!(feature = "builtin-but") {
                " and rename it to but.exe"
            } else {
                ""
            }
        );
    }

    match std::fs::symlink_metadata(UNIX_LINK_PATH) {
        Ok(md) => {
            if !md.is_symlink() {
                bail!(
                    "Refusing to install symlink onto existing non-symlink at '{UNIX_LINK_PATH}'"
                );
            }
            let current_link = std::fs::read_link(UNIX_LINK_PATH)
                .context(format!("error reading existing link: {UNIX_LINK_PATH}"))?;
            if current_link == cli_path {
                return Ok(());
            }
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            #[cfg(not(windows))]
            if std::fs::remove_file(UNIX_LINK_PATH)
                .and_then(|_| std::os::unix::fs::symlink(&cli_path, UNIX_LINK_PATH))
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            #[cfg(not(windows))]
            if std::os::unix::fs::symlink(&cli_path, UNIX_LINK_PATH).is_ok() {
                return Ok(());
            }
        }
        // Also: can happen if the `/usr/local/bin` dir doesn't exist, which then is unlikely to be in PATH anyway.
        Err(err) => return Err(err.into()),
    }

    let can_elevate_privileges = matches!(mode, InstallMode::AllowPrivilegeElevation);
    if cfg!(target_os = "macos") && can_elevate_privileges {
        let status = std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                &format!(
                    "do shell script \" \
                    ln -sf \'{}\' \'{UNIX_LINK_PATH}\' \
                \" with administrator privileges",
                    cli_path.display()
                ),
            ])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to run osascript")?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("error running osascript"))
        }
    } else {
        Err(anyhow!(
            "Would probably need to run \"ln -sf '{}' '{UNIX_LINK_PATH}'\"{privilege}",
            cli_path.display(),
            privilege = if can_elevate_privileges {
                " with root permissions"
            } else {
                ""
            }
        ))
    }
}

fn ensure_cli_path_exists_prior_to_link(cli_path: &std::path::Path) -> anyhow::Result<()> {
    if cli_path.exists() {
        return Ok(());
    }
    bail!("Run `CARGO_TARGET_DIR=$PWD/target/tauri cargo build -p but` to build the `but` binary")
}

pub fn auto_fix_broken_but_cli_symlink() {
    let Ok(absolute_link_destination) = std::fs::read_link(UNIX_LINK_PATH) else {
        return;
    };
    if absolute_link_destination.exists() {
        return;
    }

    match do_install_cli(InstallMode::CurrentUserOnly) {
        Ok(_) => {
            tracing::info!(
                "Successfully fixed symlink at {UNIX_LINK_PATH}, which pointed to non-existing location '{}'",
                absolute_link_destination.display()
            );
        }
        Err(err) => {
            tracing::error!(?err, "Failed to fix symlink at {UNIX_LINK_PATH}");
        }
    }
}
