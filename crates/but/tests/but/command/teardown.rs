use snapbox::IntoData;
use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

/// Test 1: Simple case of a single branch
/// - Teardown should return HEAD to that branch
#[test]
fn single_branch_simple_teardown() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

    // Run teardown
    env.but("teardown")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Exiting GitButler mode...

→ Creating snapshot...
  ✓ Snapshot created: [..]

→ Finding active branch to check out...
  ✓ Will check out: A

→ Checking out A...
  ✓ Checked out: A

✓ Successfully exited GitButler mode!

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Verify we're on branch A
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "A");
}

/// Test 2: Multiple branches
/// - Picks the first branch and returns HEAD to it
/// - Removes other branches' work from working directory
/// - Preserves virtual branch state
#[test]
fn multiple_branches_preserves_state() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 9477ae7 (A) add A
* | d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.setup_metadata(&["A", "B"]);

    // Run teardown
    env.but("teardown")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Exiting GitButler mode...

→ Creating snapshot...
  ✓ Snapshot created: [..]

→ Finding active branch to check out...
  ✓ Will check out: A

→ Checking out A...
  ✓ Checked out: A

✓ Successfully exited GitButler mode!

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Verify we're on branch A (the first one)
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "A");

    // Verify file from branch A is present
    let file_a = env.projects_root().join("A");
    assert!(file_a.exists(), "File A should exist after teardown");

    // Verify file from branch B is NOT present (removed from working directory)
    let file_b = env.projects_root().join("B");
    assert!(
        !file_b.exists(),
        "File B should not exist in working directory after teardown"
    );
}

/// Test 3: User has committed twice on top of gitbutler/workspace
/// - After teardown, second branch should be unapplied
#[test]
fn two_dangling_commits_different_branches() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("teardown-two-dangling-commits");
    // Initial state: user has made two commits on top of workspace
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* fc13bfb (HEAD -> gitbutler/workspace) add FileForB
* 091c8f9 add FileForA
*   c128bce GitButler Workspace Commit
|\  
| * 9477ae7 (A) add A
* | d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.setup_metadata(&["A", "B"]);

    // Run teardown - should cherry-pick both commits to first branch
    // Note: In the current implementation, ALL dangling commits are cherry-picked
    // to the first checked out branch
    env.but("teardown")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Exiting GitButler mode...

→ Creating snapshot...
  ✓ Snapshot created: [..]

→ Finding active branch to check out...

Attempting to fix workspace stacks...
→ Checking for dangling commits...
→ Resetting gitbutler/workspace to c128bce
  ✓ gitbutler/workspace reset to c128bce

  ⚠ Non-GitButler created commits found.
  ⚠ Undoing these commits but keeping the changes in your working directory.
  ⚠ Uncommitted 2 dangling commit(s):
    [..]
    [..]

  ✓ Will check out: A

→ Checking out A...
  ✓ Checked out: A

✓ Successfully exited GitButler mode!

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Verify we're on branch A
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "A");

    // Verify that changes to file A AND B are present
    let file_a_path = env.projects_root().join("FileForA");
    let file_b_path = env.projects_root().join("FileForB");
    let content_a = std::fs::read_to_string(&file_a_path).unwrap();
    let content_b = std::fs::read_to_string(&file_b_path).unwrap();
    assert!(
        content_a.contains("FileForA\n"),
        "File A should contain the modifications"
    );
    assert!(
        content_b.contains("FileForB\n"),
        "File B should contain the modifications"
    );
}

/// Test: JSON output format
#[test]
fn json_output_single_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("--json teardown")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "snapshotId": "[..]",
  "checkedOutBranch": "A"
}

"#]]);

    // check the current git branch is A
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "A");
}

/// Test: JSON output with dangling commits
#[test]
fn json_output_with_dangling_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("teardown-dangling-single-commit");
    env.setup_metadata(&["A"]);

    env.but("--json teardown")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "snapshotId": "[..]",
  "checkedOutBranch": "A"
}

"#]]);

    // check the current git branch is A
    let output = env.invoke_git("rev-parse --abbrev-ref HEAD");
    assert_eq!(output, "A");
}

#[test]
fn teardown_informs_of_checkout_to_when_there_are_no_stacks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("teardown")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Failed to determine checkout target branch. Specify a target branch with `--checkout-to <branch>`.

"#]]);
}

#[test]
fn teardown_checks_out_to_branch_override() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("--json teardown")
        .arg("--checkout-to")
        .arg("A")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "snapshotId": "[..]",
  "checkedOutBranch": "A"
}

"#]]);

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");
}

#[test]
fn teardown_checks_out_to_branch_override_with_qualified_ref_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("--json teardown")
        .arg("--checkout-to")
        .arg("refs/heads/A")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "snapshotId": "[..]",
  "checkedOutBranch": "A"
}

"#]]);

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");
}

#[test]
fn teardown_checkout_to_handles_missing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("teardown")
        .arg("--checkout-to")
        .arg("no-such-branch")
        .assert()
        .failure()
        .stderr_eq(str![
            r#"
Error: Bad input for '--checkout-to'

The reference 'no-such-branch' did not exist

"#
        ]);
}

#[test]
fn teardown_checkout_to_handles_malformed_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("teardown")
        .arg("--checkout-to")
        .arg("not a branch")
        .assert()
        .failure()
        .stderr_eq(str![
            r#"
Error: Bad input for '--checkout-to'

Invalid ref name: not a branch

"#
        ]);
}

#[test]
fn teardown_checkout_to_disallows_non_branch_ref() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("teardown")
        .arg("--checkout-to")
        .arg("HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input for '--checkout-to'

Invalid ref for checkout: 'HEAD' is not a local branch

"#]]);
}

#[test]
fn teardown_checkout_to_disallows_non_local_branch_ref() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("teardown")
        .arg("--checkout-to")
        .arg("origin/main")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input for '--checkout-to'

Invalid ref for checkout: 'origin/main' is not a local branch

"#]]);
}

/// When hook cleanup partially fails, teardown must not report unqualified
/// success: the final human message has to say hooks are left behind.
#[cfg(unix)]
#[test]
fn teardown_with_failing_hook_cleanup_does_not_claim_full_success() {
    use std::os::unix::fs::PermissionsExt;

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // Plant GitButler-managed legacy hooks, then make the hooks directory
    // read-only so their removal fails during teardown.
    let hooks_dir = env.projects_root().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let managed_hook = "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n";
    std::fs::write(hooks_dir.join("pre-commit"), managed_hook).unwrap();
    std::fs::write(hooks_dir.join("post-checkout"), managed_hook).unwrap();
    std::fs::set_permissions(&hooks_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    // Teardown completes, but instead of the "✓ Successfully exited" line it
    // prints per-hook warnings and a "⚠ ... some hooks could not be removed"
    // summary, so the user knows GitButler hooks are still active.
    env.but("teardown")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Exiting GitButler mode...

→ Creating snapshot...
  ✓ Snapshot created: [..]

→ Finding active branch to check out...
  ✓ Will check out: A

  Warning: Failed to uninstall pre-commit: [..]
  Warning: Failed to uninstall post-checkout: [..]
→ Checking out A...
  ✓ Checked out: A

⚠ Exited GitButler mode, but some GitButler hooks could not be removed (see warnings above).

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Restore permissions so the sandbox can clean up after itself.
    std::fs::set_permissions(&hooks_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// JSON consumers must be able to detect a partial hook cleanup: the warnings
/// surface as a dedicated field instead of only being printed for humans.
#[cfg(unix)]
#[test]
fn json_output_reports_partial_hook_cleanup() {
    use std::os::unix::fs::PermissionsExt;

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let hooks_dir = env.projects_root().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let managed_hook = "#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n";
    std::fs::write(hooks_dir.join("pre-commit"), managed_hook).unwrap();
    std::fs::write(hooks_dir.join("post-checkout"), managed_hook).unwrap();
    std::fs::set_permissions(&hooks_dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    env.but("--json teardown")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "snapshotId": "[..]",
  "checkedOutBranch": "A",
  "hookWarnings": [
    "Failed to uninstall pre-commit: [..]",
    "Failed to uninstall post-checkout: [..]"
  ]
}

"#]]);

    std::fs::set_permissions(&hooks_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}
