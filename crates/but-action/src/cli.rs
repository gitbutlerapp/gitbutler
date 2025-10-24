use anyhow::{Context, anyhow, bail};

pub fn get_cli_path() -> anyhow::Result<std::path::PathBuf> {
    let cli_path = std::env::current_exe()?;
    Ok(if cfg!(feature = "builtin-but") {
        // This is expected to be `tauri`, which also is expected to have `but` capabilities.
        cli_path
    } else {
        cli_path.with_file_name(if cfg!(windows) { "but.exe" } else { "but" })
    })
}

pub fn do_install_cli() -> anyhow::Result<()> {
    let cli_path = get_cli_path()?;
    if cfg!(windows) {
        bail!(
            "CLI installation is not supported on Windows. Please install manually by placing '{}' in PATH.",
            cli_path.display()
        );
    }

    let link_path = "/usr/local/bin/but";
    match std::fs::symlink_metadata(link_path) {
        Ok(md) => {
            if !md.is_symlink() {
                bail!("Refusing to install symlink onto existing non-symlink at '{link_path}'");
            }
            let current_link = std::fs::read_link(link_path)
                .context(format!("error reading existing link: {link_path}"))?;
            if current_link == cli_path {
                return Ok(());
            }
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            #[cfg(not(windows))]
            if std::fs::remove_file(link_path)
                .and_then(|_| std::os::unix::fs::symlink(&cli_path, link_path))
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            #[cfg(not(windows))]
            if std::os::unix::fs::symlink(&cli_path, link_path).is_ok() {
                return Ok(());
            }
        }
        // Also: can happen if the `/usr/local/bin` dir doesn't exist, which then is unlikely to be in PATH anyway.
        Err(err) => return Err(err.into()),
    }

    if cfg!(target_os = "macos") {
        let status = std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                &format!(
                    "do shell script \" \
                    ln -sf \'{}\' \'{link_path}\' \
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
            "Would probably need to run \"ln -sf '{}' '{link_path}'\" with root permissions",
            cli_path.display(),
        ))
    }
}

fn ensure_cli_path_exists_prior_to_link(cli_path: &std::path::Path) -> anyhow::Result<()> {
    if cli_path.exists() {
        return Ok(());
    }
    bail!("Run `CARGO_TARGET_DIR=$PWD/target/tauri cargo build -p but` to build the `but` binary")
}
