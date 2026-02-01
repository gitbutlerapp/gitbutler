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
    HookInstallationResult, install_managed_hooks, uninstall_managed_hooks,
};
use tempfile::TempDir;

/// Helper to create a test git repository
fn create_test_repo() -> Result<(TempDir, git2::Repository)> {
    let temp_dir = TempDir::new()?;
    let repo = git2::Repository::init(temp_dir.path())?;
    Ok((temp_dir, repo))
}

/// Helper to create a user hook file with content
fn create_user_hook(repo: &git2::Repository, hook_name: &str, content: &str) -> Result<()> {
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let hook_path = hooks_dir.join(hook_name);
    fs::write(&hook_path, content)?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    Ok(())
}

/// Helper to check if a file exists
fn hook_exists(repo: &git2::Repository, hook_name: &str) -> bool {
    repo.path().join("hooks").join(hook_name).exists()
}

/// Helper to read hook content
fn read_hook(repo: &git2::Repository, hook_name: &str) -> Result<String> {
    let path = repo.path().join("hooks").join(hook_name);
    Ok(fs::read_to_string(path)?)
}

/// Helper to check if hook is executable on Unix
#[cfg(unix)]
fn is_executable(repo: &git2::Repository, hook_name: &str) -> bool {
    let path = repo.path().join("hooks").join(hook_name);
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
    let hooks_dir = repo.path().join("hooks");
    if hooks_dir.exists() {
        fs::remove_dir_all(&hooks_dir)?;
    }

    install_managed_hooks(&repo)?;

    assert!(hooks_dir.exists(), "Hooks directory should be created");
    Ok(())
}

#[test]
fn test_install_creates_pre_commit_and_post_checkout_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    install_managed_hooks(&repo)?;

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

    install_managed_hooks(&repo)?;

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

    install_managed_hooks(&repo)?;

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
    let result1 = install_managed_hooks(&repo)?;
    let result2 = install_managed_hooks(&repo)?;

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

    install_managed_hooks(&repo)?;

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
    install_managed_hooks(&repo)?;

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

    install_managed_hooks(&repo)?;
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
    install_managed_hooks(&repo)?;
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

    install_managed_hooks(&repo)?;

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
    install_managed_hooks(&repo)?;

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
    install_managed_hooks(&repo)?;
    uninstall_managed_hooks(&repo)?;

    // Cycle 2
    install_managed_hooks(&repo)?;
    uninstall_managed_hooks(&repo)?;

    // Cycle 3
    install_managed_hooks(&repo)?;
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
    install_managed_hooks(&repo)?;

    // User manually modifies the hook
    let modified_hook = "#!/bin/sh\n# User modified this\necho 'modified'\n";
    let hook_path = repo.path().join("hooks").join("pre-commit");
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
fn test_respects_core_hooks_path() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Create a custom hooks directory
    let custom_hooks = _temp.path().join("custom-hooks");
    fs::create_dir_all(&custom_hooks)?;

    // Configure core.hooksPath
    let mut config = repo.config()?;
    config.set_str("core.hooksPath", custom_hooks.to_str().unwrap())?;

    // Install hooks
    install_managed_hooks(&repo)?;

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
        !repo.path().join("hooks").join("pre-commit").exists(),
        "Hook should not be in default location"
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
    install_managed_hooks(&repo)?;

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

    install_managed_hooks(&repo)?;

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
    let hooks_dir = repo.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Should install cleanly
    let result = install_managed_hooks(&repo)?;
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
    install_managed_hooks(&repo)?;

    let backup = read_hook(&repo, "pre-commit-user")?;
    assert_eq!(
        backup, backup_content,
        "Existing backup should not be modified"
    );
    Ok(())
}
