use snapbox::str;

use crate::{
    command::util::{
        commit_file_with_worktree_changes_as_two_hunks, commit_two_files_as_two_hunks_each,
    },
    utils::{CommandExt, Sandbox},
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

fn assigned_uncommitted_file_env() -> anyhow::Result<Sandbox> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    env.file("a.txt", "arbitrary text\n");
    env.but("zz:a.txt A").assert().success();
    Ok(env)
}

fn status_json(env: &Sandbox) -> anyhow::Result<serde_json::Value> {
    let output = env.but("--json status -f").allow_json().output()?;
    assert!(output.status.success());
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn branch_commit_ids(status: &serde_json::Value, branch_name: &str) -> Vec<String> {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == branch_name)
        .unwrap()["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|commit| commit["cliId"].as_str().unwrap().to_string())
        .collect()
}

fn stack_assigned_contains_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"].as_array().unwrap().iter().any(|stack| {
        let has_branch = stack["branches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|branch| branch["name"].as_str().unwrap() == branch_name);
        has_branch
            && stack["assignedChanges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|change| change["filePath"].as_str().unwrap() == file_path)
    })
}

fn unassigned_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
    status["unassignedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

fn branch_commits_contain_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .filter(|branch| branch["name"].as_str().unwrap() == branch_name)
        .flat_map(|branch| branch["commits"].as_array().unwrap().iter())
        .flat_map(|commit| commit["changes"].as_array().unwrap().iter())
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

fn branch_commit_id_for_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> Option<String> {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == branch_name)
        .and_then(|branch| {
            branch["commits"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|commit| {
                    let has_file = commit["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|change| change["filePath"].as_str().unwrap() == file_path);
                    if has_file {
                        Some(commit["cliId"].as_str().unwrap().to_string())
                    } else {
                        None
                    }
                })
        })
}

#[test]
fn assign_uncommitted_file() -> anyhow::Result<()> {
    let env = assigned_uncommitted_file_env()?;
    env.but("diff A@{stack}:a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
     1│+arbitrary text

"#]]);
    Ok(())
}

#[test]
fn uncommitted_file_to_unassigned() -> anyhow::Result<()> {
    let env = assigned_uncommitted_file_env()?;
    env.but("A@{stack}:a.txt zz")
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
────────╮
j0 a.txt│
────────╯
     1│+arbitrary text

"#]]);

    Ok(())
}

#[test]
fn uncommitted_file_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("rub a.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged all hunks in a.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn uncommitted_file_by_path_prefix_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "path/a.txt");

    env.but("rub path/ A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged hunk(s) → [A].

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
        .allow_json()
        .assert()
        .success()
        // .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
...
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "i0",
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
                  "cliId": "e8:nk",
                  "filePath": "a.txt",
                  "changeType": "modified"
                },
                {
                  "cliId": "e8:pn",
                  "filePath": "b.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "fc:nk",
                  "filePath": "a.txt",
                  "changeType": "added"
                },
                {
                  "cliId": "fc:pn",
                  "filePath": "b.txt",
                  "changeType": "added"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "94:tm",
                  "filePath": "A",
                  "changeType": "added"
                }
              ]
            }
...

"#]]);

    env.but("fc:b.txt zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted changes

"#]])
        .stderr_eq(str![""]);

    // Verify that `status` reflects the move.
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "pn",
      "filePath": "b.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "k0",
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
                  "cliId": "76:nk",
                  "filePath": "a.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "99:nk",
                  "filePath": "a.txt",
                  "changeType": "added"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "94:tm",
                  "filePath": "A",
                  "changeType": "added"
                }
...
    },
    {
      "cliId": "l0",
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
                  "cliId": "d3:pl",
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

    // Assign the change to A.
    env.but("a.txt A").assert().success();

    // Verify that the first hunk is j0, and move it to unassigned.
    env.but("diff A@{stack}:a.txt")
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
Unstaged a hunk in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk moved back to unassigned ("a.txt" appears both in the
    // unassigned area and in a stack).
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "m0",
      "assignedChanges": [
        {
          "cliId": "km",
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
Staged a hunk in a.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk was assigned ("a.txt" appears both in the
    // unassigned area and in a stack).
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "m0",
      "assignedChanges": [
        {
          "cliId": "km",
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

// Regression: filenames with dashes should not be misinterpreted as ranges.
// Before the fix, "my-file.txt" would be split on '-' and treated as a range
// from "my" to "file.txt", which would fail.
#[test]
fn filename_with_dash_not_treated_as_range() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    env.file("my-file.txt", "arbitrary text\n");

    // Staging by filename should work — the dash should NOT be interpreted as a range separator
    env.but("stage my-file.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged the only hunk in my-file.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

// Tests for convenience commands

#[test]
fn uncommit_command_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test uncommit command
    env.but(format!("uncommit {commit_id}")).assert().success();

    // Verify the files are now unassigned
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "added"
    },
    {
      "cliId": "pn",
      "filePath": "b.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "m0",
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
    env.but("uncommit a.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit a.txt - it is an uncommitted file or hunk. Only commits and files-in-commits can be uncommitted.

"#]]);

    // Test that uncommit rejects branches
    env.but("uncommit A").assert().failure().stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit A - it is a branch. Only commits and files-in-commits can be uncommitted.

"#]]);

    Ok(())
}

#[test]
fn stage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Test stage command
    env.but("stage a.txt A").assert().success();

    // Verify the file is assigned to A
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "l0",
      "assignedChanges": [
        {
          "cliId": "km",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    Ok(())
}

#[test]
fn stage_command_path_prefix() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    env.file("path/a.txt", "text\n");
    env.but("stage path/ A")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Staged hunk(s) → [A].

"#]]);
    Ok(())
}

#[test]
fn unstage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // First stage the file to A
    env.but("stage a.txt A").assert().success();

    // Verify it's assigned
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
  "stacks": [
    {
      "cliId": "l0",
      "assignedChanges": [
        {
          "cliId": "km",
          "filePath": "a.txt",
          "changeType": "modified"
        }
      ],
...

"#]]);

    // Now unstage it
    env.but("unstage A@{stack}:a.txt").assert().success();

    // Verify it's now unassigned
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
    {
      "cliId": "l0",
      "assignedChanges": [],
...

"#]]);

    Ok(())
}

#[test]
fn unstage_command_path_prefix() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
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
    Ok(())
}

#[test]
fn unstage_command_with_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Stage the file to A
    env.but("stage a.txt A").assert().success();

    // Unstage with branch parameter
    env.but("unstage A@{stack}:a.txt A").assert().success();

    // Verify it's unassigned
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [
    {
      "cliId": "nk",
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
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test that unstage rejects commits
    env.but(format!("unstage {commit_id}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. Cannot unstage fc - it is a commit. Only uncommitted files and hunks can be unstaged.

"#]]);

    // Test that unstage rejects non-branch as branch parameter
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "c.txt");
    env.but(format!("unstage c.txt {commit_id}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unstage. Cannot unstage from fc - it is a commit. Target must be a branch.

"#]]);

    Ok(())
}

// Full rub matrix CLI smoke coverage.

#[test]
fn rub_matrix_uncommitted_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("uncommitted-to-commit.txt", "content\n");

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub uncommitted-to-commit.txt {target_commit}"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "uncommitted-to-commit.txt"),
        "file should no longer be unassigned"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "uncommitted-to-commit.txt"),
        "file should appear in commits on branch A"
    );

    Ok(())
}

#[test]
fn rub_matrix_uncommitted_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("uncommitted-to-stack.txt", "content\n");

    env.but("rub uncommitted-to-stack.txt A@{stack}")
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "A", "uncommitted-to-stack.txt"),
        "file should be assigned to A stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("zz-to-branch.txt", "content\n");

    env.but("rub zz A").assert().success();

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "zz-to-branch.txt"),
        "file should no longer be unassigned"
    );
    assert!(
        stack_assigned_contains_file(&after, "A", "zz-to-branch.txt"),
        "file should be assigned to branch A stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("zz-to-commit.txt", "content\n");

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub zz {target_commit}"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "zz-to-commit.txt"),
        "file should no longer be unassigned"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "zz-to-commit.txt"),
        "file should appear in commits on branch A"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("zz-to-stack.txt", "content\n");

    env.but("rub zz A@{stack}").assert().success();

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "zz-to-stack.txt"),
        "file should no longer be unassigned"
    );
    assert!(
        stack_assigned_contains_file(&after, "A", "zz-to-stack.txt"),
        "file should be assigned to A stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_commit_to_unassigned_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub {source_commit} zz"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        after["unassignedChanges"].as_array().unwrap().len() >= 2,
        "uncommitting a commit should produce unassigned changes"
    );

    Ok(())
}

#[test]
fn rub_matrix_commit_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    let before = status_json(&env)?;
    let commits_before = branch_commit_ids(&before, "A");
    let source_commit = commits_before[0].clone();
    let target_commit = commits_before[1].clone();

    env.but(format!("rub {source_commit} {target_commit}"))
        .assert()
        .success();

    let after = status_json(&env)?;
    let commits_after = branch_commit_ids(&after, "A");
    assert_eq!(
        commits_after.len() + 1,
        commits_before.len(),
        "squashing should reduce commit count by one"
    );

    Ok(())
}

#[test]
fn rub_matrix_commit_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();
    let branch_b_count_before = branch_commit_ids(&before, "B").len();

    env.but(format!("rub {source_commit} B")).assert().success();

    let after = status_json(&env)?;
    let branch_b_count_after = branch_commit_ids(&after, "B").len();
    assert!(
        branch_b_count_after > branch_b_count_before,
        "moving a commit to B should increase commit count on B"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_unassigned_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("branch-to-zz.txt", "content\n");
    env.but("rub branch-to-zz.txt A").assert().success();

    env.but("rub A zz").assert().success();

    let after = status_json(&env)?;
    assert!(
        unassigned_contains_file(&after, "branch-to-zz.txt"),
        "file should move back to unassigned"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("branch-to-stack.txt", "content\n");
    env.but("rub branch-to-stack.txt A").assert().success();

    env.but("rub A B@{stack}").assert().success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "branch-to-stack.txt"),
        "file should be reassigned to B stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("branch-to-commit.txt", "content\n");
    env.but("rub branch-to-commit.txt A").assert().success();

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub A {target_commit}")).assert().success();

    let after = status_json(&env)?;
    assert!(
        branch_commits_contain_file(&after, "A", "branch-to-commit.txt"),
        "file should be amended into a commit on branch A"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("branch-to-branch.txt", "content\n");
    env.but("rub branch-to-branch.txt A").assert().success();

    env.but("rub A B").assert().success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "branch-to-branch.txt"),
        "file should be reassigned to branch B"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_unassigned_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("stack-to-zz.txt", "content\n");
    env.but("rub stack-to-zz.txt A").assert().success();

    env.but("rub A@{stack} zz").assert().success();

    let after = status_json(&env)?;
    assert!(
        unassigned_contains_file(&after, "stack-to-zz.txt"),
        "file should move back to unassigned"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("stack-to-stack.txt", "content\n");
    env.but("rub stack-to-stack.txt A").assert().success();

    env.but("rub A@{stack} B@{stack}").assert().success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "stack-to-stack.txt"),
        "file should be reassigned to B stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    env.file("stack-to-branch.txt", "content\n");
    env.but("rub stack-to-branch.txt A").assert().success();

    env.but("rub A@{stack} B").assert().success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "stack-to-branch.txt"),
        "file should be reassigned to B branch"
    );

    Ok(())
}

#[test]
fn rub_matrix_committed_file_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub {source_commit}:a.txt B"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "a.txt"),
        "file extracted from commit should be assigned to B"
    );

    Ok(())
}

#[test]
fn rub_matrix_committed_file_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "source-a.txt", "source-b.txt", "source commit");
    commit_two_files_as_two_hunks_each(&env, "A", "target-a.txt", "target-b.txt", "target commit");

    let before = status_json(&env)?;
    let source_commit =
        branch_commit_id_for_file(&before, "A", "source-a.txt").expect("source commit with file");
    let target_commit =
        branch_commit_id_for_file(&before, "A", "target-a.txt").expect("target commit with file");

    env.but(format!("rub {source_commit}:source-a.txt {target_commit}"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        branch_commits_contain_file(&after, "A", "source-a.txt"),
        "file should still be present in branch A history after moving between commits"
    );

    Ok(())
}

#[test]
fn rub_matrix_invalid_pairs_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "invalid matrix setup");
    env.file("invalid-a.txt", "content\n");
    env.file("invalid-b.txt", "content\n");

    let status = status_json(&env)?;
    let commit = branch_commit_ids(&status, "A")[0].clone();

    env.but("rub invalid-a.txt invalid-b.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    env.but(format!("rub {commit} A@{{stack}}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    env.but("rub A invalid-a.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    env.but(format!("rub A@{{stack}} {commit}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    env.but("rub zz zz").assert().failure().stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    env.but(format!("rub {commit}:a.txt A@{{stack}}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Rubbed the wrong way. Operation doesn't make sense.[..]

"#]]);

    Ok(())
}

#[test]
fn status_after_json_wraps_mutation_and_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    env.file("a.txt", "arbitrary text\n");

    // Use --json --status-after with a stage operation
    let output = env
        .but("--json --status-after stage a.txt A")
        .allow_json()
        .output()?;
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // The combined output must have both "result" and "status" fields
    assert!(
        json.get("result").is_some(),
        "expected 'result' field in combined JSON output"
    );
    assert!(
        json.get("status").is_some(),
        "expected 'status' field in combined JSON output"
    );

    // The result should contain the mutation output (stage produces {"ok": true})
    assert_eq!(
        json["result"]["ok"], true,
        "mutation result should indicate success"
    );

    // The status should have standard status fields
    assert!(
        json["status"].get("stacks").is_some(),
        "status should contain 'stacks'"
    );
    assert!(
        json["status"].get("unassignedChanges").is_some(),
        "status should contain 'unassignedChanges'"
    );

    Ok(())
}

#[test]
fn status_after_json_success_has_no_status_error_field() -> anyhow::Result<()> {
    // Verifies that on a successful mutation with --status-after, the combined
    // JSON output contains {result, status} but NOT status_error.
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    env.file("b.txt", "content\n");

    let output = env
        .but("--json --status-after stage b.txt A")
        .allow_json()
        .output()?;
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // On success, status_error should NOT be present
    assert!(
        json.get("status_error").is_none(),
        "status_error should not be present on success"
    );

    Ok(())
}
