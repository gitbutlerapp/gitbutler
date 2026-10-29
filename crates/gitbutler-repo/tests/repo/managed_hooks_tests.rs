//! Tests for cleaning up hooks that older GitButler versions installed
//!
//! These tests verify safety-critical behavior around hook cleanup:
//! - Idempotency of uninstall
//! - Restoring user hooks that the old installer backed up
//! - Protection of non-GitButler hooks from being removed

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Result;
use gitbutler_repo::managed_hooks::{HookInstallationResult, uninstall_managed_hooks};
use tempfile::TempDir;

/// What older GitButler versions wrote: any hook carrying the signature is theirs.
const MANAGED_HOOK: &str = "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n";

/// Helper to create a test git repository
fn create_test_repo() -> Result<(TempDir, gix::Repository)> {
    let tmp = TempDir::new()?;
    let repo = gix::init(tmp.path())?;
    // Re-open in isolated mode, it's a bit hard to pass open-options to `gix::init()`.
    but_testsupport::open_repo(repo.path()).map(|repo| (tmp, repo))
}

fn hooks_dir(repo: &gix::Repository) -> PathBuf {
    repo.git_dir().join("hooks")
}

/// Helper to write a hook file with content
fn write_hook(hooks_dir: &Path, hook_name: &str, content: &str) -> Result<()> {
    fs::create_dir_all(hooks_dir)?;
    fs::write(hooks_dir.join(hook_name), content)?;
    Ok(())
}

/// Plant the hooks the old file-based installer left in the hooks directory.
fn plant_file_based_hooks(hooks_dir: &Path) -> Result<()> {
    write_hook(hooks_dir, "pre-commit", MANAGED_HOOK)?;
    write_hook(hooks_dir, "post-checkout", MANAGED_HOOK)
}

/// Plant the scripts and config registrations the old config-based installer
/// (git 2.54+) left behind.
fn plant_config_based_hooks(repo: &gix::Repository) -> Result<()> {
    let dir = workdir(repo);
    for hook in ["pre-commit", "post-checkout"] {
        write_hook(&config_scripts_dir(repo), hook, MANAGED_HOOK)?;
        git_ok(
            &dir,
            &["config", &format!("hook.gitbutler-{hook}.event"), hook],
        );
        git_ok(
            &dir,
            &[
                "config",
                &format!("hook.gitbutler-{hook}.command"),
                &format!("gitbutler/hooks/{hook}"),
            ],
        );
    }
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

/// Run git in `repo_dir` with host configuration masked out.
fn git_in(repo_dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
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

fn config_scripts_dir(repo: &gix::Repository) -> PathBuf {
    repo.common_dir().join("gitbutler/hooks")
}

fn workdir(repo: &gix::Repository) -> PathBuf {
    repo.workdir()
        .expect("test repos have a worktree")
        .to_owned()
}

#[test]
fn test_uninstall_removes_managed_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;
    plant_file_based_hooks(&hooks_dir(&repo))?;

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

    // The old installer moved user hooks aside as `<hook>-user` before writing its own.
    let original_pre_commit = "#!/bin/sh\n# Original pre-commit\necho 'pre-commit'\n";
    let original_post_checkout = "#!/bin/sh\n# Original post-checkout\necho 'post-checkout'\n";
    plant_file_based_hooks(&hooks_dir(&repo))?;
    write_hook(&hooks_dir(&repo), "pre-commit-user", original_pre_commit)?;
    write_hook(
        &hooks_dir(&repo),
        "post-checkout-user",
        original_post_checkout,
    )?;

    uninstall_managed_hooks(&repo)?;

    assert_eq!(
        read_hook(&repo, "pre-commit")?,
        original_pre_commit,
        "pre-commit should be restored"
    );
    assert_eq!(
        read_hook(&repo, "post-checkout")?,
        original_post_checkout,
        "post-checkout should be restored"
    );
    assert!(
        !hook_exists(&repo, "pre-commit-user"),
        "Backup should be removed after restore"
    );
    assert!(
        !hook_exists(&repo, "post-checkout-user"),
        "Backup should be removed after restore"
    );
    Ok(())
}

#[test]
fn test_uninstall_does_not_remove_non_managed_hooks() -> Result<()> {
    let (_temp, repo) = create_test_repo()?;

    // Create a non-GitButler hook
    let user_hook = "#!/bin/sh\n# Not a GitButler hook\necho 'user hook'\n";
    write_hook(&hooks_dir(&repo), "pre-commit", user_hook)?;

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
    plant_file_based_hooks(&hooks_dir(&repo))?;

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
    plant_file_based_hooks(&expected_hooks_dir)?;

    uninstall_managed_hooks(&repo)?;

    assert!(
        !expected_hooks_dir.join("pre-commit").exists(),
        "Uninstall should remove the managed pre-commit hook from the configured directory"
    );
    assert!(
        !expected_hooks_dir.join("post-checkout").exists(),
        "Uninstall should remove the managed post-checkout hook from the configured directory"
    );
    Ok(())
}

#[test]
fn uninstall_from_linked_worktree_uses_the_common_hook_directory()
-> but_testsupport::gix_testtools::TestResult {
    let fixture = crate::support::RepoWithOrigin::default();
    let main = fixture
        .local_repo
        .workdir()
        .expect("fixture has a worktree");
    but_testsupport::gix_testtools::git(main, "worktree add --detach ../linked")?;
    let linked = main.parent().expect("fixture has a parent").join("linked");
    let common_hooks = fixture.local_repo.common_dir().join("hooks");
    plant_file_based_hooks(&common_hooks)?;
    let repo = but_testsupport::open_repo(&linked)?;
    uninstall_managed_hooks(&repo)?;
    for name in ["pre-commit", "post-checkout"] {
        assert!(
            !common_hooks.join(name).exists(),
            "linked worktrees clean up common hook files, including inactive hooks"
        );
    }
    Ok(())
}

#[test]
fn invalid_hooks_path_reports_a_warning_without_cleaning_up_default_hooks() -> Result<()> {
    let (_temp, mut repo) = create_test_repo()?;
    let default_hooks = hooks_dir(&repo);
    plant_file_based_hooks(&default_hooks)?;
    repo.config_snapshot_mut()
        .append_config(["core.hooksPath"], gix::config::Source::Local)?;
    let outcome = uninstall_managed_hooks(&repo)?;
    assert!(
        matches!(outcome, HookInstallationResult::PartialSuccess { warnings } if warnings.iter().any(|warning| warning.contains("core.hooksPath"))),
        "an invalid configured path is reported instead of silently selecting the default"
    );
    for name in ["pre-commit", "post-checkout"] {
        assert!(
            default_hooks.join(name).exists(),
            "invalid configuration cannot redirect cleanup"
        );
    }
    Ok(())
}

#[test]
fn test_uninstall_removes_config_hooks_and_scripts() -> Result<()> {
    let (_tmp, repo) = create_test_repo()?;
    let dir = workdir(&repo);
    plant_config_based_hooks(&repo)?;
    git_ok(&dir, &["config", "hook.user.event", "pre-commit"]);
    git_ok(&dir, &["config", "hook.user.command", "user-hook"]);

    let result = uninstall_managed_hooks(&repo)?;
    assert!(matches!(result, HookInstallationResult::Success));

    for section in ["gitbutler-pre-commit", "gitbutler-post-checkout"] {
        assert_eq!(
            local_config(&dir, &format!("hook.{section}.command")),
            None,
            "{section} should be unregistered"
        );
    }
    assert_eq!(
        local_config(&dir, "hook.user.command").as_deref(),
        Some("user-hook"),
        "the user's own config-based hook is left alone"
    );
    assert!(
        !config_scripts_dir(&repo).exists(),
        "scripts directory should be removed when empty"
    );

    Ok(())
}
