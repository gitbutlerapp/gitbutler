use snapbox::str;

use crate::{
    command::util::{
        commit_file_with_worktree_changes_as_two_hunks, commit_two_files_as_two_hunks_each,
    },
    utils::Sandbox,
};

#[test]
fn shorthand_without_subcommand() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    // Test that calling `but <id1> <id2>` defaults to rub
    // This should fail with a CliId not found error rather than a command not found error
    env.but("nonexistent1 nonexistent2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Source 'nonexistent1' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.

"#]]);

    Ok(())
}

#[test]
fn uncommitted_file_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Assign the change to A and verify that the assignment happened.
    env.but("i0 A").assert().success();
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "i0",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    env.but("i0 zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unassigned all hunks in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn uncommitted_file_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("rub i0 A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Assigned all hunks in a.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn committed_file_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
...
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
...
              "changes": [
                {
                  "cliId": "m0",
                  "filePath": "a.txt",
                  "changeType": "modified"
                },
                {
                  "cliId": "n0",
                  "filePath": "b.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "i0",
                  "filePath": "a.txt",
                  "changeType": "added"
                },
                {
                  "cliId": "j0",
                  "filePath": "b.txt",
                  "changeType": "added"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "k0",
                  "filePath": "A",
                  "changeType": "added"
                }
              ]
            }
...
    },
    {
      "cliId": "h0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "h0",
          "name": "B",
          "commits": [
            {
...
              "changes": [
                {
                  "cliId": "l0",
                  "filePath": "B",
                  "changeType": "added"
                }
              ]
            }
...

"#]]);

    env.but("j0 zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted changes

"#]])
        .stderr_eq(str![""]);

    // Verify that `status` reflects the move.
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "b.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
...
              "changes": [
                {
                  "cliId": "k0",
                  "filePath": "a.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "l0",
                  "filePath": "a.txt",
                  "changeType": "added"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "m0",
                  "filePath": "A",
                  "changeType": "added"
                }
...
    },
    {
      "cliId": "h0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "h0",
          "name": "B",
          "commits": [
            {
...
              "changes": [
                {
                  "cliId": "n0",
                  "filePath": "B",
                  "changeType": "added"
                }
              ]
            }
...

"#]]);

    Ok(())
}

#[test]
fn shorthand_uncommitted_hunk_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Assign the change to A and verify that the assignment happened.
    env.but("i0 A").assert().success();
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "i0",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    // Verify that the first hunk is j0, and move it to unassigned.
    env.but("diff i0")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
   1  │-first
     1│+firsta
   2 2│ line
   3 3│ line
   4 4│ line
────────╮
k0 a.txt│
────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);
    env.but("j0 zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unassigned a hunk in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk moved back to unassigned ("a.txt" appears both in the
    // unassigned area and in a stack).
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "j0",
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
fn uncommitted_hunk_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Verify that the first hunk is j0, and move it to unassigned.
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
   1  │-first
     1│+firsta
   2 2│ line
   3 3│ line
   4 4│ line
────────╮
k0 a.txt│
────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);
    env.but("rub j0 A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Assigned a hunk in a.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk was assigned ("a.txt" appears both in the
    // unassigned area and in a stack).
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "j0",
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

// Tests for convenience commands

#[test]
fn uncommit_command_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env
        .but("--json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test uncommit command
    env.but(format!("uncommit {}", commit_id))
        .assert()
        .success();

    // Verify the files are now unassigned
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "a.txt",
      "changeType": "added"
    },
    {
      "cliId": "j0",
      "filePath": "b.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [],
      "branches": [
...

"#]]);

    Ok(())
}

#[test]
fn uncommit_command_validation() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Test that uncommit rejects uncommitted files
    env.but("uncommit i0")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit i0 - it is an uncommitted file or hunk. Only commits and files-in-commits can be uncommitted.

"#]]);

    // Test that uncommit rejects branches
    env.but("uncommit A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit g0 - it is a branch. Only commits and files-in-commits can be uncommitted.

"#]]);

    Ok(())
}

#[test]
fn stage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Test stage command
    env.but("stage i0 A").assert().success();

    // Verify the file is assigned to A
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "i0",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // First stage the file to A
    env.but("stage i0 A").assert().success();

    // Verify it's assigned
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [
        {
          "cliId": "i0",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    // Now unstage it
    env.but("unstage i0").assert().success();

    // Verify it's now unassigned
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "g0",
      "assignedChanges": [],
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command_with_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Stage the file to A
    env.but("stage i0 A").assert().success();

    // Unstage with branch parameter
    env.but("unstage i0 A").assert().success();

    // Verify it's unassigned
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "i0",
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env
        .but("--json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test that unstage rejects commits
    env.but(format!("unstage {}", commit_id))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. Cannot unstage 3f - it is a commit. Only uncommitted files and hunks can be unstaged.

"#]]);

    // Test that unstage rejects non-branch as branch parameter
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "c.txt");
    env.but(format!("unstage i0 {}", commit_id))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. Cannot unstage from 3f - it is a commit. Target must be a branch.

"#]]);

    Ok(())
}
