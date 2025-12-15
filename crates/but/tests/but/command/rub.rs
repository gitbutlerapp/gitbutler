use snapbox::str;

use crate::utils::Sandbox;

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
fn uncommitted_hunk_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    env.file("a.txt", format!("first\n{}last\n", "line\n".repeat(100)));
    env.but("commit A -m create-a").assert().success();
    env.file("a.txt", format!("firsta\n{}lasta\n", "line\n".repeat(100)));

    // TODO When we have a way to list the hunks and their respective IDs (e.g.
    // via a "diff" or "show" command), assert that m0 is the hunk we want.
    env.but("m0 A")
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
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
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
