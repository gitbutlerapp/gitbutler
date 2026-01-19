use bstr::ByteSlice;
use snapbox::str;

use crate::utils::CommandExt;
use crate::{command::util::commit_file_with_worktree_changes_as_two_hunks, utils::Sandbox};

#[test]
fn uncommitted_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("--json status -f")
        .allow_json()
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

Absorbed to commit: f4ea7f8 a.txt
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
        .allow_json()
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

Absorbed to commit: f4ea7f8 a.txt
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
        .allow_json()
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

#[test]
fn committed_hunk() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Verify that the first hunk is j0, and commit it.
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

    env.but("commit A -m 'partial change to a.txt 1'")
        .assert()
        .success();

    let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;

    // Change the file at the top & commit
    env.file(
        "a.txt",
        format!("first\n{}lasta\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
   1  │-firsta
     1│+first
   2 2│ line
   3 3│ line
   4 4│ line

"#]]);

    env.but("commit A -m 'partial change to a.txt 2'")
        .assert()
        .success();

    // Change the file at the bottom & commit
    env.file(
        "a.txt",
        format!("first\n{}last\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-lasta
       9│+last

"#]]);

    env.but("commit A -m 'partial change to a.txt 3'")
        .assert()
        .success();

    // Change the file at the top & bottom & absorb
    env.file(
        "a.txt",
        format!("first new\n{}last new\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────╮
j0 a.txt│
────────╯
   1  │-first
     1│+first new
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
       9│+last new

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unstaged changes] 
┊   i0 M a.txt 🔒 889385c, a7aa4ef, f4ea7f8
┊
┊╭┄g0 [A]  
┊●   a7aa4ef partial change to a.txt 3  
┊│     q0 M a.txt
┊●   889385c partial change to a.txt 2  
┊│     n0 M a.txt
┊●   8dc39e0 partial change to a.txt 1  
┊│     o0 M a.txt
┊●   f4ea7f8 a.txt  
┊│     s0 A a.txt
┊●   9477ae7 add A  
┊│     p0 A A
├╯
┊
┊╭┄h0 [B]  
┊●   d3e2ba3 add B  
┊│     r0 A B
├╯
┊
┴ 0dc3733 (common base) [origin/main] 2000-01-02 add M 

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("absorb i0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 2 changed files to absorb:

Absorbed to commit: 889385c partial change to a.txt 2
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4

Absorbed to commit: a7aa4ef partial change to a.txt 3
  (files locked to commit due to hunk range overlap)
    a.txt @6,4 +6,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   4822140 partial change to a.txt 3  
┊│     l0 M a.txt
┊●   4593422 partial change to a.txt 2  
┊│     k0 M a.txt
┊●   8dc39e0 partial change to a.txt 1  
┊│     m0 M a.txt
┊●   f4ea7f8 a.txt  
┊│     p0 A a.txt
┊●   9477ae7 add A  
┊│     n0 A A
├╯
┊
┊╭┄h0 [B]  
┊●   d3e2ba3 add B  
┊│     o0 A B
├╯
┊
┴ 0dc3733 (common base) [origin/main] 2000-01-02 add M 

Hint: run `but help` for all commands

"#]]);

    // Change was full absorbed
    let repo = env.open_repo()?;
    let blob = repo.rev_parse_single(b"A:a.txt")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"
    first new
    line
    line
    line
    line
    line
    line
    line
    last new
    ");

    Ok(())
}
