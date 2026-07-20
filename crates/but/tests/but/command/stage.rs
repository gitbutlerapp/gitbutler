use snapbox::str;

use crate::{
    command::util::commit_file_with_worktree_changes_as_two_hunks,
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
fn assign_uncommitted_file() -> anyhow::Result<()> {
    let env = assigned_uncommitted_file_env()?;
    env.but("diff A@{stack}:a.txt")
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
fn uncommitted_file_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("stage a.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged all hunks in a.txt in the uncommitted area → [A].

"#]])
        .stderr_eq(str![""]);
}

#[test]
fn uncommitted_file_by_path_prefix_to_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "path/a.txt");

    env.but("stage path/ A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged hunk(s) → [A].

"#]])
        .stderr_eq(str![""]);
}

#[test]
fn uncommitted_hunk_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"]);

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Verify that the first hunk is j0, and move it to uncommitted.
    env.but("diff a.txt")
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
    env.but("stage nk:2 A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged a hunk in a.txt in the uncommitted area → [A].

"#]])
        .stderr_eq(str![""]);

    // Verify that only one hunk was assigned ("a.txt" appears both in the
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
fn filename_with_dash_not_treated_as_range() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("my-file.txt", "arbitrary text\n");

    // Staging by filename should work — the dash should NOT be interpreted as a range separator
    env.but("stage my-file.txt A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Staged the only hunk in my-file.txt in the uncommitted area → [A].

"#]])
        .stderr_eq(str![""]);
}

// Tests for convenience commands

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

Cannot stage to zz - it is the uncommitted area. Target must be a branch.

Hint: Use a branch name or branch cliId from `but status --format json -f`

"#]]);
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
        json["status"].get("uncommittedChanges").is_some(),
        "status should contain 'uncommittedChanges'"
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
