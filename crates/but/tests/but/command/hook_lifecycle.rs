//! End-to-end lifecycle tests verifying that managed hooks integrate correctly
//! with real git operations — not just `but hook` subcommands in isolation.
//!
//! These tests prove the hook files are installed, executable, and correctly
//! wired so git invokes them during actual commits, checkouts, and teardowns.

use crate::utils::Sandbox;

/// Assert all 3 managed hook files exist in the sandbox's `.git/hooks/`.
fn assert_hooks_installed(env: &Sandbox) {
    let hooks_dir = env.projects_root().join(".git/hooks");
    for name in ["pre-commit", "post-checkout", "pre-push"] {
        assert!(hooks_dir.join(name).exists(), "{name} should be installed");
    }
}

/// Assert none of the 3 managed hook files exist in the sandbox's `.git/hooks/`.
fn assert_hooks_not_installed(env: &Sandbox) {
    let hooks_dir = env.projects_root().join(".git/hooks");
    for name in ["pre-commit", "post-checkout", "pre-push"] {
        assert!(
            !hooks_dir.join(name).exists(),
            "{name} should not be installed"
        );
    }
}

/// Verify that the installed pre-commit managed hook actually blocks `git commit`
/// on the `gitbutler/workspace` branch.
///
/// This goes beyond invoking `but hook pre-commit` directly — it proves the
/// hook file is installed, executable, and correctly wired so git invokes it
/// during a real commit attempt.
#[test]
fn git_commit_blocked_by_installed_managed_hook() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Verify we're on the workspace branch and hooks are installed.
    let branch = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(branch, "gitbutler/workspace");

    let hooks_dir = env.projects_root().join(".git/hooks");
    let hook_content = std::fs::read_to_string(hooks_dir.join("pre-commit"))?;
    assert!(
        hook_content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "pre-commit hook should be GitButler-managed"
    );

    // Stage a file so git has something to commit.
    env.invoke_bash("echo test > testfile.txt && git add testfile.txt");

    // git commit should FAIL because the pre-commit hook blocks it.
    // Capture combined stdout+stderr (git may route hook output to either)
    // and verify the workspace guard error is present.
    env.invoke_bash(
        r#"OUTPUT=$(git commit -m 'should be blocked by hook' 2>&1 || true); echo "$OUTPUT" | grep -q GITBUTLER_ERROR"#,
    );

    Ok(())
}

/// Verify that `git checkout main` from the workspace branch triggers the
/// post-checkout hook's self-cleanup, removing all managed hook files.
///
/// This goes beyond `script_behavior::post_checkout_cleans_up_when_leaving_real_workspace`
/// which runs the hook script directly via `sh` — this test proves git actually
/// invokes the hook during a real checkout and the cleanup runs end-to-end.
#[test]
fn git_checkout_from_workspace_removes_managed_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Verify hooks are installed after setup.
    assert_hooks_installed(&env);

    // Leave the workspace branch — the post-checkout hook should fire and clean up.
    env.invoke_git("checkout main");

    // All managed hooks should be removed by the post-checkout self-cleanup.
    assert_hooks_not_installed(&env);

    Ok(())
}

/// Verify the full `but setup` → `but teardown` lifecycle correctly manages hooks:
/// hooks are installed after setup and explicitly uninstalled during teardown.
#[test]
fn setup_teardown_hook_lifecycle() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let hooks_dir = env.projects_root().join(".git/hooks");

    // After init_scenario (which calls `but setup`), hooks should be installed.
    assert_hooks_installed(&env);
    for hook_name in ["pre-commit", "post-checkout", "pre-push"] {
        let content = std::fs::read_to_string(hooks_dir.join(hook_name))?;
        assert!(
            content.contains("GITBUTLER_MANAGED_HOOK_V1"),
            "{hook_name} should be GitButler-managed after setup"
        );
    }

    // Verify hook status reports managed mode.
    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
  Mode:[..]GitButler-managed
...
  pre-commit:[..]GitButler-managed
  post-checkout:[..]GitButler-managed
  pre-push:[..]GitButler-managed
...
"#]]);

    // Teardown should uninstall managed hooks and check out branch A.
    env.but("teardown")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Hooks should be removed after teardown.
    assert_hooks_not_installed(&env);

    Ok(())
}
