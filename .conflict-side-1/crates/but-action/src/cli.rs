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
    #[cfg(windows)]
    {
        return install_cli_windows(cli_path);
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

/// On Windows, we'll provide helpful instructions rather than attempt automatic installation
/// since:
/// 1. Creating symlinks requires developer mode or admin privileges
/// 2. There's no standard user-writable directory that's always in PATH
/// 3. Users typically add directories to PATH manually on Windows
///
/// Note that this isn't usually called on Windows.
#[cfg(windows)]
fn install_cli_windows(cli_path: std::path::PathBuf) -> anyhow::Result<()> {
    let but_filename = cli_path
        .file_name()
        .context("BUG: encountered but CLI path without /")?;

    bail!(
        "Automatic CLI installation is not supported on Windows.\n\
        \n\
        To use the But CLI, you have two options:\n\
        \n\
        1. Copy the executable to a directory in your PATH:\n\
           copy \"{}\" \"%LOCALAPPDATA%\\Microsoft\\WindowsApps\\{}\"\n\
        \n\
        2. Add the current location to your PATH environment variable:\n\
           - Press the Win key and select 'System'\n\
           - Type 'Environment' into the search box and select 'edit variables for your account'\n\
           - Under 'User variables', select 'Path' and click 'Edit'\n\
           - Click 'New' and add: {}\n\
        \n\
        After either option, restart your terminal to use the 'but' command.",
        cli_path.display(),
        but_filename.display(),
        cli_path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| cli_path.display().to_string())
    );
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
