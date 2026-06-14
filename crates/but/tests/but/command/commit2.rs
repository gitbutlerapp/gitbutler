use crate::utils::Sandbox;

#[test]
fn no_message_nothing_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.but("commit2 --no-message").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 --no-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 7bbfdca on 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 --no-message --format shell")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
7bbfdca

"#]]);
}

#[test]
fn no_args_single_head_no_message_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 --no-message --format json")
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    // TODO: move this into Sandbox
    env.file("editor.sh", "printf 'commit from editor\\n' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 -m 'add file.txt'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 -m 'add file.txt' -m 'with more' -m 'text lines'")
        .assert()
        .success();

    env.but("status -v")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("editor.sh", "printf '' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("editor.sh", "false");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit2")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![[r#"
"#]])
        .stderr_eq(snapbox::str![[r#"
Error: Editor exited with non-zero status

"#]]);
}

#[test]
fn create_commit_on_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 --no-message").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.file("first", "Some text");

    env.but("commit2 -m 'add first' -b file").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄fi [file]
┊●   5a6fc56 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("second", "change file");

    env.but("commit2 -m 'add second' -b file")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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

    env.but("commit2 -m 'add third' -b other")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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

    env.but("commit2 -m 'add fourth' -b other")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "Some text");

    env.but("commit2 -m 'add file.txt' -b").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.invoke_git("branch existing");

    env.file("first", "Some text");

    env.but("commit2 -m 'add first' -b existing")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'existing' exists but is not applied

Hint: Run `but apply existing` to apply the branch first

"#]]);
}

#[test]
fn bails_on_rejected_specs() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.file("first", "Some text");

    env.but("commit2 -m 'add first' -b foo").assert().success();

    env.file("first", "changes");

    env.but("commit2 -m 'add first' -b bar")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Couldn't commit all changes

"#]]);
}

#[test]
fn newly_created_branches_are_included_in_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks").unwrap();
    env.setup_metadata(&[]).unwrap();

    env.file("first", "Some text");

    env.but("commit2 -m 'add first' -b foo --format json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commit": "5a6fc56305c69edc974a5ed2d100c525db8fd288",
  "branch": "foo"
}

"#]]);
}
