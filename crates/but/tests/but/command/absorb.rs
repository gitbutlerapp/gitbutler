use bstr::ByteSlice;
use snapbox::str;

use crate::{command::util::commit_file_with_worktree_changes_as_two_hunks, utils::Sandbox};

#[test]
fn uncommitted_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

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

    env.but("absorb i0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 2 changed files to absorb:

Absorbed to commit: 4fa8217 a.txt
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4
    a.txt @6,4 +6,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    // Change was absorbed
    let repo = env.open_repo()?;
    let blob = repo.rev_parse_single(b"A:a.txt")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @r"
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

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Verify that the first hunk is j0, and absorb it.
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
    env.but("absorb j0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: 4fa8217 a.txt
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    // Change was partially absorbed
    let repo = env.open_repo()?;
    let blob = repo.rev_parse_single(b"A:a.txt")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @r"
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
