//! Tests for GitButler managed hooks installation and cleanup
//!
//! These tests verify safety-critical behavior around hook management:
//! - Idempotency of install/uninstall operations
//! - Backup and restore semantics
//! - Protection of non-GitButler hooks from being overwritten or removed

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use anyhow::Result;
use gitbutler_repo::managed_hooks::{
    HookInstallationResult, install_managed_hooks, install_managed_hooks_with,
    uninstall_managed_hooks,
};
use tempfile::TempDir;

/// Helper to create a test git repository
fn create_test_repo() -> Result<(TempDir, gix::Repository)> {
    let tmp = TempDir::new()?;
    let repo = gix::init(tmp.path())?;
    // Re-open in isolated mode, it's a bit hard to pass open-options to `gix::init()`.
    but_testsupport::open_repo(repo.path()).map(|repo| (tmp, repo))
}

fn hooks_dir(repo: &gix::Repository) -> std::path::PathBuf {
    repo.git_dir().join("hooks")
}

/// Helper to create a user hook file with content
fn create_user_hook(repo: &gix::Repository, hook_name: &str, content: &str) -> Result<()> {
    let hooks_dir = hooks_dir(repo);
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join(hook_name);
    fs::write(&hook_path, content)?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    Ok(())
}

/// Helper to check if a file exists
fn hook_exists(repo: &gix::Repository, hook_name: &str) -> bool {
    hooks_dir(repo).join(hook_name).exists()
}

/// Helper to read hook content
fn read_hook(repo: &gix::Repository, hook_name: &str) -> Result<String> {
    let path = hooks_dir(repo).join(hook_name);
    Ok(fs::read_to_string(path)?)
}

fn hook_exists_at(hooks_dir: &std::path::Path, hook_name: &str) -> bool {
    hooks_dir.join(hook_name).exists()
}

/// Helper to check if hook is executable on Unix
#[cfg(unix)]
fn is_executable(repo: &gix::Repository, hook_name: &str) -> bool {
    let path = hooks_dir(repo).join(hook_name);
    if let Ok(metadata) = fs::metadata(&path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[test]
fn test_install_hooks_creates_hooks_directory() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Remove hooks directory if it exists
    let hooks_dir = hooks_dir(&repo);
    if hooks_dir.exists() {
        fs::remove_dir_all(&hooks_dir)?;
    }

    install_managed_hooks_with(&repo, false)?;

    assert!(hooks_dir.exists(), "Hooks directory should be created");
    Ok(())
}

#[test]
fn test_install_creates_pre_commit_and_post_checkout_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks_with(&repo, false)?;

    assert!(
        hook_exists(&repo, "pre-commit"),
        "pre-commit hook should exist"
    );
    assert!(
        hook_exists(&repo, "post-checkout"),
        "post-checkout hook should exist"
    );
    Ok(())
}

#[test]
fn test_installed_hooks_have_gitbutler_signature() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks_with(&repo, false)?;

    let pre_commit = read_hook(&repo, "pre-commit")?;
    let post_checkout = read_hook(&repo, "post-checkout")?;

    assert!(
        pre_commit.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "pre-commit should have signature"
    );
    assert!(
        post_checkout.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "post-checkout should have signature"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_installed_hooks_are_executable() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks_with(&repo, false)?;

    assert!(
        is_executable(&repo, "pre-commit"),
        "pre-commit should be executable"
    );
    assert!(
        is_executable(&repo, "post-checkout"),
        "post-checkout should be executable"
    );
    Ok(())
}

#[test]
fn test_install_is_idempotent() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Install twice
    let result1 = install_managed_hooks_with(&repo, false)?;
    let result2 = install_managed_hooks_with(&repo, false)?;

    // First install should succeed
    assert!(matches!(result1, HookInstallationResult::Success));

    // Second install should detect already configured
    assert!(matches!(result2, HookInstallationResult::AlreadyConfigured));

    // Hooks should still exist and be valid
    assert!(hook_exists(&repo, "pre-commit"));
    assert!(hook_exists(&repo, "post-checkout"));
    Ok(())
}

#[test]
fn test_install_backs_up_existing_user_hook() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    let user_hook_content =
        "#!/bin/sh\n# User's custom pre-commit hook\necho 'Running user hook'\n";
    create_user_hook(&repo, "pre-commit", user_hook_content)?;

    install_managed_hooks_with(&repo, false)?;

    // Original hook should be backed up
    assert!(
        hook_exists(&repo, "pre-commit-user"),
        "User hook should be backed up"
    );
    let backup_content = read_hook(&repo, "pre-commit-user")?;
    assert_eq!(
        backup_content, user_hook_content,
        "Backup should have original content"
    );

    // New hook should be GitButler managed
    let new_hook = read_hook(&repo, "pre-commit")?;
    assert!(
        new_hook.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "New hook should be GitButler managed"
    );
    Ok(())
}

#[test]
fn test_install_does_not_overwrite_existing_backup() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    let original_backup = "#!/bin/sh\n# Original user hook\necho 'original'\n";
    let new_hook = "#!/bin/sh\n# New hook\necho 'new'\n";

    // Create original backup
    create_user_hook(&repo, "pre-commit-user", original_backup)?;

    // Create a new hook (not GitButler managed)
    create_user_hook(&repo, "pre-commit", new_hook)?;

    // Install GitButler hooks - should NOT overwrite the backup
    install_managed_hooks_with(&repo, false)?;

    // Backup should still have original content
    let backup_content = read_hook(&repo, "pre-commit-user")?;
    assert_eq!(
        backup_content, original_backup,
        "Backup should not be overwritten"
    );
    Ok(())
}

#[test]
fn test_uninstall_removes_managed_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks_with(&repo, false)?;
    assert!(hook_exists(&repo, "pre-commit"));
    assert!(hook_exists(&repo, "post-checkout"));

    uninstall_managed_hooks(&repo)?;

    assert!(
        !hook_exists(&repo, "pre-commit"),
        "pre-commit should be removed"
    );
    assert!(
        !hook_exists(&repo, "post-checkout"),
        "post-checkout should be removed"
    );
    Ok(())
}

#[test]
fn test_uninstall_restores_user_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    let user_hook_content = "#!/bin/sh\n# User's custom hook\necho 'user hook'\n";
    create_user_hook(&repo, "pre-commit", user_hook_content)?;

    // Install GitButler hooks (backs up user hook)
    install_managed_hooks_with(&repo, false)?;
    assert!(hook_exists(&repo, "pre-commit-user"));

    // Uninstall should restore the backup
    uninstall_managed_hooks(&repo)?;

    assert!(
        hook_exists(&repo, "pre-commit"),
        "User hook should be restored"
    );
    assert!(
        !hook_exists(&repo, "pre-commit-user"),
        "Backup should be removed after restore"
    );

    let restored_content = read_hook(&repo, "pre-commit")?;
    assert_eq!(
        restored_content, user_hook_content,
        "Restored hook should have original content"
    );
    Ok(())
}

#[test]
fn test_uninstall_does_not_remove_non_managed_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Create a non-GitButler hook
    let user_hook = "#!/bin/sh\n# Not a GitButler hook\necho 'user hook'\n";
    create_user_hook(&repo, "pre-commit", user_hook)?;

    // Try to uninstall - should not remove the hook
    let result = uninstall_managed_hooks(&repo)?;

    // Hook should still exist
    assert!(
        hook_exists(&repo, "pre-commit"),
        "Non-managed hook should not be removed"
    );
    let content = read_hook(&repo, "pre-commit")?;
    assert_eq!(content, user_hook, "Hook content should be unchanged");

    // Should report already configured (skipped)
    assert!(matches!(
        result,
        HookInstallationResult::AlreadyConfigured | HookInstallationResult::Success
    ));
    Ok(())
}

#[test]
fn test_uninstall_is_idempotent() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks_with(&repo, false)?;

    // Uninstall twice
    let result1 = uninstall_managed_hooks(&repo)?;
    let result2 = uninstall_managed_hooks(&repo)?;

    // Both should succeed or report no work to do
    assert!(matches!(result1, HookInstallationResult::Success));
    assert!(matches!(
        result2,
        HookInstallationResult::Success | HookInstallationResult::AlreadyConfigured
    ));
    Ok(())
}

#[test]
fn test_install_uninstall_roundtrip_with_user_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    let original_pre_commit = "#!/bin/sh\n# Original pre-commit\necho 'pre-commit'\n";
    let original_post_checkout = "#!/bin/sh\n# Original post-checkout\necho 'post-checkout'\n";

    // Create user hooks
    create_user_hook(&repo, "pre-commit", original_pre_commit)?;
    create_user_hook(&repo, "post-checkout", original_post_checkout)?;

    // Install GitButler hooks
    install_managed_hooks_with(&repo, false)?;

    // Verify GitButler hooks are installed
    let installed_pre = read_hook(&repo, "pre-commit")?;
    assert!(installed_pre.contains("GITBUTLER_MANAGED_HOOK_V1"));

    // Verify backups exist
    assert!(hook_exists(&repo, "pre-commit-user"));
    assert!(hook_exists(&repo, "post-checkout-user"));

    // Uninstall
    uninstall_managed_hooks(&repo)?;

    // Verify original hooks are restored
    let restored_pre = read_hook(&repo, "pre-commit")?;
    let restored_post = read_hook(&repo, "post-checkout")?;
    assert_eq!(
        restored_pre, original_pre_commit,
        "pre-commit should be restored"
    );
    assert_eq!(
        restored_post, original_post_checkout,
        "post-checkout should be restored"
    );

    // Verify backups are gone
    assert!(!hook_exists(&repo, "pre-commit-user"));
    assert!(!hook_exists(&repo, "post-checkout-user"));
    Ok(())
}

#[test]
fn test_multiple_install_uninstall_cycles() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    let user_hook = "#!/bin/sh\necho 'user hook'\n";
    create_user_hook(&repo, "pre-commit", user_hook)?;

    // Cycle 1
    install_managed_hooks_with(&repo, false)?;
    uninstall_managed_hooks(&repo)?;

    // Cycle 2
    install_managed_hooks_with(&repo, false)?;
    uninstall_managed_hooks(&repo)?;

    // Cycle 3
    install_managed_hooks_with(&repo, false)?;
    uninstall_managed_hooks(&repo)?;

    // User hook should still be intact
    assert!(hook_exists(&repo, "pre-commit"));
    let content = read_hook(&repo, "pre-commit")?;
    assert_eq!(
        content, user_hook,
        "User hook should survive multiple cycles"
    );
    Ok(())
}

#[test]
fn test_hook_manually_modified_after_install() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Install GitButler hooks
    install_managed_hooks_with(&repo, false)?;

    // User manually modifies the hook
    let modified_hook = "#!/bin/sh\n# User modified this\necho 'modified'\n";
    let hook_path = hooks_dir(&repo).join("pre-commit");
    fs::write(&hook_path, modified_hook)?;

    // Uninstall should not remove the modified hook (no signature)
    uninstall_managed_hooks(&repo)?;

    // Hook should still exist with user's modifications
    assert!(hook_exists(&repo, "pre-commit"));
    let content = read_hook(&repo, "pre-commit")?;
    assert_eq!(content, modified_hook, "Modified hook should be preserved");
    Ok(())
}

#[test]
fn test_respects_absolute_core_hooks_path() -> Result<()> {
    let (_temp, mut repo) = create_test_repo()?;

    // Create a custom hooks directory
    let custom_hooks = _temp.path().join("custom-hooks");
    fs::create_dir_all(&custom_hooks)?;

    // Configure core.hooksPath
    repo.config_snapshot_mut().set_raw_value(
        "core.hooksPath",
        gix::path::into_bstr(&custom_hooks).as_ref(),
    )?;

    // Install hooks
    install_managed_hooks_with(&repo, false)?;

    // Hooks should be in custom directory, not .git/hooks
    assert!(
        custom_hooks.join("pre-commit").exists(),
        "Hook should be in custom directory"
    );
    assert!(
        custom_hooks.join("post-checkout").exists(),
        "Hook should be in custom directory"
    );

    // Should NOT be in default .git/hooks
    assert!(
        !hooks_dir(&repo).join("pre-commit").exists(),
        "Hook should not be in default location"
    );
    Ok(())
}

#[test]
fn test_respects_relative_core_hooks_path() -> Result<()> {
    let (_temp, mut repo) = create_test_repo()?;

    let relative_hooks = format!(
        "custom-hooks-{}",
        _temp
            .path()
            .file_name()
            .expect("temp dir name")
            .to_string_lossy()
    );
    let expected_hooks_dir = _temp.path().join(&relative_hooks);

    repo.config_snapshot_mut()
        .set_raw_value("core.hooksPath", relative_hooks.as_str())?;

    install_managed_hooks_with(&repo, false)?;

    assert!(
        hook_exists_at(&expected_hooks_dir, "pre-commit"),
        "Hook should be resolved relative to the worktree root"
    );
    assert!(
        hook_exists_at(&expected_hooks_dir, "post-checkout"),
        "Hook should be resolved relative to the worktree root"
    );
    assert!(
        !hooks_dir(&repo).join("pre-commit").exists(),
        "Hook should not be installed in the default location"
    );
    Ok(())
}

#[test]
fn test_uninstall_respects_relative_core_hooks_path() -> Result<()> {
    let (_temp, mut repo) = create_test_repo()?;

    let relative_hooks = format!(
        "custom-hooks-uninstall-{}",
        _temp
            .path()
            .file_name()
            .expect("temp dir name")
            .to_string_lossy()
    );
    let expected_hooks_dir = _temp.path().join(&relative_hooks);

    repo.config_snapshot_mut()
        .set_raw_value("core.hooksPath", relative_hooks.as_str())?;
    install_managed_hooks_with(&repo, false)?;

    assert!(
        hook_exists_at(&expected_hooks_dir, "pre-commit"),
        "Hook should be resolved relative to the worktree root"
    );
    assert!(
        hook_exists_at(&expected_hooks_dir, "post-checkout"),
        "Hook should be resolved relative to the worktree root"
    );

    uninstall_managed_hooks(&repo)?;

    assert!(
        !hook_exists_at(&expected_hooks_dir, "pre-commit"),
        "Uninstall should remove the managed pre-commit hook from the configured directory"
    );
    assert!(
        !hook_exists_at(&expected_hooks_dir, "post-checkout"),
        "Uninstall should remove the managed post-checkout hook from the configured directory"
    );
    Ok(())
}

#[test]
fn test_partial_installation_with_one_existing_hook() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Create only pre-commit user hook
    let user_hook = "#!/bin/sh\necho 'user pre-commit'\n";
    create_user_hook(&repo, "pre-commit", user_hook)?;

    // Install (should back up pre-commit, create new post-checkout)
    install_managed_hooks_with(&repo, false)?;

    assert!(hook_exists(&repo, "pre-commit"));
    assert!(hook_exists(&repo, "pre-commit-user"));
    assert!(hook_exists(&repo, "post-checkout"));
    assert!(
        !hook_exists(&repo, "post-checkout-user"),
        "No backup for post-checkout"
    );

    // Uninstall should restore pre-commit, remove post-checkout
    uninstall_managed_hooks(&repo)?;

    assert!(
        hook_exists(&repo, "pre-commit"),
        "pre-commit should be restored"
    );
    assert!(
        !hook_exists(&repo, "pre-commit-user"),
        "Backup should be removed"
    );
    assert!(
        !hook_exists(&repo, "post-checkout"),
        "post-checkout should be removed"
    );
    Ok(())
}

#[test]
fn test_hook_with_shebang_variations() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Create hooks with different shebangs
    create_user_hook(
        &repo,
        "pre-commit",
        "#!/usr/bin/env bash\necho 'bash hook'\n",
    )?;
    create_user_hook(&repo, "post-checkout", "#!/bin/bash\necho 'bash hook'\n")?;

    install_managed_hooks_with(&repo, false)?;

    // Verify backups preserved shebangs
    let pre_backup = read_hook(&repo, "pre-commit-user")?;
    let post_backup = read_hook(&repo, "post-checkout-user")?;

    assert!(pre_backup.starts_with("#!/usr/bin/env bash"));
    assert!(post_backup.starts_with("#!/bin/bash"));

    uninstall_managed_hooks(&repo)?;

    // Verify restored hooks still have correct shebangs
    let pre_restored = read_hook(&repo, "pre-commit")?;
    let post_restored = read_hook(&repo, "post-checkout")?;

    assert!(pre_restored.starts_with("#!/usr/bin/env bash"));
    assert!(post_restored.starts_with("#!/bin/bash"));
    Ok(())
}

#[test]
fn test_empty_hooks_directory() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Ensure hooks directory exists but is empty
    let hooks_dir = hooks_dir(&repo);
    fs::create_dir_all(&hooks_dir)?;

    // Should install cleanly
    let result = install_managed_hooks_with(&repo, false)?;
    assert!(matches!(result, HookInstallationResult::Success));

    assert!(hook_exists(&repo, "pre-commit"));
    assert!(hook_exists(&repo, "post-checkout"));
    Ok(())
}

#[test]
fn test_concurrent_installs_with_backup_present() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Simulate a scenario where backup already exists (from previous install)
    let backup_content = "#!/bin/sh\necho 'original backup'\n";
    create_user_hook(&repo, "pre-commit-user", backup_content)?;

    // Create a new hook that's different
    let new_hook = "#!/bin/sh\necho 'new hook'\n";
    create_user_hook(&repo, "pre-commit", new_hook)?;

    // Install should not overwrite the existing backup
    install_managed_hooks_with(&repo, false)?;

    let backup = read_hook(&repo, "pre-commit-user")?;
    assert_eq!(
        backup, backup_content,
        "Existing backup should not be modified"
    );
    Ok(())
}

// ---- Config-based hooks (git 2.54+) ----
//
// These tests drive `install_managed_hooks_with(repo, true)` explicitly so they
// are independent of the git version on `PATH`. The tests that execute real git
// commands skip themselves when git is older than 2.54.

use std::path::Path;
use std::process::Command;

/// Run git in `repo_dir` with host configuration masked out.
fn git_in(repo_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("failed to run git")
}

fn git_ok(repo_dir: &Path, args: &[&str]) -> String {
    let out = git_in(repo_dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Read a key from the repo's local git config, if set.
fn local_config(repo_dir: &Path, key: &str) -> Option<String> {
    let out = git_in(repo_dir, &["config", "--local", "--get", key]);
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn config_scripts_dir(repo: &gix::Repository) -> std::path::PathBuf {
    repo.common_dir().join("gitbutler/hooks")
}

fn workdir(repo: &gix::Repository) -> std::path::PathBuf {
    repo.workdir()
        .expect("test repos have a worktree")
        .to_owned()
}

/// `true` if the `git` on `PATH` runs config-based hooks (2.54+).
fn real_git_supports_config_hooks() -> bool {
    let out = Command::new("git")
        .arg("--version")
        .output()
        .expect("git is on PATH");
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(rest) = text.trim().strip_prefix("git version ") else {
        return false;
    };
    let mut numbers = rest.split(|c: char| !c.is_ascii_digit());
    match (
        numbers.next().and_then(|s| s.parse::<u32>().ok()),
        numbers.next().and_then(|s| s.parse::<u32>().ok()),
    ) {
        (Some(major), Some(minor)) => (major, minor) >= (2, 54),
        _ => false,
    }
}

#[test]
fn test_config_install_registers_hooks_without_touching_hooks_dir() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    // A hook-manager-style hook occupies the pre-commit slot.
    let manager_hook = "#!/bin/sh\n# File generated by prek\nexec prek hook-impl pre-commit\n";
    create_user_hook(&repo, "pre-commit", manager_hook)?;

    let result = install_managed_hooks_with(&repo, true)?;
    assert!(matches!(result, HookInstallationResult::Success));

    // Config entries point at GitButler-owned scripts.
    for (section, event) in [
        ("gitbutler-pre-commit", "pre-commit"),
        ("gitbutler-post-checkout", "post-checkout"),
    ] {
        assert_eq!(
            local_config(&dir, &format!("hook.{section}.event")).as_deref(),
            Some(event),
            "{section} should be registered for its event"
        );
        let command = local_config(&dir, &format!("hook.{section}.command"))
            .unwrap_or_else(|| panic!("{section} should have a command"));
        assert!(
            command.contains("gitbutler/hooks"),
            "command should point into the GitButler scripts dir: {command}"
        );
    }

    // Scripts exist and carry the GitButler signature.
    for name in ["pre-commit", "post-checkout"] {
        let script = config_scripts_dir(&repo).join(name);
        assert!(script.exists(), "{name} script should exist");
        assert!(fs::read_to_string(&script)?.contains("GITBUTLER_MANAGED_HOOK_V1"));
    }

    // The hooks directory is untouched: the manager's hook is byte-identical,
    // no backup was created, and no GitButler hook occupies any slot.
    assert_eq!(read_hook(&repo, "pre-commit")?, manager_hook);
    assert!(!hook_exists(&repo, "pre-commit-user"));
    assert!(!hook_exists(&repo, "post-checkout"));

    Ok(())
}

/// If another tool (a future prek/husky/etc.) registers its own hook through
/// git config for the same event, GitButler's install/uninstall must not
/// touch that tool's section at all. Config-based hooks are keyed by section
/// name, not a shared slot, so this should hold by construction — this test
/// exists to document and pin that guarantee.
#[test]
fn test_config_install_does_not_touch_other_config_based_hooks() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    // Simulate a different tool that already registered its own config-based
    // hook for the same event, under its own section name.
    let other_command = "/opt/homebrew/bin/some-other-tool hook-impl pre-commit";
    for (key, value) in [
        ("hook.some-other-tool.event", "pre-commit"),
        ("hook.some-other-tool.command", other_command),
    ] {
        let out = git_in(&dir, &["config", "--local", key, value]);
        assert!(out.status.success(), "failed to set {key}: {out:?}");
    }

    let result = install_managed_hooks_with(&repo, true)?;
    assert!(matches!(result, HookInstallationResult::Success));

    // GitButler registered its own section...
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.event").as_deref(),
        Some("pre-commit"),
        "GitButler's own hook should still be registered"
    );

    // ...without touching the other tool's section at all.
    assert_eq!(
        local_config(&dir, "hook.some-other-tool.event").as_deref(),
        Some("pre-commit"),
        "other tool's hook registration should be left alone"
    );
    assert_eq!(
        local_config(&dir, "hook.some-other-tool.command").as_deref(),
        Some(other_command),
        "other tool's hook command should be untouched"
    );

    // Uninstalling GitButler's hooks must not remove the other tool's entry either.
    uninstall_managed_hooks(&repo)?;
    assert_eq!(
        local_config(&dir, "hook.some-other-tool.event").as_deref(),
        Some("pre-commit"),
        "other tool's hook registration should survive GitButler's uninstall"
    );
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.event"),
        None,
        "GitButler's own hook should be gone after uninstall"
    );

    Ok(())
}

/// Config-based hook registration must work correctly even when a hook
/// manager has redirected `core.hooksPath` elsewhere (e.g. Husky's `.husky`
/// convention). Registration is independent of `core.hooksPath` -- this test
/// pins that the redirected directory (and whatever lives in it) is
/// inspected only for legacy-signature cleanup purposes and otherwise left
/// completely alone.
#[test]
fn test_config_install_coexists_with_custom_core_hooks_path() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    // Simulate a Husky-style redirect: hooks live in a custom directory, with
    // a manager-owned hook already installed there.
    let custom_hooks_dir = dir.join(".husky");
    fs::create_dir_all(&custom_hooks_dir)?;
    let manager_hook = "#!/bin/sh\n# husky\nexit 0\n";
    fs::write(custom_hooks_dir.join("pre-commit"), manager_hook)?;
    git_in(&dir, &["config", "--local", "core.hooksPath", ".husky"]);
    let repo = but_testsupport::open_repo(repo.path())?;

    let result = install_managed_hooks_with(&repo, true)?;
    assert!(matches!(result, HookInstallationResult::Success));

    // GitButler's registration succeeded, independent of core.hooksPath.
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.event").as_deref(),
        Some("pre-commit"),
        "GitButler's own hook should register even with a custom core.hooksPath"
    );

    // The redirected hooks directory (and the manager's hook in it) is
    // completely untouched.
    assert_eq!(
        fs::read_to_string(custom_hooks_dir.join("pre-commit"))?,
        manager_hook,
        "core.hooksPath-redirected hook should be byte-identical, untouched"
    );
    assert!(!custom_hooks_dir.join("post-checkout").exists());

    Ok(())
}

#[test]
fn test_config_install_is_idempotent() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;

    let first = install_managed_hooks_with(&repo, true)?;
    assert!(matches!(first, HookInstallationResult::Success));

    let second = install_managed_hooks_with(&repo, true)?;
    assert!(
        matches!(second, HookInstallationResult::AlreadyConfigured),
        "second install should change nothing, got {second:?}"
    );

    Ok(())
}

#[test]
fn test_config_install_migrates_legacy_file_hooks() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    // An older GitButler displaced this user hook into a backup.
    let user_hook = "#!/bin/sh\necho user hook\n";
    create_user_hook(&repo, "pre-commit", user_hook)?;
    install_managed_hooks_with(&repo, false)?;
    assert!(
        hook_exists(&repo, "pre-commit-user"),
        "legacy install backs up"
    );

    // Installing config-based hooks cleans up the legacy install and restores the user hook.
    install_managed_hooks_with(&repo, true)?;

    assert_eq!(
        read_hook(&repo, "pre-commit")?,
        user_hook,
        "user hook restored"
    );
    assert!(!hook_exists(&repo, "pre-commit-user"), "backup consumed");
    assert!(
        !hook_exists(&repo, "post-checkout"),
        "legacy managed hook removed"
    );
    assert!(
        local_config(&dir, "hook.gitbutler-pre-commit.command").is_some(),
        "config-based hooks registered"
    );

    Ok(())
}

#[test]
fn test_uninstall_removes_config_hooks_and_scripts() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    install_managed_hooks_with(&repo, true)?;
    let result = uninstall_managed_hooks(&repo)?;
    assert!(matches!(result, HookInstallationResult::Success));

    for section in ["gitbutler-pre-commit", "gitbutler-post-checkout"] {
        assert_eq!(
            local_config(&dir, &format!("hook.{section}.command")),
            None,
            "{section} should be unregistered"
        );
    }
    assert!(
        !config_scripts_dir(&repo).exists(),
        "scripts directory should be removed when empty"
    );

    Ok(())
}

#[test]
fn test_opt_out_skips_installation_entirely() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    git_ok(&dir, &["config", "gitbutler.installHooks", "false"]);
    // Re-open so the freshly written config is visible to the snapshot.
    let repo = but_testsupport::open_repo(repo.path())?;

    let result = install_managed_hooks(&repo)?;
    assert!(
        matches!(result, HookInstallationResult::SkippedByConfig),
        "expected SkippedByConfig, got {result:?}"
    );

    assert!(!config_scripts_dir(&repo).exists());
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None
    );
    assert!(!hook_exists(&repo, "pre-commit"));

    Ok(())
}

/// Drive a real `git commit` and verify the config-registered guard blocks it.
#[cfg(unix)]
#[test]
fn test_real_git_commit_blocked_on_workspace_branch() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);

    install_managed_hooks_with(&repo, true)?;

    fs::write(dir.join("file.txt"), "content")?;
    git_ok(&dir, &["add", "file.txt"]);
    let out = git_in(&dir, &["commit", "-m", "should be blocked"]);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "commit should be blocked: {combined}"
    );
    assert!(
        combined.contains("GITBUTLER_ERROR"),
        "guard message should be shown: {combined}"
    );

    Ok(())
}

/// Checking out away from the workspace branch removes the config entries and scripts.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_cleans_up() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    // Advance the workspace branch so the previous HEAD resolves to it unambiguously.
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "workspace commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    git_ok(&dir, &["checkout", &default_branch]);

    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None,
        "pre-commit registration should be cleaned up"
    );
    assert_eq!(
        local_config(&dir, "hook.gitbutler-post-checkout.command"),
        None,
        "post-checkout registration should be cleaned up"
    );
    assert!(!config_scripts_dir(&repo).join("pre-commit").exists());
    assert!(!config_scripts_dir(&repo).join("post-checkout").exists());

    Ok(())
}

/// If a previous install left legacy file-based GitButler hooks behind (e.g.
/// because cleanup during migration to config-based hooks failed partway),
/// the config-based post-checkout hook must tear those down too when the
/// user leaves the workspace branch -- not just the config-based
/// registration. `pre-commit` (a different event) is removed outright;
/// `post-checkout` is this same event's own hooks-directory hook, so it is
/// neutered in place instead of deleted (deleting it out from under git's
/// own resolved exec would make the checkout itself fail).
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_removes_legacy_hooks_from_partial_migration() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    // Advance the workspace branch so the previous HEAD resolves to it unambiguously.
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "workspace commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    // Simulate legacy file-based hooks stranded by a partially failed
    // migration: GitButler-signed files sitting directly in the hooks dir,
    // even though config-based hooks are the ones actually registered. The
    // marker lets us tell "still the stale legacy script" apart from "has
    // been neutered by the fix".
    let legacy_hooks_dir = hooks_dir(&repo);
    fs::create_dir_all(&legacy_hooks_dir)?;
    for name in ["pre-commit", "post-checkout"] {
        let legacy_path = legacy_hooks_dir.join(name);
        fs::write(
            &legacy_path,
            "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\n# ACTIVE_LEGACY_HOOK_MARKER\nexit 0\n",
        )?;
        fs::set_permissions(&legacy_path, fs::Permissions::from_mode(0o755))?;
    }

    git_ok(&dir, &["checkout", &default_branch]);

    for section in ["gitbutler-pre-commit", "gitbutler-post-checkout"] {
        assert_eq!(
            local_config(&dir, &format!("hook.{section}.command")),
            None,
            "config-based {section} hook should be unregistered"
        );
    }
    assert!(!config_scripts_dir(&repo).join("pre-commit").exists());
    assert!(!config_scripts_dir(&repo).join("post-checkout").exists());

    assert!(
        !legacy_hooks_dir.join("pre-commit").exists(),
        "legacy pre-commit hook stranded by a partial migration should be removed outright"
    );

    let post_checkout_content = fs::read_to_string(legacy_hooks_dir.join("post-checkout"))?;
    assert!(
        !post_checkout_content.contains("ACTIVE_LEGACY_HOOK_MARKER"),
        "legacy post-checkout hook should be neutered, not left running its stale logic: {post_checkout_content}"
    );
    assert!(
        post_checkout_content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "neutered post-checkout hook should still carry the signature so a future install recognizes it"
    );

    Ok(())
}

/// If another ref happens to coincide on the exact same commit as
/// `gitbutler/workspace`'s tip (e.g. a branch created but never advanced),
/// `git name-rev` can report that *other* ref's name for the previous HEAD
/// instead of "gitbutler/workspace" -- it picks some name for a commit, not
/// necessarily the ref that was actually checked out. Detecting "was on
/// workspace" must not depend on that heuristic guess.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_cleans_up_even_with_ambiguous_commit() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);

    // A decoy ref coinciding on the exact same commit as workspace's tip --
    // name-rev may prefer naming this one instead of "gitbutler/workspace".
    git_ok(&dir, &["branch", "decoy-branch"]);

    install_managed_hooks_with(&repo, true)?;

    git_ok(&dir, &["checkout", &default_branch]);

    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None,
        "leaving workspace should clean up hooks even when another ref coincides on the same commit"
    );

    Ok(())
}

/// A user's `core.hooksPath` may use `~` (e.g. `~/hooks`), which git expands
/// against `HOME` -- but a bare `git config --get` returns the raw,
/// unexpanded string; only `--type=path` applies that expansion. The
/// legacy-hook sweep must expand it the same way this crate's own gix-based
/// `get_hooks_dir` already does (via `trusted_path`), or it computes a
/// nonexistent path and never finds the legacy hooks it's meant to remove.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_sweeps_legacy_hooks_under_tilde_hooks_path() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let tmp = TempDir::new()?;
    // A fake HOME confined to the tempdir, so `~` expansion never touches
    // the real user's home directory.
    let fake_home = tmp.path().join("fake-home");
    fs::create_dir_all(&fake_home)?;
    let repo_dir = tmp.path().join("repo");
    fs::create_dir_all(&repo_dir)?;

    let git = |args: &[&str]| -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(&repo_dir)
            .env("HOME", &fake_home)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("failed to run git")
    };
    let git_ok = |args: &[&str]| -> String {
        let out = git(args);
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };

    git(&["init", "-q"]);
    git_ok(&["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&["symbolic-ref", "--short", "HEAD"]);
    git_ok(&["checkout", "-b", "gitbutler/workspace"]);
    git_ok(&[
        "commit",
        "--allow-empty",
        "--no-verify",
        "-m",
        "workspace commit",
    ]);
    git_ok(&["config", "--local", "core.hooksPath", "~/legacy-hooks"]);

    let repo = but_testsupport::open_repo(&repo_dir)?;
    install_managed_hooks_with(&repo, true)?;

    // Plant a legacy GitButler hook under the ~-expanded hooks directory,
    // simulating a partial migration that left it behind.
    let legacy_hooks_dir = fake_home.join("legacy-hooks");
    fs::create_dir_all(&legacy_hooks_dir)?;
    let legacy_pre_commit = legacy_hooks_dir.join("pre-commit");
    fs::write(
        &legacy_pre_commit,
        "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n",
    )?;
    fs::set_permissions(&legacy_pre_commit, fs::Permissions::from_mode(0o755))?;

    git_ok(&["checkout", &default_branch]);

    assert!(
        !legacy_pre_commit.exists(),
        "legacy hook under a ~-expanded core.hooksPath should be swept, not left running"
    );

    Ok(())
}

/// Leaving a branch whose name merely *contains* "gitbutler/workspace" as a
/// substring (but isn't the managed branch itself) must not trigger cleanup.
/// The previous-branch check must be an exact match, not a substring search.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_from_similarly_named_branch_does_not_clean_up() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);

    // A decoy branch containing "gitbutler/workspace" as a substring, but not
    // equal to it.
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace-old"]);
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "decoy commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    git_ok(&dir, &["checkout", &default_branch]);

    assert!(
        local_config(&dir, "hook.gitbutler-pre-commit.command").is_some(),
        "leaving a decoy branch that only contains 'gitbutler/workspace' as a substring must not trigger cleanup"
    );

    Ok(())
}

/// Opting out via `gitbutler.installHooks=false` after hooks were already
/// installed must actually remove them, not just skip future installs.
#[test]
fn test_opt_out_after_install_uninstalls_existing_hooks() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    install_managed_hooks_with(&repo, true)?;
    assert!(local_config(&dir, "hook.gitbutler-pre-commit.command").is_some());

    git_ok(&dir, &["config", "gitbutler.installHooks", "false"]);
    // Re-open so the freshly written config is visible to the snapshot.
    let repo = but_testsupport::open_repo(repo.path())?;

    install_managed_hooks_with(&repo, true)?;

    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None,
        "opting out after an existing install should remove the previously installed hooks, \
         not just skip future ones"
    );

    Ok(())
}

/// If `git config --remove-section` fails during checkout-away cleanup (e.g.
/// a permissions error), the config entry and its script must be left in a
/// *consistent* state -- either both present or both gone -- never a config
/// entry pointing at a script that was deleted anyway.
#[cfg(unix)]
#[test]
fn test_checkout_away_cleanup_stays_consistent_when_config_removal_fails() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "workspace commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    // Force `git config --remove-section` to fail during checkout-away
    // cleanup: git's config writer needs to create `config.lock` before it
    // can rewrite the file, so pre-creating that lockfile makes the write
    // fail with "could not lock config file" without touching directory
    // permissions (which would also break the checkout's own HEAD/ref
    // updates and defeat the point of the test).
    let config_lock_path = repo.git_dir().join("config.lock");
    fs::write(&config_lock_path, "")?;

    let checkout_result = git_in(&dir, &["checkout", &default_branch]);

    fs::remove_file(&config_lock_path).ok();

    assert!(
        checkout_result.status.success(),
        "post-checkout hook failures must not block the checkout itself: {}",
        String::from_utf8_lossy(&checkout_result.stderr)
    );

    let command_still_registered =
        local_config(&dir, "hook.gitbutler-pre-commit.command").is_some();
    let script_still_exists = config_scripts_dir(&repo).join("pre-commit").exists();

    assert_eq!(
        command_still_registered, script_still_exists,
        "config registration and script presence must stay consistent when config removal fails \
         (command registered: {command_still_registered}, script exists: {script_still_exists})"
    );

    Ok(())
}

/// Cleanup keys off which branch the user actually left (git's own `@{-1}`
/// record), not the workspace ref's current commit -- another GitButler
/// process may advance `gitbutler/workspace` between checkout start and hook
/// execution, and that must not suppress cleanup. Simulated by recording a
/// departure from the workspace branch, advancing the workspace ref, and then
/// invoking the installed hook script directly with the pre-advance HEAD as
/// `$1`, exactly as git would have during the racy checkout.
#[cfg(unix)]
#[test]
fn test_checkout_away_cleans_up_even_when_workspace_ref_advances_concurrently() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    let prev_head = git_ok(&dir, &["rev-parse", "HEAD"]);

    // Leave the workspace branch before hooks are installed: the reflog
    // records the departure, but no hook runs yet.
    git_ok(&dir, &["checkout", &default_branch]);

    // The "concurrent" advance: by hook run time the workspace ref no longer
    // points at the commit that was actually checked out away from.
    let tree = format!("{prev_head}^{{tree}}");
    let advanced = git_ok(
        &dir,
        &["commit-tree", "-p", &prev_head, "-m", "advanced", &tree],
    );
    git_ok(
        &dir,
        &["update-ref", "refs/heads/gitbutler/workspace", &advanced],
    );

    install_managed_hooks_with(&repo, true)?;
    assert!(local_config(&dir, "hook.gitbutler-pre-commit.command").is_some());

    // Run the hook exactly as git would have at the end of the racy checkout.
    let script = config_scripts_dir(&repo).join("post-checkout");
    let new_head = git_ok(&dir, &["rev-parse", "HEAD"]);
    let out = Command::new("sh")
        .arg(&script)
        .args([prev_head.as_str(), new_head.as_str(), "1"])
        .current_dir(&dir)
        .output()?;
    assert!(
        out.status.success(),
        "hook failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None,
        "cleanup must not be suppressed by a concurrent workspace ref advance"
    );

    Ok(())
}

/// `@{-1}` needs the HEAD reflog; with `core.logAllRefUpdates=false` (and no
/// pre-existing logs) it yields nothing. Cleanup must then fall back to
/// comparing the previous HEAD commit against the workspace ref instead of
/// silently never tearing down.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_cleans_up_without_reflog() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["config", "core.logAllRefUpdates", "false"]);
    fs::remove_dir_all(repo.git_dir().join("logs")).ok();

    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "workspace commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    // Sanity: the reflog really is absent, so @{-1} has nothing to answer.
    let prev = git_in(&dir, &["rev-parse", "--symbolic-full-name", "@{-1}"]);
    assert!(
        !prev.status.success() || prev.stdout.iter().all(|b| b.is_ascii_whitespace()),
        "test setup should leave @{{-1}} unresolvable"
    );

    git_ok(&dir, &["checkout", &default_branch]);

    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.command"),
        None,
        "cleanup must still run when the reflog is disabled"
    );
    assert!(!config_scripts_dir(&repo).join("pre-commit").exists());

    Ok(())
}

/// The config-hook scripts live in the common dir, which all linked worktrees
/// share. A code review flagged that one worktree leaving `gitbutler/workspace`
/// would remove the shared scripts and unguard a sibling worktree still on that
/// branch. That sibling state is unreachable: git refuses to check out a branch
/// that is already checked out in another worktree (without `--force`). This
/// test pins the refusal, which is what makes the shared location safe.
#[test]
fn test_second_worktree_cannot_check_out_workspace_branch() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);

    install_managed_hooks_with(&repo, true)?;

    let sibling = dir.join("sibling-worktree");
    let output = git_in(
        &dir,
        &[
            "worktree",
            "add",
            sibling.to_str().expect("test path is valid UTF-8"),
            "gitbutler/workspace",
        ],
    );
    assert!(
        !output.status.success(),
        "git must refuse a second worktree on the workspace branch, but it succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    // Nothing was swept: the shared hook installation is still intact.
    assert!(config_scripts_dir(&repo).join("pre-commit").exists());
    assert!(config_scripts_dir(&repo).join("post-checkout").exists());

    Ok(())
}

/// If the user points `core.hooksPath` at GitButler's own config-scripts
/// directory, config-based installation would overwrite the user's hooks in
/// place (no backup), and its legacy-hook sweep would then delete the freshly
/// written scripts again, leaving the config registrations pointing at nothing.
/// Installation must detect the collision and fall back to the file-based
/// mechanism, which backs up and restores user hooks.
#[test]
fn test_config_install_hooks_path_collision_preserves_user_hook() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    let scripts_dir = config_scripts_dir(&repo);
    fs::create_dir_all(&scripts_dir)?;
    let user_hook = "#!/bin/sh\n# the user's own pre-commit\nexit 0\n";
    fs::write(scripts_dir.join("pre-commit"), user_hook)?;
    git_in(
        &dir,
        &[
            "config",
            "--local",
            "core.hooksPath",
            scripts_dir.to_str().expect("test path is valid UTF-8"),
        ],
    );
    let repo = but_testsupport::open_repo(repo.path())?;

    install_managed_hooks_with(&repo, true)?;

    // An active GitButler hook must exist at the effective hooks path...
    let installed = fs::read_to_string(scripts_dir.join("pre-commit"))?;
    assert!(
        installed.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "a GitButler hook must be in effect at the hooks path, got: {installed}"
    );
    // ...the user's hook must survive as a backup...
    assert_eq!(
        fs::read_to_string(scripts_dir.join("pre-commit-user"))?,
        user_hook,
        "the user's hook must be preserved as a backup"
    );
    // ...and no config registration may point into the directory the legacy
    // sweep cleans, where its script would immediately be removed again.
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.event"),
        None,
        "config-based registration must not be used when core.hooksPath collides with the scripts directory"
    );

    // Uninstall restores the user's hook.
    uninstall_managed_hooks(&repo)?;
    assert_eq!(
        fs::read_to_string(scripts_dir.join("pre-commit"))?,
        user_hook,
        "uninstall must restore the user's original hook"
    );

    Ok(())
}

/// A partially failed migration can leave the old file-based installer's
/// `pre-commit-user` / `post-checkout-user` backups stranded next to the
/// GitButler-signed hooks. The config-based post-checkout cleanup must put the
/// user's originals back in place, not just delete or neuter the managed
/// hooks — otherwise the user's real hooks never run again after leaving the
/// workspace branch.
#[cfg(unix)]
#[test]
fn test_real_git_checkout_away_restores_user_backups_from_partial_migration() -> Result<()> {
    if !real_git_supports_config_hooks() {
        eprintln!("skipping: git on PATH is older than 2.54");
        return Ok(());
    }

    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    git_ok(&dir, &["commit", "--allow-empty", "-m", "initial"]);
    let default_branch = git_ok(&dir, &["symbolic-ref", "--short", "HEAD"]);
    git_ok(&dir, &["checkout", "-b", "gitbutler/workspace"]);
    // Advance the workspace branch so the previous HEAD resolves to it unambiguously.
    git_ok(
        &dir,
        &[
            "commit",
            "--allow-empty",
            "--no-verify",
            "-m",
            "workspace commit",
        ],
    );

    install_managed_hooks_with(&repo, true)?;

    // Strand legacy managed hooks together with the user backups the old
    // file-based installer created when it displaced the user's hooks.
    let legacy_hooks_dir = hooks_dir(&repo);
    fs::create_dir_all(&legacy_hooks_dir)?;
    let user_pre_commit = "#!/bin/sh\n# the user's own pre-commit\nexit 0\n";
    let user_post_checkout = "#!/bin/sh\n# the user's own post-checkout\nexit 0\n";
    for (name, user_content) in [
        ("pre-commit", user_pre_commit),
        ("post-checkout", user_post_checkout),
    ] {
        let managed_path = legacy_hooks_dir.join(name);
        fs::write(
            &managed_path,
            "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n",
        )?;
        fs::set_permissions(&managed_path, fs::Permissions::from_mode(0o755))?;
        let backup_path = legacy_hooks_dir.join(format!("{name}-user"));
        fs::write(&backup_path, user_content)?;
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o755))?;
    }

    git_ok(&dir, &["checkout", &default_branch]);

    assert_eq!(
        fs::read_to_string(legacy_hooks_dir.join("pre-commit"))?,
        user_pre_commit,
        "the user's pre-commit must be restored from its backup"
    );
    assert_eq!(
        fs::read_to_string(legacy_hooks_dir.join("post-checkout"))?,
        user_post_checkout,
        "the user's post-checkout must be restored from its backup"
    );
    assert!(!legacy_hooks_dir.join("pre-commit-user").exists());
    assert!(!legacy_hooks_dir.join("post-checkout-user").exists());

    Ok(())
}

/// The git on `PATH` can lose config-hook support after an install -- a PATH
/// change that puts an older git first is enough. Installing file-based hooks
/// on top of a config-based installation must drop the config registrations,
/// or every commit runs GitButler's hook twice: once via config, once via the
/// hooks directory.
#[test]
fn test_file_install_over_config_install_removes_config_registrations() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);

    install_managed_hooks_with(&repo, true)?;
    assert_eq!(
        local_config(&dir, "hook.gitbutler-pre-commit.event").as_deref(),
        Some("pre-commit"),
        "precondition: the config-based install must have registered the hook"
    );

    // The same repo, now seen by a git that predates config-based hooks.
    install_managed_hooks_with(&repo, false)?;

    let installed = fs::read_to_string(hooks_dir(&repo).join("pre-commit"))?;
    assert!(
        installed.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "the file-based hook must be installed, got: {installed}"
    );
    for section in ["gitbutler-pre-commit", "gitbutler-post-checkout"] {
        assert_eq!(
            local_config(&dir, &format!("hook.{section}.event")),
            None,
            "{section} must be unregistered, or it runs alongside the hooks-directory hook"
        );
    }
    assert!(
        !config_scripts_dir(&repo).join("pre-commit").exists(),
        "the orphaned config script must be removed with its registration"
    );

    Ok(())
}
