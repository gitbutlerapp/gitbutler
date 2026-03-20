//! Tests for GitButler managed hooks installation and cleanup
//!
//! These tests verify safety-critical behavior around hook management:
//! - Idempotency of install/uninstall operations
//! - Backup and restore semantics
//! - Protection of non-GitButler hooks from being overwritten or removed

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gitbutler_repo::managed_hooks::{
    HookInstallationResult, HookSetupOutcome, ensure_managed_hooks, install_hooks_config_key,
    install_managed_hooks, install_managed_hooks_enabled, set_install_managed_hooks_enabled,
    uninstall_managed_hooks,
};
use tempfile::TempDir;

/// Helper to create a test hooks directory (simulates `.git/hooks/`)
fn create_test_hooks_dir() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let hooks_dir = temp_dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    Ok((temp_dir, hooks_dir))
}

/// Helper to create a gix repo with its hooks directory for `ensure_managed_hooks` tests.
fn create_repo_with_hooks_dir() -> Result<(TempDir, gix::Repository, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;
    let hooks_dir = repo.git_dir().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    Ok((temp_dir, repo, hooks_dir))
}

/// Helper to create a user hook file with content
fn create_user_hook(hooks_dir: &Path, hook_name: &str, content: &str) -> Result<()> {
    fs::create_dir_all(hooks_dir)?;
    let hook_path = hooks_dir.join(hook_name);
    fs::write(&hook_path, content)?;

    #[cfg(unix)]
    fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;

    Ok(())
}

/// Helper to check if a file exists
fn hook_exists(hooks_dir: &Path, hook_name: &str) -> bool {
    hooks_dir.join(hook_name).exists()
}

/// Helper to read hook content
fn read_hook(hooks_dir: &Path, hook_name: &str) -> Result<String> {
    let path = hooks_dir.join(hook_name);
    Ok(fs::read_to_string(path)?)
}

/// Helper to create a GitButler-managed hook file directly (simulates prior installation)
fn create_managed_hook(hooks_dir: &Path, hook_name: &str) -> Result<()> {
    let content = format!(
        "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\n# Test managed hook for {hook_name}\nexit 0\n"
    );
    create_user_hook(hooks_dir, hook_name, &content)
}

/// Helper to check if hook is executable on Unix
#[cfg(unix)]
fn is_executable(hooks_dir: &Path, hook_name: &str) -> bool {
    let path = hooks_dir.join(hook_name);
    if let Ok(metadata) = fs::metadata(&path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[test]
fn test_install_hooks_creates_hooks_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let hooks_dir = temp_dir.path().join("hooks");
    // hooks_dir does not exist yet

    install_managed_hooks(&hooks_dir, false)?;

    assert!(hooks_dir.exists(), "Hooks directory should be created");
    Ok(())
}

#[test]
fn test_install_managed_hooks_enabled_defaults_to_true() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    assert!(install_managed_hooks_enabled(&repo));
    Ok(())
}

#[test]
fn test_set_install_managed_hooks_enabled_persists_false() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    set_install_managed_hooks_enabled(&repo, false)?;

    let reopened = gix::open(temp_dir.path())?;
    assert!(!install_managed_hooks_enabled(&reopened));
    assert_eq!(
        reopened
            .config_snapshot()
            .string(install_hooks_config_key())
            .as_deref(),
        Some("false".into())
    );
    Ok(())
}

#[test]
fn test_set_install_managed_hooks_enabled_persists_true() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = gix::init(temp_dir.path())?;

    set_install_managed_hooks_enabled(&repo, false)?;
    set_install_managed_hooks_enabled(&repo, true)?;

    let reopened = gix::open(temp_dir.path())?;
    assert!(install_managed_hooks_enabled(&reopened));
    assert_eq!(
        reopened
            .config_snapshot()
            .string(install_hooks_config_key())
            .as_deref(),
        Some("true".into())
    );
    Ok(())
}

#[test]
fn test_install_creates_pre_commit_and_post_checkout_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    install_managed_hooks(&hooks_dir, false)?;

    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "pre-commit hook should exist"
    );
    assert!(
        hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout hook should exist"
    );
    assert!(
        hook_exists(&hooks_dir, "pre-push"),
        "pre-push hook should exist"
    );
    Ok(())
}

#[test]
fn test_installed_hooks_have_gitbutler_signature() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    install_managed_hooks(&hooks_dir, false)?;

    let pre_commit = read_hook(&hooks_dir, "pre-commit")?;
    let post_checkout = read_hook(&hooks_dir, "post-checkout")?;
    let pre_push = read_hook(&hooks_dir, "pre-push")?;

    assert!(
        pre_commit.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "pre-commit should have signature"
    );
    assert!(
        post_checkout.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "post-checkout should have signature"
    );
    assert!(
        pre_push.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "pre-push should have signature"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_installed_hooks_are_executable() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    install_managed_hooks(&hooks_dir, false)?;

    assert!(
        is_executable(&hooks_dir, "pre-commit"),
        "pre-commit should be executable"
    );
    assert!(
        is_executable(&hooks_dir, "post-checkout"),
        "post-checkout should be executable"
    );
    assert!(
        is_executable(&hooks_dir, "pre-push"),
        "pre-push should be executable"
    );
    Ok(())
}

#[test]
fn test_install_is_idempotent() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Install twice
    let result1 = install_managed_hooks(&hooks_dir, false)?;
    let result2 = install_managed_hooks(&hooks_dir, false)?;

    // First install should succeed
    assert!(matches!(result1, HookInstallationResult::Success));

    // Second install should detect already configured
    assert!(matches!(result2, HookInstallationResult::AlreadyConfigured));

    // Hooks should still exist and be valid
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    Ok(())
}

#[test]
fn test_install_preserves_external_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let user_hook_content =
        "#!/bin/sh\n# User's custom pre-commit hook\necho 'Running user hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", user_hook_content)?;

    // Install should skip pre-commit (external hook, no prior backup)
    // but still install post-checkout, so overall result is Success
    install_managed_hooks(&hooks_dir, false)?;

    // User hook should be untouched
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(content, user_hook_content, "User hook should be preserved");

    // No backup should be created
    assert!(
        !hook_exists(&hooks_dir, "pre-commit-user"),
        "No backup should be created for external hooks"
    );

    // post-checkout (no existing hook) should still be installed
    assert!(
        hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout should be installed when no external hook exists"
    );
    Ok(())
}

#[test]
fn test_install_does_not_overwrite_existing_backup() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let original_backup = "#!/bin/sh\n# Original user hook\necho 'original'\n";
    let new_hook = "#!/bin/sh\n# New hook\necho 'new'\n";

    // Create original backup
    create_user_hook(&hooks_dir, "pre-commit-user", original_backup)?;

    // Create a new hook (not GitButler managed)
    create_user_hook(&hooks_dir, "pre-commit", new_hook)?;

    // Install GitButler hooks - should NOT overwrite the backup
    install_managed_hooks(&hooks_dir, false)?;

    // Backup should still have original content
    let backup_content = read_hook(&hooks_dir, "pre-commit-user")?;
    assert_eq!(
        backup_content, original_backup,
        "Backup should not be overwritten"
    );
    Ok(())
}

#[test]
fn test_uninstall_removes_managed_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    install_managed_hooks(&hooks_dir, false)?;
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));

    uninstall_managed_hooks(&hooks_dir)?;

    assert!(
        !hook_exists(&hooks_dir, "pre-commit"),
        "pre-commit should be removed"
    );
    assert!(
        !hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout should be removed"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-push"),
        "pre-push should be removed"
    );
    Ok(())
}

#[test]
fn test_uninstall_restores_user_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let user_hook_content = "#!/bin/sh\n# User's custom hook\necho 'user hook'\n";

    // Simulate prior GitButler installation: managed hook + user backup
    create_managed_hook(&hooks_dir, "pre-commit")?;
    create_user_hook(&hooks_dir, "pre-commit-user", user_hook_content)?;

    // Uninstall should restore the backup
    uninstall_managed_hooks(&hooks_dir)?;

    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "User hook should be restored"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-commit-user"),
        "Backup should be removed after restore"
    );

    let restored_content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        restored_content, user_hook_content,
        "Restored hook should have original content"
    );
    Ok(())
}

#[test]
fn test_uninstall_does_not_remove_non_managed_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Create a non-GitButler hook
    let user_hook = "#!/bin/sh\n# Not a GitButler hook\necho 'user hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", user_hook)?;

    // Try to uninstall - should not remove the hook
    let result = uninstall_managed_hooks(&hooks_dir)?;

    // Hook should still exist
    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "Non-managed hook should not be removed"
    );
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(content, user_hook, "Hook content should be unchanged");

    // Should report skipped (external hook)
    assert!(matches!(
        result,
        HookInstallationResult::Success | HookInstallationResult::Skipped { .. }
    ));
    Ok(())
}

#[test]
fn test_uninstall_is_idempotent() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    install_managed_hooks(&hooks_dir, false)?;

    // Uninstall twice
    let result1 = uninstall_managed_hooks(&hooks_dir)?;
    let result2 = uninstall_managed_hooks(&hooks_dir)?;

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
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let original_pre_commit = "#!/bin/sh\n# Original pre-commit\necho 'pre-commit'\n";
    let original_post_checkout = "#!/bin/sh\n# Original post-checkout\necho 'post-checkout'\n";
    let original_pre_push = "#!/bin/sh\n# Original pre-push\necho 'pre-push'\n";

    // Simulate prior GitButler installation: managed hooks + user backups
    create_managed_hook(&hooks_dir, "pre-commit")?;
    create_managed_hook(&hooks_dir, "post-checkout")?;
    create_managed_hook(&hooks_dir, "pre-push")?;
    create_user_hook(&hooks_dir, "pre-commit-user", original_pre_commit)?;
    create_user_hook(&hooks_dir, "post-checkout-user", original_post_checkout)?;
    create_user_hook(&hooks_dir, "pre-push-user", original_pre_push)?;

    // Re-installing should either update stale hooks or detect them as current
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(matches!(
        result,
        HookInstallationResult::Success | HookInstallationResult::AlreadyConfigured
    ));

    // Uninstall should restore originals
    uninstall_managed_hooks(&hooks_dir)?;

    // Verify original hooks are restored
    let restored_pre = read_hook(&hooks_dir, "pre-commit")?;
    let restored_post = read_hook(&hooks_dir, "post-checkout")?;
    let restored_push = read_hook(&hooks_dir, "pre-push")?;
    assert_eq!(
        restored_pre, original_pre_commit,
        "pre-commit should be restored"
    );
    assert_eq!(
        restored_post, original_post_checkout,
        "post-checkout should be restored"
    );
    assert_eq!(
        restored_push, original_pre_push,
        "pre-push should be restored"
    );

    // Verify backups are gone
    assert!(!hook_exists(&hooks_dir, "pre-commit-user"));
    assert!(!hook_exists(&hooks_dir, "post-checkout-user"));
    assert!(!hook_exists(&hooks_dir, "pre-push-user"));
    Ok(())
}

#[test]
fn test_multiple_install_uninstall_cycles() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let user_hook = "#!/bin/sh\necho 'user hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", user_hook)?;

    // Cycle 1
    install_managed_hooks(&hooks_dir, false)?;
    uninstall_managed_hooks(&hooks_dir)?;

    // Cycle 2
    install_managed_hooks(&hooks_dir, false)?;
    uninstall_managed_hooks(&hooks_dir)?;

    // Cycle 3
    install_managed_hooks(&hooks_dir, false)?;
    uninstall_managed_hooks(&hooks_dir)?;

    // User hook should still be intact
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, user_hook,
        "User hook should survive multiple cycles"
    );
    Ok(())
}

#[test]
fn test_hook_manually_modified_after_install() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Install GitButler hooks
    install_managed_hooks(&hooks_dir, false)?;

    // User manually modifies the hook
    let modified_hook = "#!/bin/sh\n# User modified this\necho 'modified'\n";
    let hook_path = hooks_dir.join("pre-commit");
    fs::write(&hook_path, modified_hook)?;

    // Uninstall should not remove the modified hook (no signature)
    uninstall_managed_hooks(&hooks_dir)?;

    // Hook should still exist with user's modifications
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(content, modified_hook, "Modified hook should be preserved");
    Ok(())
}

#[test]
fn test_installs_into_custom_hooks_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // The caller resolves core.hooksPath — we just verify install_managed_hooks
    // correctly installs into whatever directory is passed.
    let custom_hooks = temp_dir.path().join("custom-hooks");
    let default_hooks = temp_dir.path().join("default-hooks");
    fs::create_dir_all(&default_hooks)?;

    // Install hooks into the custom directory
    install_managed_hooks(&custom_hooks, false)?;

    // Hooks should be in custom directory
    assert!(
        custom_hooks.join("pre-commit").exists(),
        "Hook should be in custom directory"
    );
    assert!(
        custom_hooks.join("post-checkout").exists(),
        "Hook should be in custom directory"
    );

    // Should NOT be in default directory
    assert!(
        !default_hooks.join("pre-commit").exists(),
        "Hook should not be in default location"
    );
    Ok(())
}

#[test]
fn test_partial_installation_with_one_existing_hook() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Create only pre-commit user hook (external, not GitButler-managed)
    let user_hook = "#!/bin/sh\necho 'user pre-commit'\n";
    create_user_hook(&hooks_dir, "pre-commit", user_hook)?;

    // Install should preserve pre-commit (external) and create post-checkout
    install_managed_hooks(&hooks_dir, false)?;

    // pre-commit should be preserved (external hook, no backup created)
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, user_hook,
        "External pre-commit should be preserved"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-commit-user"),
        "No backup should be created for external hooks"
    );

    // post-checkout should be newly installed
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    let post = read_hook(&hooks_dir, "post-checkout")?;
    assert!(
        post.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "post-checkout should be GitButler managed"
    );
    assert!(
        !hook_exists(&hooks_dir, "post-checkout-user"),
        "No backup for post-checkout"
    );

    // Uninstall should skip pre-commit (not managed), remove post-checkout
    uninstall_managed_hooks(&hooks_dir)?;

    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "External pre-commit should still exist"
    );
    assert!(
        !hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout should be removed"
    );
    Ok(())
}

#[test]
fn test_hook_with_shebang_variations_preserved() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Create external hooks with different shebangs
    let bash_env = "#!/usr/bin/env bash\necho 'bash hook'\n";
    let bash_direct = "#!/bin/bash\necho 'bash hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", bash_env)?;
    create_user_hook(&hooks_dir, "post-checkout", bash_direct)?;

    // Install should preserve both external hooks
    install_managed_hooks(&hooks_dir, false)?;

    // Hooks should be untouched (no backups created)
    assert!(!hook_exists(&hooks_dir, "pre-commit-user"));
    assert!(!hook_exists(&hooks_dir, "post-checkout-user"));

    // Verify original shebangs are preserved
    let pre_restored = read_hook(&hooks_dir, "pre-commit")?;
    let post_restored = read_hook(&hooks_dir, "post-checkout")?;

    assert!(pre_restored.starts_with("#!/usr/bin/env bash"));
    assert!(post_restored.starts_with("#!/bin/bash"));
    Ok(())
}

#[test]
fn test_empty_hooks_directory() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // hooks_dir exists but is empty (create_test_hooks_dir creates it)

    // Should install cleanly
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(matches!(result, HookInstallationResult::Success));

    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    Ok(())
}

#[test]
fn test_concurrent_installs_with_backup_present() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Simulate a scenario where backup already exists (from previous install)
    let backup_content = "#!/bin/sh\necho 'original backup'\n";
    create_user_hook(&hooks_dir, "pre-commit-user", backup_content)?;

    // Create a new hook that's different
    let new_hook = "#!/bin/sh\necho 'new hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", new_hook)?;

    // Install should not overwrite the existing backup
    install_managed_hooks(&hooks_dir, false)?;

    let backup = read_hook(&hooks_dir, "pre-commit-user")?;
    assert_eq!(
        backup, backup_content,
        "Existing backup should not be modified"
    );
    Ok(())
}

#[test]
fn test_force_install_overwrites_external_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let external_hook = "#!/bin/sh\n# External manager hook\necho 'external'\n";
    create_user_hook(&hooks_dir, "pre-commit", external_hook)?;

    // Without force, the external hook is preserved
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(matches!(
        result,
        HookInstallationResult::Success | HookInstallationResult::Skipped { .. }
    ));
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, external_hook,
        "External hook should be preserved without force"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-commit-user"),
        "No backup without force"
    );

    // With force, the external hook is backed up and overwritten
    let result = install_managed_hooks(&hooks_dir, true)?;
    assert!(matches!(result, HookInstallationResult::Success));

    // GitButler hook should now be installed
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert!(
        content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "GitButler hook should be installed after force"
    );

    // Original hook should be backed up
    assert!(
        hook_exists(&hooks_dir, "pre-commit-user"),
        "Backup should exist after force"
    );
    let backup = read_hook(&hooks_dir, "pre-commit-user")?;
    assert_eq!(
        backup, external_hook,
        "Backup should contain original hook content"
    );

    Ok(())
}

/// Regression test: after force-install backs up an external hook, if the external
/// tool (e.g. prek) later overwrites the GB hook, a non-force `install_managed_hooks`
/// must skip (not silently overwrite the new external hook).
#[test]
fn test_non_force_skips_external_hook_even_when_backup_exists() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    let external_hook_v1 = "#!/bin/sh\n# External manager hook v1\necho 'external v1'\n";
    create_user_hook(&hooks_dir, "pre-commit", external_hook_v1)?;

    // Step 1: force-install backs up external hook and installs GB hook
    let result = install_managed_hooks(&hooks_dir, true)?;
    assert!(matches!(result, HookInstallationResult::Success));
    assert!(
        hook_exists(&hooks_dir, "pre-commit-user"),
        "Backup should exist after force-install"
    );

    // Step 2: external tool overwrites GB hook with its own (e.g. `prek install`)
    let external_hook_v2 = "#!/bin/sh\n# File generated by prek\nprek hook-impl pre-commit\n";
    fs::write(hooks_dir.join("pre-commit"), external_hook_v2)?;

    // Step 3: non-force install must NOT overwrite the external hook
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(
        matches!(result, HookInstallationResult::Skipped { .. }),
        "Non-force install should skip when external hook exists (even with backup), got: {result:?}"
    );

    // Verify external hook is preserved
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, external_hook_v2,
        "External hook should NOT be overwritten by non-force install"
    );

    Ok(())
}

#[test]
fn test_reinstall_updates_stale_managed_hook() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Install hooks normally
    install_managed_hooks(&hooks_dir, false)?;

    // Manually overwrite pre-commit with stale content (keeping the marker)
    let stale_content =
        "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\n# Stale version of the hook\nexit 0\n";
    fs::write(hooks_dir.join("pre-commit"), stale_content)?;

    // Verify the content is stale
    let before = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(before, stale_content, "Precondition: hook should be stale");

    // Re-install should detect staleness and update
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(
        matches!(result, HookInstallationResult::Success),
        "Re-install of stale hook should return Success, got: {result:?}"
    );

    // Content should now match the current template
    let after = read_hook(&hooks_dir, "pre-commit")?;
    assert_ne!(
        after, stale_content,
        "Hook content should have been updated"
    );
    assert!(
        after.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "Updated hook should still have the signature"
    );
    assert!(
        after.contains("Cannot commit directly to gitbutler/workspace"),
        "Updated hook should have the current pre-commit logic"
    );

    Ok(())
}

#[test]
fn test_reinstall_all_hooks_reports_correctly_when_some_stale() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Install hooks normally
    install_managed_hooks(&hooks_dir, false)?;

    // Make only pre-push stale (keeping marker)
    let stale_content = "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\n# Stale pre-push\nexit 0\n";
    fs::write(hooks_dir.join("pre-push"), stale_content)?;

    // Re-install: 2 hooks current, 1 stale → overall result should be Success
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(
        matches!(result, HookInstallationResult::Success),
        "Should return Success when any hook was updated, got: {result:?}"
    );

    // All hooks should now have current content
    let pre_commit = read_hook(&hooks_dir, "pre-commit")?;
    let post_checkout = read_hook(&hooks_dir, "post-checkout")?;
    let pre_push = read_hook(&hooks_dir, "pre-push")?;

    assert!(pre_commit.contains("Cannot commit directly to gitbutler/workspace"));
    assert!(post_checkout.contains("You have left GitButler"));
    assert!(pre_push.contains("Cannot push the gitbutler/workspace"));
    assert_ne!(pre_push, stale_content, "pre-push should have been updated");

    Ok(())
}

#[test]
fn test_partial_managed_state_reinstalls_missing_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_test_hooks_dir()?;

    // Install all hooks
    install_managed_hooks(&hooks_dir, false)?;
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));

    // User manually deletes post-checkout
    fs::remove_file(hooks_dir.join("post-checkout"))?;
    assert!(!hook_exists(&hooks_dir, "post-checkout"));

    // Re-install should reinstall the missing hook
    let result = install_managed_hooks(&hooks_dir, false)?;
    assert!(
        matches!(result, HookInstallationResult::Success),
        "Should return Success when reinstalling missing hook, got: {result:?}"
    );

    // All hooks should exist with current content
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));

    let post_checkout = read_hook(&hooks_dir, "post-checkout")?;
    assert!(
        post_checkout.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "Reinstalled post-checkout should have the signature"
    );

    Ok(())
}

// ── ensure_managed_hooks tests ─────────────────────────────────────────

#[test]
fn test_ensure_managed_hooks_installs_when_no_manager_and_config_default() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(outcome, HookSetupOutcome::Installed),
        "Should install hooks on clean repo, got: {outcome:?}"
    );

    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_returns_disabled_by_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    set_install_managed_hooks_enabled(&repo, false)?;
    // Re-open to pick up the config change.
    let repo = gix::open(_temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(outcome, HookSetupOutcome::DisabledByConfig),
        "Should return DisabledByConfig when config is false, got: {outcome:?}"
    );

    // No hooks should be installed.
    assert!(!hook_exists(&hooks_dir, "pre-commit"));
    assert!(!hook_exists(&hooks_dir, "post-checkout"));
    assert!(!hook_exists(&hooks_dir, "pre-push"));
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_force_bypasses_disabled_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    set_install_managed_hooks_enabled(&repo, false)?;
    // Re-open to pick up the config change.
    let repo = gix::open(_temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, true);
    assert!(
        matches!(outcome, HookSetupOutcome::Installed),
        "force=true should bypass DisabledByConfig, got: {outcome:?}"
    );

    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_detects_prek_and_persists_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Create a prek-managed hook.
    let prek_hook = "#!/bin/sh\n# File generated by prek\nexec prek hook-impl pre-commit\n";
    create_user_hook(&hooks_dir, "pre-commit", prek_hook)?;

    // Also create a prek.toml in workdir so the manager is detected.
    fs::write(_temp.path().join("prek.toml"), "# prek config\n")?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(
            outcome,
            HookSetupOutcome::ExternalManagerDetected {
                ref manager_name,
                ..
            } if manager_name == "prek"
        ),
        "Should detect prek, got: {outcome:?}"
    );

    // Config should be persisted as false.
    let repo = gix::open(_temp.path())?;
    assert!(
        !install_managed_hooks_enabled(&repo),
        "installHooks should be persisted as false after detection"
    );
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_subsequent_call_after_detection_returns_disabled() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Create a prek-managed hook + config.
    let prek_hook = "#!/bin/sh\n# File generated by prek\nexec prek hook-impl pre-commit\n";
    create_user_hook(&hooks_dir, "pre-commit", prek_hook)?;
    fs::write(_temp.path().join("prek.toml"), "# prek config\n")?;

    // First call: detect manager.
    let outcome1 = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(matches!(
        outcome1,
        HookSetupOutcome::ExternalManagerDetected { .. }
    ));

    // Re-open repo to see persisted config.
    let repo = gix::open(_temp.path())?;

    // Second call: should hit DisabledByConfig fast path.
    let outcome2 = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(outcome2, HookSetupOutcome::DisabledByConfig),
        "Subsequent call should return DisabledByConfig, got: {outcome2:?}"
    );
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_force_overrides_external_manager() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Create a prek-managed hook + config.
    let prek_hook = "#!/bin/sh\n# File generated by prek\nexec prek hook-impl pre-commit\n";
    create_user_hook(&hooks_dir, "pre-commit", prek_hook)?;
    fs::write(_temp.path().join("prek.toml"), "# prek config\n")?;

    // Force install should skip detection and install hooks.
    let outcome = ensure_managed_hooks(&repo, &hooks_dir, true);
    assert!(
        matches!(outcome, HookSetupOutcome::Installed),
        "force=true should override external manager, got: {outcome:?}"
    );

    // Prek hook should be backed up, GB hook installed.
    assert!(hook_exists(&hooks_dir, "pre-commit-user"));
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert!(
        content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "GitButler hook should be installed after force"
    );
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_detection_cleans_up_existing_gb_hooks() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Pre-install GitButler hooks (simulates prior app-triggered install).
    install_managed_hooks(&hooks_dir, false)?;
    assert!(hook_exists(&hooks_dir, "pre-commit"));
    assert!(hook_exists(&hooks_dir, "post-checkout"));
    assert!(hook_exists(&hooks_dir, "pre-push"));

    // Now introduce a prek-managed hook (overwriting GB's pre-commit).
    let prek_hook = "#!/bin/sh\n# File generated by prek\nexec prek hook-impl pre-commit\n";
    fs::write(hooks_dir.join("pre-commit"), prek_hook)?;
    fs::write(_temp.path().join("prek.toml"), "# prek config\n")?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(matches!(
        outcome,
        HookSetupOutcome::ExternalManagerDetected { .. }
    ));

    // GB-managed post-checkout and pre-push should be cleaned up.
    assert!(
        !hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout should be removed after external manager detection"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-push"),
        "pre-push should be removed after external manager detection"
    );

    // The prek hook should still be there (uninstall only removes GB-managed hooks).
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert!(
        content.contains("File generated by prek"),
        "Prek hook should not be removed"
    );
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_skips_unknown_external_hook() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Create an unknown external hook (not a known manager, no config files).
    let unknown_hook = "#!/bin/sh\n# Some unknown hook manager\necho 'unknown'\n";
    create_user_hook(&hooks_dir, "pre-commit", unknown_hook)?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(
            outcome,
            HookSetupOutcome::HookSkipped { ref hook_name } if hook_name == "pre-commit"
        ) || matches!(outcome, HookSetupOutcome::Installed),
        "Should either skip unknown hook or install remaining, got: {outcome:?}"
    );

    // The unknown hook should be preserved.
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, unknown_hook,
        "Unknown external hook should be preserved"
    );
    Ok(())
}

#[test]
fn test_ensure_managed_hooks_idempotent_on_installed_hooks() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // First install.
    let outcome1 = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(matches!(outcome1, HookSetupOutcome::Installed));

    // Second install — should still return Installed (already configured).
    let outcome2 = ensure_managed_hooks(&repo, &hooks_dir, false);
    assert!(
        matches!(outcome2, HookSetupOutcome::Installed),
        "Idempotent call should return Installed, got: {outcome2:?}"
    );
    Ok(())
}
