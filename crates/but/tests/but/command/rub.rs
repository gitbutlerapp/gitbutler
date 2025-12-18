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
fn shorthand_uncommitted_hunk_to_unassigned() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    commit_file_a_with_worktree_changes_as_two_hunks(&env, "a.txt");

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

    commit_file_a_with_worktree_changes_as_two_hunks(&env, "a.txt");

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

    /// Create, then edit two lines that are far apart to ensure that they become 2 hunks.
    pub fn commit_file_a_with_worktree_changes_as_two_hunks(env: &Sandbox, filename: &str) {
        let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;
        env.file(
            filename,
            format!("first\n{}last\n", "line\n".repeat(context_distance)),
        );
        env.but("commit A -m create-a").assert().success();
        env.file(
            filename,
            format!("firsta\n{}lasta\n", "line\n".repeat(context_distance)),
        );
    }
}
use crate::command::rub::util::commit_file_a_with_worktree_changes_as_two_hunks;
