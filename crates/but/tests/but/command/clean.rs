use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn no_empty_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("clean")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
No empty branches found.

"#]]);
    Ok(())
}

#[test]
fn removes_empty_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new empty-branch").assert().success();

    env.but("clean")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
  Deleted branch: empty-branch
✓ Deleted 1 empty branch(es)

"#]]);
    Ok(())
}

#[test]
fn dry_run_does_not_delete() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new empty-branch").assert().success();

    env.but("clean --dry-run")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Would delete branch: empty-branch
Found 1 empty branch(es)

"#]]);

    // Branch should still exist — clean again would still find it
    env.but("clean --dry-run")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Would delete branch: empty-branch
Found 1 empty branch(es)

"#]]);
    Ok(())
}

#[test]
fn does_not_remove_branch_with_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // A has a commit, so clean should find nothing
    env.but("clean").assert().success().stdout_eq(str![[r#"
No empty branches found.

"#]]);
    Ok(())
}

#[test]
fn json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new empty-branch").assert().success();

    env.but("--json clean")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "deleted": [
    {
      "name": "empty-branch"
    }
  ],
  "dry_run": false
}

"#]]);
    Ok(())
}

#[test]
fn json_output_dry_run() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new empty-branch").assert().success();

    env.but("--json clean --dry-run")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "deleted": [
    {
      "name": "empty-branch"
    }
  ],
  "dry_run": true
}

"#]]);
    Ok(())
}

#[test]
fn json_output_no_empty_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("--json clean")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "deleted": [],
  "dry_run": false
}

"#]]);
    Ok(())
}

#[test]
fn does_not_remove_branch_with_assigned_changes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a new empty branch
    env.but("branch new my-branch").assert().success();

    // Assign a file change to this branch's stack
    env.file("new-file.txt", "content");
    env.but("rub new-file.txt my-branch").assert().success();

    // my-branch has assigned changes, should not be cleaned
    env.but("clean").assert().success().stdout_eq(str![[r#"
No empty branches found.

"#]]);
    Ok(())
}

#[test]
fn creates_oplog_entry() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new empty-branch").assert().success();
    env.but("clean").assert().success();

    // Verify oplog has a CLEAN entry
    let output = env.but("oplog").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CLEAN"),
        "oplog should contain a CLEAN entry, got:\n{stdout}"
    );
    Ok(())
}
