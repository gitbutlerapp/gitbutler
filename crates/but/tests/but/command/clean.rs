use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn no_empty_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.but("clean")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
No empty branches found.

"#]]);
}

#[test]
fn removes_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

    env.but("branch new empty-branch").assert().success();

    env.but("clean")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
  Deleted branch: empty-branch
✓ Deleted 1 empty branch(es)

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 7fa7db9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn dry_run_does_not_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

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
}

#[test]
fn does_not_remove_branch_with_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // A has a commit, so clean should find nothing
    env.but("clean").assert().success().stdout_eq(str![[r#"
No empty branches found.

"#]]);
}

#[test]
fn json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

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
}

#[test]
fn json_output_dry_run() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

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
}

#[test]
fn json_output_no_empty_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

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
}

#[test]
fn creates_oplog_entry() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new empty-branch").assert().success();
    env.but("clean").assert().success();

    // Verify oplog has a CLEAN entry
    let output = env.but("oplog").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CLEAN"),
        "oplog should contain a CLEAN entry, got:\n{stdout}"
    );
}
