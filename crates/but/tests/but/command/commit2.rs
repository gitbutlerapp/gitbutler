use crate::utils::Sandbox;

#[test]
fn no_message_nothing_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_commit2 --no-message").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1049574 (no commit message) (no changes)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn no_args_single_head_no_message_human_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 --no-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 7bbfdca on branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   7bbfdca (no commit message)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn no_args_single_head_no_message_shell_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 --no-message --format shell")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
7bbfdca

"#]]);
}

#[test]
fn no_args_single_head_no_message_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 --no-message --format json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commit": "7bbfdca68284535242b93595db5f6a5bc885a124"
}

"#]]);
}

#[test]
fn no_args_single_head_message_from_editor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // TODO: move this into Sandbox
    env.file("editor.sh", "printf 'commit from editor\\n' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("_commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   d4e7c2a commit from editor
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn single_head_with_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 -m 'add file.txt'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   a41148c add file.txt
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_repeat_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 -m 'add file.txt' -m 'with more' -m 'text lines'")
        .assert()
        .success();

    env.but("status -v")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊● b141567 author 2000-01-01 00:00:00 +0000
┊│     add file.txt  with more  text lines
┊● 9477ae7 author 2000-01-01 00:00:00 +0000
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("show b141567")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Commit:    b14156794f81a138bd06c2a5287fd5db15408b56
Change-ID: 1
Author:    author <author@example.com>
Date:      2000-01-02 00:00:00 +0000 (26y ago)
Committer: committer <committer@example.com>

add file.txt

with more

text lines

Files changed:
  A file.txt

"#]]);
}

#[test]
fn editor_user_writes_no_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("editor.sh", "printf '' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("_commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   3b915b5 (no commit message)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn editor_fails() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("editor.sh", "false");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("_commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::str![[r#"
Error: Editor exited with non-zero status

"#]]);
}

#[test]
fn create_commit_on_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file.txt", "Some text");

    env.but("_commit2 --no-message").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   d4910f8 (no commit message)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_user_provided_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("_commit2 -m 'add first' -b file")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄fi [file]
┊●   5a6fc56 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("second", "change file");

    env.but("_commit2 -m 'add second' -b file")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄fi [file]
┊●   49fc2f0 add second
┊●   5a6fc56 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("third", "change file");

    env.but("_commit2 -m 'add third' -b other")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄fi [file]
┊●   49fc2f0 add second
┊●   5a6fc56 add first
├╯
┊
┊╭┄ot [other]
┊●   ed433d3 add third
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("fourth", "change file");

    env.but("_commit2 -m 'add fourth' -b other")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄fi [file]
┊●   49fc2f0 add second
┊●   5a6fc56 add first
├╯
┊
┊╭┄ot [other]
┊●   81bd527 add fourth
┊●   ed433d3 add third
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_new_branch_with_canned_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("_commit2 -m 'add file.txt' -b").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄br [a-branch-1]
┊●   633b765 add file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_branch_that_is_not_applied_fails() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.invoke_git("branch existing");

    env.file("first", "Some text");

    env.but("_commit2 -m 'add first' -b existing")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'existing' exists but is not applied

Hint: Run `but apply existing` to apply the branch first

"#]]);
}

#[test]
fn bails_on_rejected_specs() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("_commit2 -m 'add first' -b foo").assert().success();

    env.file("first", "changes");

    env.but("_commit2 -m 'add first' -b bar")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Couldn't commit all changes

"#]]);
}

#[test]
fn newly_created_branches_are_included_in_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("_commit2 -m 'add first' -b foo --format json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commit": "5a6fc56305c69edc974a5ed2d100c525db8fd288",
  "branch": "foo"
}

"#]]);
}

#[test]
fn empty_flag_to_force_empty_commit_when_changes_exist() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.file(
        "changes",
        "Some changes that will not be included in commit",
    );

    env.but("_commit2 -m 'empty commit despite changes in worktree' --empty")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   vq A changes
┊
┊╭┄br [a-branch-1]
┊●   341ce70 empty commit despite changes in worktree (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);
}

#[test]
fn commit_empty_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_commit2 --no-message --above fe")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   955bee4 add second
┊●   4400448 (no commit message) (no changes)
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_empty_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_commit2 --no-message --below fe")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   eee1eaf add second
┊●   c149530 add first
┊●   8b16ff4 (no commit message) (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 -m 'add file.txt' --above fe")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   e38b8f7 add second
┊●   94fecae add file.txt
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 -m 'add file.txt' --above g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   092d7a9 add file.txt
┊│
┊├┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 -m 'add file.txt' --below fe")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   d7e6d8f add second
┊●   12982b7 add first
┊●   bdfeaa4 add file.txt
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 -m 'add file.txt' --below g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   8a3fbb3 add A
┊│
┊├┄br [a-branch-1]
┊●   28e38bd add file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_branch_with_multiple_commits_treats_branch_as_bucket() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 -m 'add file.txt' --below g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   d7e6d8f add second
┊●   12982b7 add first
┊│
┊├┄br [a-branch-1]
┊●   bdfeaa4 add file.txt
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_refuses_on_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.file("second", "Conflicting with commit 9ac4652");

    env.but("_commit2 -m 'add second' --above fe")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Couldn't commit all changes

"#]]);
}

#[test]
fn commit_below_refuses_on_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.file("second", "Conflicting with commit 9ac4652");

    env.but("_commit2 -m 'add second' --below 9a")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Couldn't commit all changes

"#]]);
}

#[test]
fn refuses_above_and_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("_commit2 --above dontcare --below dontcare")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--above <BRANCH_OR_COMMIT>' cannot be used with '--below <BRANCH_OR_COMMIT>'

Usage: but _commit2 --above <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

"#]]);
}

#[test]
fn refuses_above_and_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("_commit2 --above dontcare -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--above <BRANCH_OR_COMMIT>' cannot be used with '--branch [<BRANCH>]'

Usage: but _commit2 --above <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

"#]]);
}

#[test]
fn refuses_below_and_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("_commit2 --below dontcare -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--below <BRANCH_OR_COMMIT>' cannot be used with '--branch [<BRANCH>]'

Usage: but _commit2 --below <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

"#]]);
}

#[test]
fn above_branch_not_in_workspace_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("_commit2 --above B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'B'

Hint: Target must be an applied branch or commit. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn above_commit_not_in_workspace_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply B").assert().success();

    env.but("_commit2 --above d3")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'd3'

Hint: Target must be an applied branch or commit. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn above_non_branch_non_commit_target_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("_commit2 --above zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected a commit or a branch, got uncommitted changes

Hint: Run `but status` to show applicable targets

"#]]);
}

#[test]
fn committing_specific_cli_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "content");
    env.file("two", "content");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊     kl A one
┊   twop A two
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 --no-message kl").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   twop A two
┊
┊╭┄g0 [A]
┊●   f86bb7b (no commit message)
┊│     f8:kl A one
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);
}

#[test]
fn hunks_within_file_are_not_order_dependent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let original_data = "enough\nlines\nto\ncreate\nmultiple\nhunks\nwhen\nediting";

    env.file("file", original_data);

    env.but("_commit2 --no-message").assert().success();

    env.file("file", format!("first hunk\n{original_data}\nlast hunk"));

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────╮
i0 file│
───────╯
     1│+first hunk
   1 2│ enough
   2 3│ lines
   3 4│ to
───────╮
j0 file│
───────╯
    6  7│ hunks
    7  8│ when
    8  9│ editing
      10│+last hunk

"#]]);

    env.but("_commit2 --no-message i0 j0").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   f0a3edc (no commit message)
┊│     f0:qs M file
┊●   21b345e (no commit message)
┊│     21:qs A file
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("_commit2 --no-message j0 i0").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   f0a3edc (no commit message)
┊│     f0:qs M file
┊●   21b345e (no commit message)
┊│     21:qs A file
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn overlapping_changes_to_modified_file_are_deduplicated() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let original_data = "enough\nlines\nto\ncreate\nmultiple\nhunks\nwhen\nediting";

    env.file("file", original_data);

    env.but("_commit2 --no-message").assert().success();

    env.file("file", format!("first hunk\n{original_data}\nlast hunk"));

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────╮
i0 file│
───────╯
     1│+first hunk
   1 2│ enough
   2 3│ lines
   3 4│ to
───────╮
j0 file│
───────╯
    6  7│ hunks
    7  8│ when
    8  9│ editing
      10│+last hunk

"#]]);

    env.but("_commit2 --no-message i0 j0 i0").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   f0a3edc (no commit message)
┊│     f0:qs M file
┊●   21b345e (no commit message)
┊│     21:qs A file
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("_commit2 --no-message file j0").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   f0a3edc (no commit message)
┊│     f0:qs M file
┊●   21b345e (no commit message)
┊│     21:qs A file
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_something_that_isnt_a_cli_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_commit2 --no-message A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find uncommitted change: 'A'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn can_commit_with_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("path/to/first.txt", "first");
    env.file("path/to/second.txt", "second");
    env.file("path/other/to/third.txt", "third");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   om A path/other/to/third.txt
┊   ms A path/to/first.txt
┊   rr A path/to/second.txt
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 path/to/ --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   om A path/other/to/third.txt
┊
┊╭┄g0 [A]
┊●   e1c5473 (no commit message)
┊│     e1:ms A path/to/first.txt
┊│     e1:rr A path/to/second.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);
}

#[test]
fn path_prefix_with_mix_of_modifications() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("dir/to_modify.txt", "first");
    env.file("dir/to_delete.txt", "second");
    env.file("dir/to_empty.txt", "third");

    env.but("_commit2 --no-message").assert().success();

    std::fs::remove_file(env.projects_root().join("dir/to_delete.txt")).unwrap();
    env.file("dir/to_empty.txt", "");
    env.file(
        env.projects_root().join("dir/to_modify.txt"),
        "first\nnew line",
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   lm D dir/to_delete.txt
┊   no M dir/to_empty.txt
┊   xv M dir/to_modify.txt
┊
┊╭┄g0 [A]
┊●   d199c17 (no commit message)
┊│     d1:lm A dir/to_delete.txt
┊│     d1:no A dir/to_empty.txt
┊│     d1:xv A dir/to_modify.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 dir/ --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   d1a6de8 (no commit message)
┊│     d1a:lm D dir/to_delete.txt
┊│     d1a:no M dir/to_empty.txt
┊│     d1a:xv M dir/to_modify.txt
┊●   d199c17 (no commit message)
┊│     d19:lm A dir/to_delete.txt
┊│     d19:no A dir/to_empty.txt
┊│     d19:xv A dir/to_modify.txt
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("diff d1a")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────────╮
D dir/to_delete.txt│
───────────────────╯
   1  │-second
──────────────────╮
M dir/to_empty.txt│
──────────────────╯
   1  │-third
───────────────────╮
M dir/to_modify.txt│
───────────────────╯
   1 1│ first
     2│+new line

"#]]);
}

#[test]
fn requires_specifying_stack_when_there_are_multiple() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("_commit2 --empty --no-message")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Unclear where to commit. Found more than one stack

Hint: You can specify where to commit with `--branch [<BRANCH>]`

"#]]);
}

#[test]
fn committing_above_an_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    env.but("branch new top").assert().success();
    env.but("_commit2 one -m 'add one' --above top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   75b9f19 add one
┊│
┊├┄to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_empty_branch_with_empty_branch_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    env.but("branch new middle").assert().success();
    env.but("branch new --anchor middle top").assert().success();
    env.but("_commit2 one -m 'add one' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄to [top] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   75b9f19 add one
┊│
┊├┄mi [middle] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_non_top_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.file("two", "two content");

    env.but("_commit2 one -m 'add one' -b bottom")
        .assert()
        .success();
    env.but("branch new --anchor bottom middle")
        .assert()
        .success();
    env.but("branch new --anchor middle top").assert().success();
    env.but("_commit2 two -m 'add two' --below middle")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄op [top] (no commits)
┊│
┊├┄mi [middle] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   af4ddbe add two
┊│
┊├┄bo [bottom]
┊●   75b9f19 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_an_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.file("two", "two content");

    env.but("branch new top").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊     kl A one
┊   twop A two
┊
┊╭┄to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_commit2 one -m 'add one' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   twop A two
┊
┊╭┄to [top] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   75b9f19 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("reword a-branch-1 -m bottom").assert().success();

    env.but("_commit2 two -m 'add two' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄op [top] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   af4ddbe add two
┊│
┊├┄bo [bottom]
┊●   75b9f19 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn gives_good_error_when_your_terminal_doesnt_support_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("_commit2 --interactive")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Terminal doesn't support interactivity

"#]]);
}

#[test]
fn commit_to_existing_branch_via_short_code() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_commit2 -b g0 -m 'new commit'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   2e0e1d8 new commit (no changes)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_to_new_branch_with_same_name_as_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "");

    env.but("_commit2 -b file -m 'add file'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄fi [file]
┊●   46b0823 add file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
