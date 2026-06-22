use snapbox::str;

use crate::{
    command::util::{
        branch_commit_id_for_file, branch_commit_ids,
        commit_file_with_worktree_changes_as_two_hunks, commit_two_files_as_two_hunks_each,
        status_json_with_files as status_json,
    },
    utils::{CommandExt, Sandbox},
};

fn assigned_uncommitted_file_env() -> anyhow::Result<Sandbox> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "arbitrary text\n");
    env.but("rub zz:a.txt A").assert().success();
    Ok(env)
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

fn unassigned_cli_id_for_file(status: &serde_json::Value, file_path: &str) -> Option<String> {
    status["unassignedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|change| {
            (change["filePath"].as_str().unwrap() == file_path)
                .then(|| change["cliId"].as_str().unwrap().to_string())
        })
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

fn committed_file_id_for_file(
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
                    commit["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find_map(|change| {
                            (change["filePath"].as_str().unwrap() == file_path)
                                .then(|| change["cliId"].as_str().unwrap().to_string())
                        })
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
    env.but("rub A@{stack}:a.txt zz")
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
fn uncommitted_file_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("rub a.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged all hunks in a.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);
}

#[test]
fn uncommitted_file_by_path_prefix_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "path/a.txt");

    env.but("rub path/ A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged hunk(s) → [A].

"#]])
        .stderr_eq(str![""]);
}

#[test]
fn committed_file_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    env.but("--format json status -f")
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

    env.but("rub e8:b.txt zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted changes

"#]])
        .stderr_eq(str![""]);

    // Verify that `status` reflects the move.
    env.but("--format json status -f")
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
      "changeType": "modified"
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
                  "cliId": "ce:nk",
                  "filePath": "a.txt",
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Assign the change to A.
    env.but("rub a.txt A").assert().success();

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
    env.but("rub j0 zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstaged a hunk in a.txt in a stack

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk moved back to unassigned ("a.txt" appears both in the
    // unassigned area and in a stack).
    env.but("--format json status -f")
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"]);

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
    env.but("--format json status -f")
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
fn filename_with_dash_not_treated_as_range() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("my-file.txt", "arbitrary text\n");

    // Staging by filename should work — the dash should NOT be interpreted as a range separator
    env.but("stage my-file.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged the only hunk in my-file.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);
}

// Tests for convenience commands

#[test]
fn uncommit_command_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env.but("--format json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test uncommit command
    env.but(format!("uncommit {commit_id}")).assert().success();

    // Verify the files are now unassigned
    env.but("--format json status -f")
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
fn uncommit_diff_json_keeps_mutation_result_and_diff() -> anyhow::Result<()> {
    fn run(agent: bool) -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

        env.setup_metadata(&["A", "B"]);
        commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

        let before = status_json(&env)?;
        let commit_id = branch_commit_ids(&before, "A")[0].clone();
        let command = format!("--format json uncommit {commit_id} --diff");
        let output = if agent {
            env.but(command)
                .env("AI_AGENT", "codex")
                .allow_json()
                .output()?
        } else {
            env.but(command).allow_json().output()?
        };
        assert!(
            output.status.success(),
            "uncommit --diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let result = if agent { &json["result"] } else { &json };
        assert_eq!(result["ok"], true);

        let changes = result["diff"]["changes"]
            .as_array()
            .expect("diff changes should be an array");
        assert!(
            changes
                .iter()
                .any(|change| change["path"].as_str() == Some("a.txt")),
            "diff output should include the uncommitted files"
        );

        if agent {
            assert!(
                json.get("status").is_some(),
                "agent JSON output should still include status"
            );
        }

        Ok(())
    }

    run(false)?;
    run(true)?;
    Ok(())
}

#[test]
fn uncommit_command_validation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
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
}

#[test]
fn uncommit_command_with_discard_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let commits_before = branch_commit_ids(&before, "A");
    let source_commit = commits_before[0].clone();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   fce8ecc create a.txt and b.txt
┊│     fc:nk A a.txt
┊│     fc:pn A b.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("uncommit {source_commit} --discard"))
        .assert()
        .success();

    let after = status_json(&env)?;
    let commits_after = branch_commit_ids(&after, "A");

    assert_eq!(
        commits_after.len() + 1,
        commits_before.len(),
        "discarding a commit via uncommit should remove that commit from branch history"
    );
    assert!(
        !commits_after.contains(&source_commit),
        "source commit should no longer be present after discard"
    );
    assert!(
        !unassigned_contains_file(&after, "a.txt") && !unassigned_contains_file(&after, "b.txt"),
        "discarding a commit should not move its changes into unassigned"
    );

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn uncommit_command_with_discard_on_committed_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let committed_file_id = committed_file_id_for_file(&before, "A", "b.txt")
        .expect("b.txt committed-file id should exist");

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   fce8ecc create a.txt and b.txt
┊│     fc:nk A a.txt
┊│     fc:pn A b.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("uncommit {committed_file_id} -d"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "b.txt"),
        "discarded committed file changes should not end up unassigned"
    );
    assert!(
        !branch_commits_contain_file(&after, "A", "b.txt"),
        "discarded committed file changes should no longer be in commit history"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "a.txt"),
        "other committed file changes should remain in history"
    );

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   993513d create a.txt and b.txt
┊│     99:nk A a.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn uncommit_help_mentions_discard_flag() -> anyhow::Result<()> {
    let env = Sandbox::empty();

    let output = env.but("uncommit --help").output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains("-d, --discard"),
        "expected uncommit help to list the discard flag"
    );
    assert!(
        stdout.contains("Discard the selected committed changes"),
        "expected uncommit help to describe discard behavior"
    );

    Ok(())
}

#[test]
fn agent_uncommit_discard_multiple_sources_writes_single_json_with_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    let before = status_json(&env)?;
    let commits_before = branch_commit_ids(&before, "A");
    let sources = format!("{},{}", commits_before[0], commits_before[1]);

    let output = env
        .but(format!("--format json uncommit {sources} --discard"))
        .env("AI_AGENT", "codex")
        .allow_json()
        .output()?;
    assert!(output.status.success());

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed["result"]["ok"], serde_json::json!(true));
    assert!(
        parsed.get("status").is_some(),
        "agent JSON wrapper should include status"
    );

    let after = status_json(&env)?;
    let commits_after = branch_commit_ids(&after, "A");

    assert_eq!(
        commits_after.len() + 2,
        commits_before.len(),
        "discarding two commit sources should remove both from branch history"
    );
    assert!(
        !unassigned_contains_file(&after, "a.txt") && !unassigned_contains_file(&after, "b.txt"),
        "discarded commits should not move their changes into unassigned"
    );

    Ok(())
}

#[test]
fn stage_command() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Test stage command
    env.but("stage a.txt A").assert().success();

    // Verify the file is assigned to A
    env.but("--format json status -f")
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
fn stage_command_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("path/a.txt", "text\n");
    env.but("stage path/ A")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Staged hunk(s) → [A].

"#]]);
}

#[test]
fn stage_command_missing_source_hints_to_refresh_cli_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.but("stage missing-file A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'missing-file' for '<FILE_OR_HUNK>'

Source 'missing-file' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.

Hint: Run `but status --format json -f` to refresh CLI IDs, then retry with a file or hunk cliId from the output

"#]]);
}

#[test]
fn stage_command_missing_branch_hints_to_refresh_cli_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "text\n");

    env.but("stage a.txt missing-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'missing-branch' for '<BRANCH>'

Branch 'missing-branch' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.

Hint: Use a branch name or branch cliId from `but status --format json -f`

"#]]);
}

#[test]
fn stage_command_non_branch_target_hints_to_use_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "text\n");

    env.but("stage a.txt zz")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'zz' for '<BRANCH>'

Cannot stage to zz - it is the unassigned area. Target must be a branch.

Hint: Use a branch name or branch cliId from `but status --format json -f`

"#]]);
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
    env.but("--format json status -f")
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

    // Verify it's unassigned
    env.but("--format json status -f")
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env.but("--format json status").allow_json().output()?;
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("uncommitted-to-commit.txt", "content\n");

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub uncommitted-to-commit.txt {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended [..] → [..]

"#]])
        .stderr_eq(str![""]);

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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("uncommitted-to-stack.txt", "content\n");

    env.but("rub uncommitted-to-stack.txt A@{stack}")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in uncommitted-to-stack.txt in the unassigned area → stack [..].

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "A", "uncommitted-to-stack.txt"),
        "file should be assigned to A stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("zz-to-branch.txt", "content\n");

    env.but("rub zz A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all unstaged changes to [A].

"#]])
        .stderr_eq(str![""]);

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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("zz-to-commit.txt", "content\n");

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub zz {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended unassigned files → [..]

"#]])
        .stderr_eq(str![""]);

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
fn rub_matrix_unassigned_to_commit_consumes_renames() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let original = (1..=120)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("rename-source.txt", &original);
    env.but("commit A -m 'seed rename source'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("rename-source.txt"),
        env.projects_root().join("rename-target.txt"),
    )?;
    env.file(
        "rename-target.txt",
        original.replace("40\n41\n42\n", "40\nchanged\n42\n"),
    );

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub zz {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended unassigned files → [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "rename-target.txt"),
        "renamed file should no longer be unassigned"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "expected all zz changes to be committed"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_file_to_commit_consumes_renames() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let original = (1..=120)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("rename-source-single.txt", &original);
    env.but("commit A -m 'seed rename source single'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("rename-source-single.txt"),
        env.projects_root().join("rename-target-single.txt"),
    )?;
    env.file(
        "rename-target-single.txt",
        original.replace("70\n71\n72\n", "70\nchanged\n72\n"),
    );

    let before = status_json(&env)?;
    let source_file_id = unassigned_cli_id_for_file(&before, "rename-target-single.txt")
        .expect("renamed unassigned file should be present in status");
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub {source_file_id} {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended the only hunk in rename-target-single.txt in the unassigned area → [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "rename-target-single.txt"),
        "renamed file should no longer be unassigned"
    );

    let remaining = env.invoke_git("status --porcelain");
    assert_eq!(
        remaining, "",
        "expected selected renamed file to be committed; remaining status:\n{remaining}"
    );

    Ok(())
}

#[test]
fn rub_unassigned_deleted_file_to_commit_keeps_unrelated_deleted_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("a.txt", "a\n");
    env.file("b.txt", "b\n");
    env.file("c.txt", "c\n");
    env.but("commit A -m 'Add a.txt, b.txt, and c.txt'")
        .assert()
        .success();

    std::fs::remove_file(env.projects_root().join("a.txt"))?;
    std::fs::remove_file(env.projects_root().join("b.txt"))?;

    let before = status_json(&env)?;
    let source_file_id = unassigned_cli_id_for_file(&before, "a.txt")
        .expect("a.txt deletion should be present in the unassigned area");
    let target_commit = branch_commit_ids(&before, "A")[0].clone();
    assert!(
        unassigned_contains_file(&before, "b.txt"),
        "b.txt deletion should start in the unassigned area"
    );

    env.but(format!("rub {source_file_id} {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended the only hunk in a.txt in the unassigned area → [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        !unassigned_contains_file(&after, "a.txt"),
        "selected a.txt deletion should be amended into the target commit"
    );
    assert!(
        unassigned_contains_file(&after, "b.txt"),
        "unrelated b.txt deletion should remain unassigned"
    );
    assert!(
        !env.projects_root().join("a.txt").exists(),
        "selected a.txt deletion should stay applied to the worktree"
    );
    assert!(
        !env.projects_root().join("b.txt").exists(),
        "unrelated b.txt deletion should stay applied to the worktree"
    );
    assert!(
        env.projects_root().join("c.txt").exists(),
        "untouched c.txt should stay in the worktree"
    );

    Ok(())
}

#[test]
fn rub_matrix_unassigned_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("zz-to-stack.txt", "content\n");

    env.but("rub zz A@{stack}")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all unstaged changes to [A].

"#]])
        .stderr_eq(str![""]);

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
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let commits_before = branch_commit_ids(&before, "A");
    let source_commit = commits_before[0].clone();

    env.but(format!("rub {source_commit} zz"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Uncommitted [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    let commits_after = branch_commit_ids(&after, "A");

    assert_eq!(
        commits_after.len() + 1,
        commits_before.len(),
        "uncommitting a commit should remove that commit from branch history"
    );
    assert!(
        !commits_after.contains(&source_commit),
        "source commit should no longer be present after uncommit"
    );

    assert!(
        unassigned_contains_file(&after, "a.txt") && unassigned_contains_file(&after, "b.txt"),
        "uncommitting a commit should move its changes into unassigned"
    );

    Ok(())
}

#[test]
fn rub_matrix_commit_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    let before = status_json(&env)?;
    let commits_before = branch_commit_ids(&before, "A");
    let source_commit = commits_before[0].clone();
    let target_commit = commits_before[1].clone();

    env.but(format!("rub {source_commit} {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Squashed [..] → [..]

"#]])
        .stderr_eq(str![""]);

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
fn rub_commit_without_message_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one.txt", "one.txt contents");
    env.but("commit -m 'add one.txt'").assert().success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   aec35ac add one.txt
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);

    env.but("commit empty --after aec35ac").assert().success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   5e5c05a (no commit message) (no changes)
┊●   aec35ac add one.txt
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);

    env.but("rub 5e5c05a aec35ac").assert().success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   aec35ac add one.txt
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);
}

#[test]
fn rub_commit_to_commit_without_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one.txt", "one.txt contents");
    env.but("commit -m 'add one.txt'").assert().success();
    env.but("commit empty --after aec35ac").assert().success();

    env.but("rub aec35ac 5e5c05a").assert().success();

    let status = status_json(&env)?;
    let branch = status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == "A")
        .unwrap();
    let commit_messages = branch["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|commit| commit["message"].as_str().unwrap().trim_end_matches('\n'))
        .collect::<Vec<_>>();

    assert_eq!(commit_messages, vec!["add one.txt", "add A"]);

    Ok(())
}

#[test]
fn rub_matrix_commit_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();
    let branch_b_count_before = branch_commit_ids(&before, "B").len();

    env.but(format!("rub {source_commit} B"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved [..] → [B]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    let branch_b_count_after = branch_commit_ids(&after, "B").len();
    assert!(
        branch_b_count_after > branch_b_count_before,
        "moving a commit to B should increase commit count on B"
    );

    Ok(())
}

#[test]
fn rub_matrix_commit_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub {source_commit} B@{{stack}}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Uncommitted [..] to [B]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    let commits_after = branch_commit_ids(&after, "A");
    assert!(
        !commits_after.contains(&source_commit),
        "source commit should no longer be present in branch A after uncommit to stack"
    );
    assert!(
        stack_assigned_contains_file(&after, "B", "a.txt")
            && stack_assigned_contains_file(&after, "B", "b.txt"),
        "source commit files should be assigned to branch B stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_unassigned_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("branch-to-zz.txt", "content\n");
    env.but("rub branch-to-zz.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in branch-to-zz.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A zz")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Unstaged all [A] changes.

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        unassigned_contains_file(&after, "branch-to-zz.txt"),
        "file should move back to unassigned"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("branch-to-stack.txt", "content\n");
    env.but("rub branch-to-stack.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in branch-to-stack.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A B@{stack}")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all [A] changes to [B].

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "branch-to-stack.txt"),
        "file should be reassigned to B stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("branch-to-commit.txt", "content\n");
    env.but("rub branch-to-commit.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in branch-to-commit.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub A {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended assigned files [A] → [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        branch_commits_contain_file(&after, "A", "branch-to-commit.txt"),
        "file should be amended into a commit on branch A"
    );

    Ok(())
}

#[test]
fn rub_matrix_branch_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("branch-to-branch.txt", "content\n");
    env.but("rub branch-to-branch.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in branch-to-branch.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A B")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all [A] changes to [B].

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "branch-to-branch.txt"),
        "file should be reassigned to branch B"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_unassigned_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("stack-to-zz.txt", "content\n");
    env.but("rub stack-to-zz.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in stack-to-zz.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A@{stack} zz")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Unstaged all [A] changes.

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        unassigned_contains_file(&after, "stack-to-zz.txt"),
        "file should move back to unassigned"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_stack_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("stack-to-stack.txt", "content\n");
    env.but("rub stack-to-stack.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in stack-to-stack.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A@{stack} B@{stack}")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all [A] changes to [B].

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "stack-to-stack.txt"),
        "file should be reassigned to B stack"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("stack-to-branch.txt", "content\n");
    env.but("rub stack-to-branch.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in stack-to-branch.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    env.but("rub A@{stack} B")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged all [A] changes to [B].

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "stack-to-branch.txt"),
        "file should be reassigned to B branch"
    );

    Ok(())
}

#[test]
fn rub_matrix_stack_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("stack-to-commit.txt", "content\n");
    env.but("rub stack-to-commit.txt A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Staged the only hunk in stack-to-commit.txt in the unassigned area → [A].

"#]])
        .stderr_eq(str![""]);

    let before = status_json(&env)?;
    let target_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub A@{{stack}} {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended files assigned to [A] → [..]

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        !stack_assigned_contains_file(&after, "A", "stack-to-commit.txt"),
        "file should no longer be assigned to stack A"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "stack-to-commit.txt"),
        "file should be amended into a commit on branch A"
    );

    Ok(())
}

#[test]
fn rub_matrix_committed_file_to_branch_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let source_commit = branch_commit_ids(&before, "A")[0].clone();

    env.but(format!("rub {source_commit}:a.txt B"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Uncommitted changes

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        stack_assigned_contains_file(&after, "B", "a.txt"),
        "file extracted from commit should be assigned to B"
    );

    Ok(())
}

#[test]
fn rub_matrix_committed_file_to_commit_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    commit_two_files_as_two_hunks_each(&env, "A", "source-a.txt", "source-b.txt", "source commit");
    commit_two_files_as_two_hunks_each(&env, "A", "target-a.txt", "target-b.txt", "target commit");

    let before = status_json(&env)?;
    let source_commit =
        branch_commit_id_for_file(&before, "A", "source-a.txt").expect("source commit with file");
    let target_commit =
        branch_commit_id_for_file(&before, "A", "target-a.txt").expect("target commit with file");

    env.but(format!("rub {source_commit}:source-a.txt {target_commit}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved files between commits!

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env)?;
    assert!(
        branch_commits_contain_file(&after, "A", "source-a.txt"),
        "file should still be present in branch A history after moving between commits"
    );

    Ok(())
}

#[test]
fn rub_matrix_invalid_pairs_smoke() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

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

    env.but("rub A invalid-a.txt")
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
fn agent_json_wraps_mutation_and_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("a.txt", "arbitrary text\n");

    let output = env
        .but("--format json stage a.txt A")
        .env("AI_AGENT", "codex")
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
fn agent_invocation_enables_status_after_for_mutations() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("agent.txt", "content\n");

    let output = env
        .but("--format json stage agent.txt A")
        .env("AI_AGENT", "codex")
        .allow_json()
        .output()?;
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(
        json.get("result").is_some(),
        "agent mutation output should include the command result"
    );
    assert!(
        json.get("status").is_some(),
        "agent mutation output should include workspace status"
    );

    Ok(())
}

#[test]
fn agent_json_success_has_no_status_error_field() -> anyhow::Result<()> {
    // Verifies that on a successful agent mutation, the combined JSON output
    // contains {result, status} but NOT status_error.
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("b.txt", "content\n");

    let output = env
        .but("--format json stage b.txt A")
        .env("AI_AGENT", "codex")
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

#[test]
fn rubbing_modified_and_renamed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");
    env.file("file-2", "content-2");

    env.but("commit -m 'add files'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e3f869d add files
┊│     e3:qs A file
┊│     e3:kw A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("file-2", "new content");
    env.rename_file("file-2", "file");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes]
┊   qs M file
┊   kw D file-2
┊
┊╭┄br [a-branch-1]
┊●   e3f869d add files
┊│     e3:qs A file
┊│     e3:kw A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("rub zz e3f869d").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   3a32c97 add files
┊│     3a:qs A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_modified_and_renamed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");
    env.file("file-2", "content-2");

    env.but("commit -m 'add files'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e3f869d add files
┊│     e3:qs A file
┊│     e3:kw A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("file-2", "new content");
    env.rename_file("file-2", "file");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes]
┊   qs M file
┊   kw D file-2
┊
┊╭┄br [a-branch-1]
┊●   e3f869d add files
┊│     e3:qs A file
┊│     e3:kw A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("commit -m 'change file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e419886 change file
┊│     e4:qs M file
┊│     e4:kw D file-2
┊●   e3f869d add files
┊│     e3:qs A file
┊│     e3:kw A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
