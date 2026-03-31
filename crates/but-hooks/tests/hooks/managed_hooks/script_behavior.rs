//! Integration tests for the actual shell script behavior of managed hooks.
//!
//! These tests run the installed hook scripts in real git repositories
//! to verify runtime behavior that can't be tested at the filesystem level.

use std::process::Command;
use std::time::SystemTime;

use anyhow::Result;
use but_hooks::managed_hooks::install_managed_hooks;
use tempfile::TempDir;

/// Helper to run a git command in a directory, returning trimmed stdout.
fn git(repo_dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run git {}: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Create a temporary git repo with an initial commit on `main`, returning
/// the temp directory (kept alive by the caller) and the repo path.
fn init_test_repo() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_dir = temp_dir.path().to_path_buf();
    git(&repo_dir, &["init", "-b", "main"]);
    git(&repo_dir, &["config", "user.email", "test@test.com"]);
    git(&repo_dir, &["config", "user.name", "Test"]);
    git(
        &repo_dir,
        &["commit", "--allow-empty", "-m", "initial commit"],
    );
    (temp_dir, repo_dir)
}

/// Create a test repo with managed hooks installed in `.git/hooks/`.
fn init_repo_with_managed_hooks() -> Result<(TempDir, std::path::PathBuf, std::path::PathBuf)> {
    let (temp, repo_dir) = init_test_repo();
    let hooks_dir = repo_dir.join(".git/hooks");
    install_managed_hooks(&hooks_dir, false, SystemTime::now())?;
    Ok((temp, repo_dir, hooks_dir))
}

/// Run a hook script and return its output.
fn run_hook(
    hooks_dir: &std::path::Path,
    hook_name: &str,
    args: &[&str],
    repo_dir: &std::path::Path,
) -> Result<std::process::Output> {
    Ok(Command::new("sh")
        .arg(hooks_dir.join(hook_name))
        .args(args)
        .current_dir(repo_dir)
        .output()?)
}

/// Regression test: branch names containing "gitbutler/workspace" as a substring
/// must NOT trigger post-checkout cleanup. Only the actual gitbutler/workspace
/// branch (or its ancestors via `~N`/`^N` notation) should trigger it.
#[test]
#[cfg(unix)]
fn post_checkout_ignores_branches_with_workspace_substring() -> Result<()> {
    let (_temp, repo_dir, hooks_dir) = init_repo_with_managed_hooks()?;
    let repo_dir = repo_dir.as_path();

    // Create a branch whose name contains "gitbutler/workspace" as a substring
    git(
        repo_dir,
        &["checkout", "-b", "feature/gitbutler/workspace-fix"],
    );
    git(
        repo_dir,
        &["commit", "--allow-empty", "-m", "feature commit"],
    );
    let feature_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    // Switch back to main
    git(repo_dir, &["checkout", "main"]);
    let main_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    let output = run_hook(
        &hooks_dir,
        "post-checkout",
        &[&feature_sha, &main_sha, "1"],
        repo_dir,
    )?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The hook must NOT trigger cleanup for a branch that merely contains
    // "gitbutler/workspace" as a substring
    assert!(
        !stdout.contains("Cleaning up GitButler hooks"),
        "Hook should not cleanup for branch 'feature/gitbutler/workspace-fix', got: {stdout}"
    );

    // All managed hooks should still be present
    super::assert_all_hooks_exist(&hooks_dir);

    Ok(())
}

/// Shared logic for the two detached-HEAD scenarios: the post-checkout hook must
/// not clean up when git's reflog shows the previous state was detached (not the
/// gitbutler/workspace branch), regardless of whether the detach was at HEAD or
/// at the workspace tip explicitly.
#[cfg(unix)]
fn assert_post_checkout_no_cleanup_from_detached(detach_args: &[&str]) -> Result<()> {
    let (_temp, repo_dir) = init_test_repo();
    let repo_dir = repo_dir.as_path();

    git(repo_dir, &["checkout", "-b", "gitbutler/workspace"]);
    git(
        repo_dir,
        &["commit", "--allow-empty", "-m", "workspace commit"],
    );
    let workspace_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    git(repo_dir, detach_args);
    git(repo_dir, &["checkout", "main"]);
    let main_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    // Install hooks AFTER branch setup — the pre-commit hook would block
    // the `git commit --allow-empty` on gitbutler/workspace above.
    let hooks_dir = repo_dir.join(".git/hooks");
    install_managed_hooks(&hooks_dir, false, SystemTime::now())?;

    let output = run_hook(
        &hooks_dir,
        "post-checkout",
        &[&workspace_sha, &main_sha, "1"],
        repo_dir,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("Cleaning up GitButler hooks"),
        "Hook should not cleanup when previous checkout source was detached HEAD, got: {stdout}"
    );
    super::assert_all_hooks_exist(&hooks_dir);

    Ok(())
}

#[test]
#[cfg(unix)]
fn post_checkout_does_not_clean_up_when_previous_state_was_detached() -> Result<()> {
    assert_post_checkout_no_cleanup_from_detached(&["checkout", "--detach"])
}

#[test]
#[cfg(unix)]
fn post_checkout_does_not_clean_up_when_previous_state_was_detached_at_workspace_tip() -> Result<()>
{
    // Detach at the workspace tip explicitly (vs bare `--detach` above).
    // Both must behave identically — the reflog says "detached", not "workspace".
    assert_post_checkout_no_cleanup_from_detached(&["checkout", "--detach", "gitbutler/workspace"])
}

// ── Pre-commit script behavior ────────────────────────────────────────────

/// The pre-commit hook must exit 1 and print `GITBUTLER_ERROR` when on
/// `gitbutler/workspace`, regardless of whether a user hook is present.
#[test]
#[cfg(unix)]
fn pre_commit_blocks_on_workspace_branch() -> Result<()> {
    let (_temp, repo_dir, hooks_dir) = init_repo_with_managed_hooks()?;
    let repo_dir = repo_dir.as_path();

    git(repo_dir, &["checkout", "-b", "gitbutler/workspace"]);

    let output = run_hook(&hooks_dir, "pre-commit", &[], repo_dir)?;

    assert!(
        !output.status.success(),
        "pre-commit should exit non-zero on gitbutler/workspace"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("GITBUTLER_ERROR"),
        "pre-commit should print GITBUTLER_ERROR on workspace branch, got: {stdout}"
    );
    Ok(())
}

/// The pre-commit hook must exit 0 on a normal branch with no user hook.
#[test]
#[cfg(unix)]
fn pre_commit_allows_on_non_workspace_branch() -> Result<()> {
    let (_temp, repo_dir, hooks_dir) = init_repo_with_managed_hooks()?;
    let repo_dir = repo_dir.as_path();
    // stays on `main`

    let output = run_hook(&hooks_dir, "pre-commit", &[], repo_dir)?;

    assert!(
        output.status.success(),
        "pre-commit should exit 0 on a normal branch, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// The workspace-branch guard must fire *before* the user hook runs.
///
/// A user hook that creates a sentinel file is installed. When the pre-commit
/// hook is run on `gitbutler/workspace`, the sentinel must NOT be created —
/// proving the GB check short-circuits before delegating to the user hook.
#[test]
#[cfg(unix)]
fn pre_commit_blocks_before_user_hook_runs() -> Result<()> {
    let (_temp, repo_dir, hooks_dir) = init_repo_with_managed_hooks()?;
    let repo_dir = repo_dir.as_path();

    git(repo_dir, &["checkout", "-b", "gitbutler/workspace"]);

    // Install a user hook that creates a sentinel file when it runs.
    let sentinel = repo_dir.join("user_hook_ran");
    let user_hook = format!("#!/bin/sh\ntouch \"{}\"\nexit 0\n", sentinel.display());
    super::create_user_hook(&hooks_dir, "pre-commit-user", &user_hook)?;

    let output = run_hook(&hooks_dir, "pre-commit", &[], repo_dir)?;

    assert!(
        !output.status.success(),
        "pre-commit should still block on gitbutler/workspace"
    );
    assert!(
        !sentinel.exists(),
        "user hook must NOT run before the workspace guard fires"
    );
    Ok(())
}

/// On a normal branch, the pre-commit hook must delegate to the user hook and
/// propagate its exit code.
#[test]
#[cfg(unix)]
fn pre_commit_delegates_to_user_hook_on_non_workspace() -> Result<()> {
    let (_temp, repo_dir, hooks_dir) = init_repo_with_managed_hooks()?;
    let repo_dir = repo_dir.as_path();
    // stays on `main`

    // Install a user hook that exits 1 with a recognisable message.
    let user_hook = "#!/bin/sh\necho 'user hook ran'\nexit 1\n";
    super::create_user_hook(&hooks_dir, "pre-commit-user", user_hook)?;

    let output = run_hook(&hooks_dir, "pre-commit", &[], repo_dir)?;

    assert!(
        !output.status.success(),
        "pre-commit should propagate non-zero exit from user hook"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("user hook ran"),
        "user hook output should be present, got: {stdout}"
    );
    Ok(())
}

// ── Post-checkout script behavior ─────────────────────────────────────────

/// Verify that the post-checkout hook DOES trigger cleanup when leaving
/// the actual gitbutler/workspace branch (positive case for the grep pattern).
#[test]
#[cfg(unix)]
fn post_checkout_cleans_up_when_leaving_real_workspace() -> Result<()> {
    let (_temp, repo_dir) = init_test_repo();
    let repo_dir = repo_dir.as_path();

    // Create the real gitbutler/workspace branch
    git(repo_dir, &["checkout", "-b", "gitbutler/workspace"]);
    git(
        repo_dir,
        &["commit", "--allow-empty", "-m", "workspace commit"],
    );
    let workspace_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    // Switch to main
    git(repo_dir, &["checkout", "main"]);
    let main_sha = git(repo_dir, &["rev-parse", "HEAD"]);

    // Install hooks AFTER branch setup — the pre-commit hook would block
    // the `git commit --allow-empty` on gitbutler/workspace above.
    let hooks_dir = repo_dir.join(".git/hooks");
    install_managed_hooks(&hooks_dir, false, SystemTime::now())?;

    // Run the post-checkout hook: leaving workspace → main
    let output = run_hook(
        &hooks_dir,
        "post-checkout",
        &[&workspace_sha, &main_sha, "1"],
        repo_dir,
    )?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // The hook SHOULD trigger cleanup
    assert!(
        stdout.contains("Cleaning up GitButler hooks"),
        "Hook should cleanup when leaving gitbutler/workspace, got: {stdout}"
    );

    // Managed hooks should be removed by cleanup
    super::assert_no_hooks_exist(&hooks_dir);

    Ok(())
}
