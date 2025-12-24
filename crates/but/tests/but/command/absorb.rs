use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn uncommitted_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("absorb i0")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/absorb/uncommitted-file.stdout.term.svg"
        ])
        .stderr_eq(str![""]);

    // Change was absorbed
    let repo = env.open_repo()?;
    let mut blob = repo.find_blob(repo.rev_parse_single(b"A:a.txt")?)?;
    insta::assert_snapshot!(String::from_utf8(blob.take_data())?, @r"
    firsta
    line
    line
    line
    line
    line
    line
    line
    lasta
    ");

    // Status is clean
    env.but("--json status -f")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "unassignedChanges": [],
...

"#]]);

    Ok(())
}

#[test]
fn uncommitted_hunk() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // TODO When we have a way to list the hunks and their respective IDs (e.g.
    //      via a "diff" or "show" command), assert that m0 is the hunk we want.
    env.but("absorb m0")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/absorb/uncommitted-hunk.stdout.term.svg"
        ])
        .stderr_eq(str![""]);

    // Change was partially absorbed
    let repo = env.open_repo()?;
    let mut blob = repo.find_blob(repo.rev_parse_single(b"A:a.txt")?)?;
    insta::assert_snapshot!(String::from_utf8(blob.take_data())?, @r"
    firsta
    line
    line
    line
    line
    line
    line
    line
    last
    ");

    // Status is not clean
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
...

"#]]);

    Ok(())
}

mod util {
    use crate::utils::Sandbox;

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
use crate::command::absorb::util::commit_file_with_worktree_changes_as_two_hunks;
