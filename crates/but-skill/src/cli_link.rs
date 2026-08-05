//! Installing and inspecting the `but` CLI symlink that lets a terminal
//! reach the binary shipped inside the desktop app.

use anyhow::{Context as _, anyhow, bail};
use but_error::{Code, Context as ErrorContext};

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

/// Where the `but` symlink lives, or `None` on Windows where there is no
/// symlink-based install at all.
///
/// When `E2E_TEST_APP_DATA_DIR` is set the link is redirected under that
/// directory, so tests can exercise install and uninstall without touching
/// `/usr/local/bin` — mirroring [`but_path::home_dir`].
pub fn link_path() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        return None;
    }
    if let Some(test_dir) = std::env::var_os("E2E_TEST_APP_DATA_DIR") {
        return Some(std::path::PathBuf::from(test_dir).join("bin").join("but"));
    }
    Some(std::path::PathBuf::from(UNIX_LINK_PATH))
}

pub enum InstallMode {
    AllowPrivilegeElevation,
    CurrentUserOnly,
}

/// What currently sits at the CLI link path.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum CliLinkStatus {
    /// A symlink pointing at the binary this app would install.
    Installed,
    /// A symlink pointing somewhere else. We refuse to remove it, since it is
    /// most likely another GitButler channel's install.
    InstalledElsewhere {
        /// Where the existing link actually points.
        actual: String,
    },
    /// Nothing at the link path.
    NotInstalled,
    /// A regular file or directory sits there. Never ours to touch — a
    /// package manager's real `but` binary looks exactly like this.
    Blocked,
    /// Windows, where there is no symlink install path.
    Unsupported,
}

/// Everything the UI needs to describe, install, and uninstall the CLI link.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallState {
    /// The `but` binary this app links to.
    pub target_path: String,
    /// Whether that binary is actually present. False in a dev build that
    /// hasn't built `but` yet.
    pub target_exists: bool,
    /// The link location, absent on Windows.
    pub link_path: Option<String>,
    /// What is at [`Self::link_path`] right now.
    pub status: CliLinkStatus,
}

impl CliInstallState {
    /// Whether the CLI is installed and pointing at this app's binary.
    pub fn is_installed(&self) -> bool {
        matches!(self.status, CliLinkStatus::Installed)
    }
}

/// Inspect the CLI link without changing anything.
///
/// Deliberately read-only: [`auto_fix_broken_but_cli_symlink`] already repairs
/// stale links at app startup, and a status query that silently rewrote the
/// filesystem would make the UI's "not installed" state unreproducible.
pub fn cli_install_state() -> anyhow::Result<CliInstallState> {
    let cli_path = get_cli_path()?;
    let target_exists = cli_path.exists();
    let target_path = cli_path.to_string_lossy().to_string();

    let Some(link) = link_path() else {
        return Ok(CliInstallState {
            target_path,
            target_exists,
            link_path: None,
            status: CliLinkStatus::Unsupported,
        });
    };

    let status = link_state(&link, &cli_path)?;

    Ok(CliInstallState {
        target_path,
        target_exists,
        link_path: Some(link.to_string_lossy().to_string()),
        status,
    })
}

/// What sits at `link`, judged against the binary we would install.
///
/// Split out from [`cli_install_state`] so the decision can be tested against
/// temporary paths without redirecting the real link location.
fn link_state(link: &std::path::Path, cli_path: &std::path::Path) -> anyhow::Result<CliLinkStatus> {
    Ok(match std::fs::symlink_metadata(link) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => CliLinkStatus::NotInstalled,
        Err(err) => return Err(err).context(format!("Failed to inspect {}", link.display())),
        Ok(md) if !md.is_symlink() => CliLinkStatus::Blocked,
        Ok(_) => {
            let actual = std::fs::read_link(link)
                .with_context(|| format!("Failed to read link {}", link.display()))?;
            if actual == cli_path {
                CliLinkStatus::Installed
            } else {
                CliLinkStatus::InstalledElsewhere {
                    actual: actual.to_string_lossy().to_string(),
                }
            }
        }
    })
}

/// Remove the `but` CLI symlink.
///
/// Only ever removes a symlink we can identify as ours: one pointing at this
/// app's binary, or a dangling `but` link (the stale-install case
/// [`auto_fix_broken_but_cli_symlink`] exists to repair). A regular file, or a
/// link pointing at some other binary, is left alone and reported instead —
/// deleting a package manager's real `but` would be unrecoverable.
///
/// Returns the resulting state so callers can refresh their UI in one round
/// trip.
pub fn uninstall_cli() -> anyhow::Result<CliInstallState> {
    let Some(link) = link_path() else {
        return cli_install_state();
    };
    uninstall_link(&link, &get_cli_path()?)?;
    cli_install_state()
}

/// The removal decision and action, against explicit paths.
///
/// Returns `false` when there was nothing to remove.
fn uninstall_link(link: &std::path::Path, cli_path: &std::path::Path) -> anyhow::Result<bool> {
    match link_state(link, cli_path)? {
        CliLinkStatus::NotInstalled | CliLinkStatus::Unsupported => return Ok(false),
        CliLinkStatus::Blocked => bail!(
            "Refusing to remove '{}': it is a real file, not a symlink created by GitButler.",
            link.display()
        ),
        CliLinkStatus::InstalledElsewhere { actual } => {
            // A dangling link named `but` is still ours — that is exactly the
            // stale state auto-repair handles. Anything else is not.
            let dangling_but = !std::path::Path::new(&actual).exists()
                && link.file_name().is_some_and(|name| {
                    name == std::ffi::OsStr::new("but") || name == std::ffi::OsStr::new("but.exe")
                });
            if !dangling_but {
                bail!(
                    "Refusing to remove '{}': it points at '{actual}', not at GitButler's `but` binary.",
                    link.display()
                );
            }
        }
        CliLinkStatus::Installed => {}
    }

    // Removing a symlink removes the link itself, never the binary it targets.
    match std::fs::remove_file(link) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            remove_link_with_privileges(link)?;
            Ok(true)
        }
        Err(err) => Err(err).context(format!("Failed to remove {}", link.display())),
    }
}

/// Fall back to an authenticated `rm` when the link directory is not writable.
/// `/usr/local/bin` is usually user-writable after Homebrew, so most users
/// never see the prompt.
fn remove_link_with_privileges(link: &std::path::Path) -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        bail!(
            "Would probably need to run \"rm -f '{}'\" with root permissions",
            link.display()
        );
    }

    let status = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            &format!(
                "do shell script \" rm -f \'{}\' \" with administrator privileges",
                link.display()
            ),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run osascript")?;

    if status.success() {
        Ok(())
    } else if status.code() == Some(1) {
        // Same benign-abort convention as `do_install_cli`: exit 1 means the
        // user dismissed the privileges prompt.
        Err(
            anyhow!("osascript exited with status 1").context(ErrorContext::new_static(
                Code::CliUninstallCancelled,
                "CLI uninstall cancelled",
            )),
        )
    } else {
        Err(anyhow!(
            "osascript exited with status {}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".into())
        ))
    }
}

pub fn do_install_cli(mode: InstallMode) -> anyhow::Result<()> {
    let cli_path = get_cli_path()?;
    #[cfg(windows)]
    {
        return install_cli_windows(cli_path);
    }

    #[cfg(not(windows))]
    let link = link_path().context("No CLI link path on this platform")?;
    #[cfg(not(windows))]
    let link_display = link.display();

    #[cfg(not(windows))]
    match std::fs::symlink_metadata(&link) {
        Ok(md) => {
            if !md.is_symlink() {
                bail!("Refusing to install symlink onto existing non-symlink at '{link_display}'");
            }
            let current_link = std::fs::read_link(&link)
                .context(format!("error reading existing link: {link_display}"))?;
            if current_link == cli_path {
                return Ok(());
            }
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            if std::fs::remove_file(&link)
                .and_then(|_| std::os::unix::fs::symlink(&cli_path, &link))
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            ensure_cli_path_exists_prior_to_link(&cli_path)?;
            // The parent may not exist yet under a redirected test root.
            if let Some(parent) = link.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::os::unix::fs::symlink(&cli_path, &link).is_ok() {
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
        } else if status.code() == Some(1) {
            // osascript exits 1 when the user dismisses the admin-privileges
            // prompt. This is a benign abort, not an error — tag it with a
            // dedicated Code so the frontend can react based on the code
            // rather than matching on an English message.
            Err(
                anyhow!("osascript exited with status 1").context(ErrorContext::new_static(
                    Code::CliInstallCancelled,
                    "CLI install cancelled",
                )),
            )
        } else {
            Err(anyhow!(
                "osascript exited with status {}",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into())
            ))
        }
    } else {
        Err(anyhow!(
            "Would probably need to run \"ln -sf '{}' '{link_display}'\"{privilege}",
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
    let Some(link) = link_path() else {
        return;
    };
    let Ok(absolute_link_destination) = std::fs::read_link(&link) else {
        return;
    };
    if absolute_link_destination.exists() {
        return;
    }

    match do_install_cli(InstallMode::CurrentUserOnly) {
        Ok(_) => {
            tracing::info!(
                "Successfully fixed symlink at {}, which pointed to non-existing location '{}'",
                link.display(),
                absolute_link_destination.display()
            );
        }
        Err(err) => {
            tracing::error!(?err, "Failed to fix symlink at {}", link.display());
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use super::*;

    fn link_to(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    /// The whole point of uninstall: drop the link, keep the binary. Getting
    /// this backwards would delete the user's `but` (or, with `builtin-but`,
    /// the running app itself).
    #[test]
    fn removes_our_symlink_and_leaves_the_target_binary_intact() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("but-binary");
        std::fs::write(&target, b"#!/bin/sh\n").unwrap();
        let link = dir.path().join("but");
        link_to(&link, &target);

        assert!(uninstall_link(&link, &target).unwrap(), "it removed a link");
        assert!(link.symlink_metadata().is_err(), "the symlink is gone");
        assert!(target.is_file(), "the binary it pointed at still exists");
    }

    #[test]
    fn refuses_a_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("but");
        std::fs::write(&link, b"a real binary from a package manager").unwrap();

        let err = uninstall_link(&link, &dir.path().join("but-binary")).unwrap_err();
        assert!(
            err.to_string().contains("not a symlink"),
            "explains why it refused, got: {err}"
        );
        assert!(link.is_file(), "the real binary is untouched");
    }

    #[test]
    fn refuses_a_symlink_pointing_at_another_binary() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("some-other-but");
        std::fs::write(&other, b"#!/bin/sh\n").unwrap();
        let link = dir.path().join("but");
        link_to(&link, &other);

        let err = uninstall_link(&link, &dir.path().join("but-binary")).unwrap_err();
        assert!(
            err.to_string().contains("it points at"),
            "names the unexpected target, got: {err}"
        );
        assert!(link.symlink_metadata().is_ok(), "the link is untouched");
        assert!(other.is_file(), "the other binary is untouched");
    }

    /// A link left behind by a previous install whose binary has since moved.
    /// That is the state `auto_fix_broken_but_cli_symlink` repairs, so it is
    /// unambiguously ours to remove.
    #[test]
    fn removes_a_dangling_but_link() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("but");
        link_to(&link, &dir.path().join("gone-away"));

        assert!(uninstall_link(&link, &dir.path().join("but-binary")).unwrap());
        assert!(link.symlink_metadata().is_err(), "the stale link is gone");
    }

    #[test]
    fn is_a_no_op_when_nothing_is_installed() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("but");

        assert!(
            !uninstall_link(&link, &dir.path().join("but-binary")).unwrap(),
            "reports that there was nothing to remove"
        );
    }

    #[test]
    fn link_state_distinguishes_ours_from_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("but-binary");
        std::fs::write(&target, b"#!/bin/sh\n").unwrap();
        let link = dir.path().join("but");

        assert_eq!(
            link_state(&link, &target).unwrap(),
            CliLinkStatus::NotInstalled
        );

        link_to(&link, &target);
        assert_eq!(
            link_state(&link, &target).unwrap(),
            CliLinkStatus::Installed
        );
    }
}
