use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

/// Test 1: Simple case of a single branch
/// - Teardown should return HEAD to that branch
#[test]
fn single_branch_simple_teardown() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    Ok(())
}

/// Test 2: Multiple branches
/// - Picks the first branch and returns HEAD to it
/// - Removes other branches' work from working directory
/// - Preserves virtual branch state
#[test]
fn multiple_branches_preserves_state() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;

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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    // Verify file from branch A is present
    let file_a = env.projects_root().join("A");
    assert!(file_a.exists(), "File A should exist after teardown");

    // Verify file from branch B is NOT present (removed from working directory)
    let file_b = env.projects_root().join("B");
    assert!(
        !file_b.exists(),
        "File B should not exist in working directory after teardown"
    );

    Ok(())
}

/// Test 3: User has committed on top of gitbutler/workspace
/// - Should detect the dangling commit
/// - Should reset the commit
#[test]
#[ignore = "flaky test - needs investigation"]
fn dangling_commit_on_workspace() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a dangling commit on top of workspace
    env.file("UserFile", "user content");
    let git_dir = env.projects_root();
    std::process::Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["add", "."])
        .output()?;
    std::process::Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["commit", "-m", "User commit on workspace"])
        .output()?;

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

Attempting to fix workspace stacks...
→ Checking for dangling commits...
→ Resetting gitbutler/workspace to [..]
  ✓ gitbutler/workspace reset to [..]

  ⚠ Non-GitButler created commits found.
  ⚠ Undoing these commits but keeping the changes in your working directory.
  ⚠ Uncommitted 1 dangling commit(s):
...

  ✓ Will check out: A

→ Checking out A...
  ✓ Checked out: A

✓ Successfully exited GitButler mode!

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Verify we're on branch A
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    // Verify the change is left uncommitted (not cherry-picked)
    let file_path = env.projects_root().join("UserFile");
    assert!(file_path.exists(), "UserFile should exist in working directory");

    // Check that there are uncommitted changes
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .args(["status", "--porcelain"])
        .output()?;
    let status_output = String::from_utf8_lossy(&status.stdout);
    assert!(
        status_output.contains("UserFile"),
        "UserFile should be uncommitted: {}",
        status_output
    );

    Ok(())
}

/// Test 4: User commit on workspace with changes locked to two different branches
/// - Should detect the dangling commit
/// - This tests the edge case where changes belong to different virtual branches
#[test]
#[ignore = "flaky test - needs investigation"]
fn dangling_commit_spanning_multiple_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    // Create a dangling commit touching files from both branches
    let git_dir = env.projects_root();
    std::process::Command::new("sh")
        .arg("-c")
        .arg("echo modified >> A && echo modified >> B")
        .current_dir(git_dir)
        .output()?;
    std::process::Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["add", "A", "B"])
        .output()?;
    std::process::Command::new("git")
        .arg("-C")
        .arg(git_dir)
        .args(["commit", "-m", "User commit touching both branches"])
        .output()?;

    // Run teardown - should cherry-pick to first branch (A)
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
→ Resetting gitbutler/workspace to [..]
  ✓ gitbutler/workspace reset to [..]

  ⚠ Non-GitButler created commits found.
  ⚠ Undoing these commits but keeping the changes in your working directory.
  ⚠ Uncommitted 1 dangling commit(s):
    [..]: User commit touching both branches

  ✓ Will check out: A

→ Checking out A...
  ⚠ Checkout failed, trying soft reset...
  ⚠ This will leave changes from multiple branches in your working directory.
  ⚠ You will have to manually remove, stash or re-commit the changes.
  ✓ Checked out: A

✓ Successfully exited GitButler mode!

You are now on branch: A

To return to GitButler mode, run:
  but setup


"#]]);

    // Verify we're on branch A
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    // Verify that changes to file A AND B are present
    let file_a_path = env.projects_root().join("A");
    let file_b_path = env.projects_root().join("B");
    let content_a = std::fs::read_to_string(&file_a_path)?;
    let content_b = std::fs::read_to_string(&file_b_path)?;
    assert!(
        content_a.contains("modified"),
        "File A should contain the modifications"
    );
    assert!(
        content_b.contains("modified"),
        "File B should contain the modifications"
    );

    Ok(())
}

/// Test 5: User has committed twice on top of gitbutler/workspace
/// - After teardown, second branch should be unapplied
#[test]
fn two_dangling_commits_different_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("teardown-two-dangling-commits")?;
    // Initial state: user has made two commits on top of workspace
    insta::assert_snapshot!(env.git_log()?, @r"
    * fc13bfb (HEAD -> gitbutler/workspace) add FileForB
    * 091c8f9 add FileForA
    *   c128bce GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;

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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    // Verify that changes to file A AND B are present
    let file_a_path = env.projects_root().join("FileForA");
    let file_b_path = env.projects_root().join("FileForB");
    let content_a = std::fs::read_to_string(&file_a_path)?;
    let content_b = std::fs::read_to_string(&file_b_path)?;
    assert!(
        content_a.contains("FileForA\n"),
        "File A should contain the modifications"
    );
    assert!(
        content_b.contains("FileForB\n"),
        "File B should contain the modifications"
    );

    Ok(())
}

/// Test: JSON output format
#[test]
fn json_output_single_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    Ok(())
}

/// Test: JSON output with dangling commits
#[test]
fn json_output_with_dangling_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("teardown-dangling-single-commit")?;
    env.setup_metadata(&["A"])?;

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
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "A");

    Ok(())
}
