use anyhow::Context as _;

/// Run Git's `post-checkout` hooks after a branch checkout in `workdir`.
///
/// Requires Git 2.36 or newer due to the use of the `git hook` command. For older Git versions,
/// hooks simply won't execute.
///
/// When `gix-hook` is finished, we should migrate to that instead.
pub fn post_checkout(
    workdir: &std::path::Path,
    old_head: gix::ObjectId,
    new_head: gix::ObjectId,
) -> anyhow::Result<()> {
    post_checkout_with_git(
        gix::path::env::exe_invocation().as_ref(),
        workdir,
        old_head,
        new_head,
    )
}

fn post_checkout_with_git(
    git: &std::ffi::OsStr,
    workdir: &std::path::Path,
    old_head: gix::ObjectId,
    new_head: gix::ObjectId,
) -> anyhow::Result<()> {
    let result = (|| {
        let output = std::process::Command::new(git)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(workdir)
            .args(["hook", "run", "--ignore-missing", "post-checkout", "--"])
            .arg(old_head.to_string())
            .arg(new_head.to_string())
            .arg("1")
            .output()
            .context("Failed to run post-checkout hook")?;
        anyhow::ensure!(
            output.status.success(),
            "Post-checkout hook failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(())
    })();
    if let Err(hook_error) = result {
        match git_version(git) {
            Ok(version) if version < (2, 36) => return Ok(()),
            Ok(_) => return Err(hook_error),
            Err(version_error) => {
                return Err(hook_error).with_context(|| format!("{version_error:#}"));
            }
        }
    }
    Ok(())
}

fn git_version(git: &std::ffi::OsStr) -> anyhow::Result<(u32, u32)> {
    let output = std::process::Command::new(git)
        .arg("--version")
        .output()
        .context("Failed to check Git version")?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to check Git version ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .trim()
        .strip_prefix("git version ")
        .and_then(|version| {
            let mut parts = version.split('.');
            Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
        });
    version.with_context(|| format!("Failed to parse Git version: {}", stdout.trim()))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn run(hook_status: u8, version_command: &str) -> anyhow::Result<()> {
        let dir = but_testsupport::gix_testtools::tempfile::tempdir()?;
        let git = dir.path().join("git");
        std::fs::write(
            &git,
            format!(
                "#!/bin/sh\nif test \"$1\" = --version; then\n  touch \"$0.version-checked\"\n  test -f \"$0.called\" || exit 99\n  {version_command}\nelse\n  touch \"$0.called\"\n  echo hook-error >&2\n  exit {hook_status}\nfi\n"
            ),
        )?;
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755))?;
        let result = post_checkout_with_git(
            git.as_os_str(),
            dir.path(),
            gix::ObjectId::null(gix::hash::Kind::Sha1),
            gix::ObjectId::null(gix::hash::Kind::Sha1),
        );
        assert_eq!(
            dir.path().join("git.version-checked").exists(),
            hook_status != 0,
            "Git version is checked only after hook failure"
        );
        result
    }

    #[test]
    fn old_git_ignores_hook_failure() -> anyhow::Result<()> {
        for version in [
            "2.35.9",
            "2.35.1.windows.1",
            "2.35.1 (Apple Git-154)",
            "1.9.5",
        ] {
            run(1, &format!("echo 'git version {version}'"))?;
        }
        Ok(())
    }

    #[test]
    fn supported_git_preserves_hook_failure() {
        for version in ["2.36.0", "2.54.0", "3.0.0"] {
            let err = run(1, &format!("echo 'git version {version}'")).unwrap_err();
            assert!(
                err.to_string().contains("hook-error"),
                "hook stderr is preserved: {err:#}"
            );
        }
    }

    #[test]
    fn success_does_not_check_version() -> anyhow::Result<()> {
        run(0, "echo unexpected-version-check >&2; exit 1")
    }

    #[test]
    fn launch_failure_preserves_both_errors() -> anyhow::Result<()> {
        let dir = but_testsupport::gix_testtools::tempfile::tempdir()?;
        let err = post_checkout_with_git(
            dir.path().join("missing-git").as_os_str(),
            dir.path(),
            gix::ObjectId::null(gix::hash::Kind::Sha1),
            gix::ObjectId::null(gix::hash::Kind::Sha1),
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(
            message.contains("Failed to run post-checkout hook")
                && message.contains("Failed to check Git version"),
            "both launch errors are preserved: {message}"
        );
        Ok(())
    }

    #[test]
    fn version_failure_preserves_both_errors() {
        for command in ["echo version-error >&2; exit 1", "echo version-error"] {
            let err = run(1, command).unwrap_err();
            let message = format!("{err:#}");
            assert!(
                message.contains("hook-error"),
                "hook error is preserved: {message}"
            );
            assert!(
                message.contains("version-error"),
                "version error is preserved: {message}"
            );
        }
    }
}
