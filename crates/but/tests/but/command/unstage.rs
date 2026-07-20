use snapbox::str;

use crate::{
    command::util::{
        branch_commit_cli_ids, commit_file_with_worktree_changes_as_two_hunks,
        commit_two_files_as_two_hunks_each, status_json_with_files as status_json,
    },
    utils::{CommandExt, Sandbox},
};

fn assigned_uncommitted_file_env() -> anyhow::Result<Sandbox> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "arbitrary text\n");
    env.but("stage a.txt A").assert().success();
    Ok(env)
}

#[test]
fn uncommitted_file_to_uncommitted_area() -> anyhow::Result<()> {
    let env = assigned_uncommitted_file_env()?;
    env.but("unstage A@{stack}:a.txt")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstaged the only hunk in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    env.but("diff zz:a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────╮
n:c a.txt│
─────────╯
     1│+arbitrary text

"#]]);

    Ok(())
}

#[test]
fn shorthand_uncommitted_hunk_to_uncommitted_area() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Assign the change to A.
    env.but("stage a.txt A").assert().success();

    // Verify the displayed hunk IDs, then move the first hunk to uncommitted
    // using a shorter prefix than the displayed padded ID — the padding is
    // cosmetic and any unambiguous prefix of the full ID resolves.
    env.but("diff A@{stack}:a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────╮
n:2 a.txt│
─────────╯
   1  │-first
     1│+firsta
   2 2│ line
   3 3│ line
   4 4│ line
─────────╮
n:e a.txt│
─────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);
    env.but("unstage nk:2")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstaged a hunk in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk moved back to uncommitted ("a.txt" appears both in the
    // uncommitted area and in a stack).
    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "n#0",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "k0",
      "assignedChanges": [
        {
          "cliId": "n#1",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // First stage the file to A
    env.but("stage a.txt A").assert().success();

    // Verify it's assigned
    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [],
  "stacks": [
    {
      "cliId": "j0",
      "assignedChanges": [
        {
          "cliId": "n",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    // Now unstage it
    env.but("unstage A@{stack}:a.txt").assert().success();

    // Verify it's now uncommitted
    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "n",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "j0",
      "assignedChanges": [],
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("path/a.txt", "text\n");

    // First stage the file to A
    env.but("stage path/a.txt A").assert().success();

    // Now unstage it, giving a path prefix
    env.but("unstage A@{stack}:path/ A")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Unstaged hunk(s)

"#]]);
}

#[test]
fn unstage_command_with_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Stage the file to A
    env.but("stage a.txt A").assert().success();

    // Unstage with branch parameter
    env.but("unstage A@{stack}:a.txt A").assert().success();

    // Verify it's uncommitted
    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "n",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command_validation() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit CLI ID from status
    let status_output = env.but("--format json status").allow_json().output()?;
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_cli_id = status["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test that unstage rejects commits
    env.but(format!("unstage {commit_cli_id}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. '1' is a commit but must be an uncommitted file or hunk

"#]]);

    // Test that unstage rejects non-branch as branch parameter. Refresh the ID after adding a
    // commit, as duplicate change IDs can gain a new disambiguator.
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "c.txt");
    let commit_cli_id = branch_commit_cli_ids(&status_json(&env)?, "A")[0].clone();
    env.but(format!("unstage c.txt {commit_cli_id}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. Cannot unstage from 2 - it is a commit. Target must be a branch.

"#]]);

    Ok(())
}
