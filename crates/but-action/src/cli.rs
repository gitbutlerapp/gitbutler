#[cfg(any(target_os = "macos", all(test, unix)))]
use anyhow::{Context as _, anyhow};
#[cfg(any(target_os = "macos", all(test, unix)))]
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

#[cfg(all(test, unix))]
mod tests;

#[cfg(any(target_os = "macos", all(test, unix)))]
const UNIX_LINK_PATH: &str = "/usr/local/bin/but";

pub enum InstallMode {
    AllowPrivilegeElevation,
    CurrentUserOnly,
}

/// Whether installation may replace an existing symlink to another executable.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum ExistingSymlinkPolicy {
    /// Report a conflict without changing the existing link.
    Refuse,
    /// Replace existing symlinks, including dangling ones, but not files or directories.
    Replace,
}

/// Install an explicitly supplied bundled CLI at `/usr/local/bin/but` on macOS.
///
/// The caller must supply an absolute executable path from a stable app installation,
/// not renderer input or a mounted disk image. Existing symlinks are handled according
/// to `symlink_policy`; files and directories are refused. Authorization cancellation
/// is reported as [`Code::CliInstallCancelled`].
/// This does not discover the CLI or change shell configuration.
#[cfg(any(target_os = "macos", all(test, unix)))]
pub fn do_install_cli_v2(
    cli_path: &std::path::Path,
    mode: InstallMode,
    symlink_policy: ExistingSymlinkPolicy,
) -> anyhow::Result<()> {
    let destination = std::path::Path::new(UNIX_LINK_PATH);
    match install_cli_link(cli_path, destination, symlink_policy) {
        Ok(()) => Ok(()),
        Err(InstallError::InstallationRequiresElevatedPrivileges(err)) => match mode {
            InstallMode::AllowPrivilegeElevation => {
                install_cli_link_escalated(cli_path, destination, symlink_policy)
            }
            InstallMode::CurrentUserOnly => Err(err)
                .context("Privilege escalation required but not allowed under user install mode"),
        },
        Err(InstallError::Other(err)) => Err(err),
    }
}

/// Installs the CLI link with escalated privileges.
#[cfg(any(target_os = "macos", all(test, unix)))]
fn install_cli_link_escalated(
    cli_path: &std::path::Path,
    destination: &std::path::Path,
    symlink_policy: ExistingSymlinkPolicy,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        destination.is_absolute(),
        "CLI destination must be absolute"
    );
    let directory = destination
        .parent()
        .context("CLI destination has no parent")?;
    // osascript accepts text arguments, so reject non-UTF-8 instead of changing paths.
    let source = cli_path.to_str().context("CLI path is not valid UTF-8")?;
    let target = destination
        .to_str()
        .context("CLI destination is not valid UTF-8")?;
    let directory = directory
        .to_str()
        .context("CLI destination directory is not valid UTF-8")?;
    // The script's destination check and `ln` are not atomic: if a directory appears at the
    // destination between them, `ln` can create a link inside it. Verification below rejects that
    // result, but the stray link remains. Avoiding this race would require an elevated helper
    // calling symlink(2) directly instead of `ln`. That seems a bit overkill for now.
    let policy = match symlink_policy {
        ExistingSymlinkPolicy::Refuse => "refuse",
        ExistingSymlinkPolicy::Replace => "replace",
    };
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", INSTALL_CLI_SCRIPT, source, target, directory, policy])
        .output()
        .context("Failed to request administrator authorization for CLI installation")?;
    check_cli_install_output(output)?;
    verify_cli_link(cli_path, destination)
}

/// Specific errors returned by [`install_cli_link`].
#[cfg(any(target_os = "macos", all(test, unix)))]
#[derive(Debug)]
enum InstallError {
    /// The install failed because of insufficient privileges - escalation recommended to proceed.
    InstallationRequiresElevatedPrivileges(anyhow::Error),
    /// Any other error - we don't act on this.
    Other(anyhow::Error),
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn install_cli_link(
    source: &std::path::Path,
    destination: &std::path::Path,
    symlink_policy: ExistingSymlinkPolicy,
) -> Result<(), InstallError> {
    use std::fs;

    validate_cli_source(source).map_err(InstallError::Other)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if !metadata.is_symlink() {
                return Err(InstallError::Other(anyhow::anyhow!(
                    "Refusing to replace non-symlink file '{}'",
                    destination.display()
                )));
            }
            let target = fs::read_link(destination)
                .context("Cannot read existing CLI symlink")
                .map_err(InstallError::Other)?;
            if target == source || matches!(symlink_policy, ExistingSymlinkPolicy::Refuse) {
                return verify_cli_link(source, destination).map_err(InstallError::Other);
            }
            fs::remove_file(destination)
                .context("Cannot remove existing CLI symlink")
                .map_err(escalate_privilege_error_if_permission_denied)?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err)
                .context("Cannot inspect CLI installation destination")
                .map_err(InstallError::Other);
        }
    }
    fs::create_dir_all(
        destination
            .parent()
            .context("CLI destination has no parent")
            .map_err(InstallError::Other)?,
    )
    .context("Cannot create CLI installation directory")
    .map_err(escalate_privilege_error_if_permission_denied)?;
    std::os::unix::fs::symlink(source, destination)
        .context("Cannot create CLI symlink; existing entries will not be replaced")
        .map_err(escalate_privilege_error_if_permission_denied)?;
    verify_cli_link(source, destination).map_err(InstallError::Other)?;
    Ok(())
}

/// Map the error to escalate privileges if permission was denied.
#[cfg(any(target_os = "macos", all(test, unix)))]
fn escalate_privilege_error_if_permission_denied(err: anyhow::Error) -> InstallError {
    match err.downcast_ref::<std::io::Error>() {
        Some(io_err) if io_err.kind() == std::io::ErrorKind::PermissionDenied => {
            InstallError::InstallationRequiresElevatedPrivileges(err)
        }
        _ => InstallError::Other(err),
    }
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn validate_cli_source(source: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    anyhow::ensure!(source.is_absolute(), "CLI source path must be absolute");
    let metadata = std::fs::metadata(source)
        .with_context(|| format!("Cannot access CLI executable at '{}'", source.display()))?;
    anyhow::ensure!(
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        "CLI source '{}' must be an executable file",
        source.display()
    );
    Ok(())
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn verify_cli_link(source: &std::path::Path, destination: &std::path::Path) -> anyhow::Result<()> {
    let target = std::fs::read_link(destination).context("Cannot read installed CLI symlink")?;
    anyhow::ensure!(
        target == source,
        "Refusing to replace '{}', which points to '{}' instead of '{}'",
        destination.display(),
        target.display(),
        source.display()
    );
    Ok(())
}

#[cfg(any(target_os = "macos", all(test, unix)))]
const INSTALL_CLI_SCRIPT: &str = r#"
on run argv
    set sourcePath to quoted form of (item 1 of argv)
    set targetPath to quoted form of (item 2 of argv)
    set targetDirectory to quoted form of (item 3 of argv)
    set symlinkPolicy to quoted form of (item 4 of argv)
    try
        do shell script ("/bin/mkdir -p " & targetDirectory & " || exit $?; " & ¬
            "if [ -L " & targetPath & " ] && [ " & symlinkPolicy & " = replace ]; then " & ¬
            "/bin/rm " & targetPath & " || exit $?; " & ¬
            "elif [ -e " & targetPath & " ] || [ -L " & targetPath & " ]; then " & ¬
            "echo 'CLI destination already exists' >&2; exit 1; fi; " & ¬
            "/bin/ln -s -h " & sourcePath & " " & targetPath) ¬
            with prompt "GitButler needs administrator access to install but into /usr/local/bin." ¬
            with administrator privileges
        return "gitbutler-cli-installed"
    on error messageText number errorNumber
        if errorNumber is -128 then return "gitbutler-cli-install-cancelled"
        error messageText number errorNumber
    end try
end run
"#;

#[cfg(any(target_os = "macos", all(test, unix)))]
fn check_cli_install_output(output: std::process::Output) -> anyhow::Result<()> {
    anyhow::ensure!(
        output.status.success(),
        "CLI installation failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    if output.stdout == b"gitbutler-cli-install-cancelled\n" {
        return Err(
            anyhow!("Administrator authorization was cancelled").context(ErrorContext::new_static(
                Code::CliInstallCancelled,
                "CLI install cancelled",
            )),
        );
    }
    anyhow::ensure!(
        output.stdout == b"gitbutler-cli-installed\n",
        "Unexpected CLI installer response"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn auto_fix_broken_but_cli_symlink() {
    let Ok(absolute_link_destination) = std::fs::read_link(UNIX_LINK_PATH) else {
        return;
    };
    if absolute_link_destination.exists() {
        return;
    }

    let result = get_cli_path().and_then(|source| {
        do_install_cli_v2(
            &source,
            InstallMode::CurrentUserOnly,
            ExistingSymlinkPolicy::Replace,
        )
    });
    match result {
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
