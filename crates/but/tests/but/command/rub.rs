use crate::utils::Sandbox;
use snapbox::str;

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

    // Must set metadata to match the scenario
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

    env.but("i0 00")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/rub/uncommitted-file-to-unassigned.stdout.term.svg"
        ])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn uncommitted_file_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("rub i0 A")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/rub/uncommitted-file-to-branch.stdout.term.svg"
        ])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn committed_file_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
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

    env.but("j0 00")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/rub/committed-file-to-unassigned.stdout.term.svg"
        ])
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
                  "cliId": "j0",
                  "filePath": "a.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "k0",
                  "filePath": "a.txt",
                  "changeType": "added"
                }
              ]
            },
            {
...
              "changes": [
                {
                  "cliId": "l0",
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
                  "cliId": "m0",
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

    // Must set metadata to match the scenario
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

    // TODO When we have a way to list the hunks and their respective IDs (e.g.
    //      via a "diff" or "show" command), assert that m0 is the hunk we want.
    env.but("m0 00")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/rub/uncommitted-hunk-to-unassigned.stdout.term.svg"
        ])
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

    // TODO When we have a way to list the hunks and their respective IDs (e.g.
    //      via a "diff" or "show" command), assert that m0 is the hunk we want.
    env.but("rub m0 A")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/rub/uncommitted-hunk-to-branch.stdout.term.svg"
        ])
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

mod util {
    use crate::utils::Sandbox;

    /// Create two files `filename1` and `filename2` and commit them to `branch`,
    /// each having two lines, `first_line`, then filler, and a last line that are far enough apart to
    /// ensure that they become 2 hunks when changed.
    pub fn commit_two_files_as_two_hunks_each(
        env: &Sandbox,
        branch: &str,
        filename1: &str,
        filename2: &str,
        first_line: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename1,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.file(
            filename2,
            format!("{first_line}\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!(
            "commit {branch} -m 'create {filename1} and {filename2}'"
        ))
        .assert()
        .success();
    }

    /// Create a file with `filename`, commit it to `branch`, then edit it once more to have two uncommitted hunks.
    pub fn commit_file_with_worktree_changes_as_two_hunks(
        env: &Sandbox,
        branch: &str,
        filename: &str,
    ) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename,
            format!("first\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but(format!("commit {branch} -m {filename}"))
            .assert()
            .success();
        env.file(
            filename,
            format!("firsta\n{}lasta\n", "line\n".repeat(context_distance)),
        );
    }
}
use crate::command::rub::util::{
    commit_file_with_worktree_changes_as_two_hunks, commit_two_files_as_two_hunks_each,
};
