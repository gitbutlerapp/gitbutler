use crate::utils::Sandbox;

// TODO: make fixture for this
fn one_branch_three_commits() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    env.but("commit2 -m 'add one' one").assert().success();
    env.but("commit2 -m 'add two' two").assert().success();
    env.but("commit2 -m 'add three' three").assert().success();

    env
}

#[test]
fn squash_two_commits() {
    let env = one_branch_three_commits();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   f55169f add three
┊│     f5:or A three
┊●   f63361f add two
┊│     f6:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // TODO(david): handle --message

    env.but("squash2 f55169f --target f63361f --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed f55169f into f63361f to create 7251301

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   7251301 squashed
┊│     72:or A three
┊│     72:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("squash2 f55169f --target f63361f --message 'squashed' --format json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "new_commit": "725130139e9f0178e29afbe9eff6a988afbca3fa"
}

"#]]);

    env.but("undo").assert().success();

    env.but("squash2 f55169f --target f63361f --message 'squashed' --format shell")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
725130139e9f0178e29afbe9eff6a988afbca3fa

"#]]);
}

#[test]
fn squash_multiple_sources() {
    let env = one_branch_three_commits();

    env.but("squash2 f55169f f63361f --target ea345ba --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed f55169f, f63361f into ea345ba to create e355a10

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e355a10 squashed
┊│     e3:kl A one
┊│     e3:or A three
┊│     e3:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_target_message() {
    let env = one_branch_three_commits();

    env.but("squash2 f55169f --target f63361f --use-target-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 5ab5165 author 2000-01-01 00:00:00 +0000
┊│     add two
┊│     5a:or A three
┊│     5a:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_source_message() {
    let env = one_branch_three_commits();

    env.but("squash2 f55169f --target f63361f --use-source-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊● c441d34 author 2000-01-01 00:00:00 +0000
┊│     add three
┊│     c4:or A three
┊│     c4:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch() {
    let env = one_branch_three_commits();

    env.but("squash2 a-branch-1 -m 'squashed a branch'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![["
Squashed branch 'a-branch-1' to create commit a694042

"]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊● a694042 author 2000-01-01 00:00:00 +0000
┊│     squashed a branch
┊│     a6:kl A one
┊│     a6:or A three
┊│     a6:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_reword_with_editor() {
    let env = one_branch_three_commits();

    env.file(
        ".git/editor.sh",
        "printf 'message from editor\\n' > \"$1\"\n",
    );
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("squash2 a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![["
Squashed branch 'a-branch-1' to create commit 7b3d915

"]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 7b3d915 author 2000-01-01 00:00:00 +0000
┊│     message from editor
┊│     7b:kl A one
┊│     7b:or A three
┊│     7b:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_combine_messages_with_editor() {
    let env = one_branch_three_commits();

    env.file(".git/editor.sh", "true");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("squash2 a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' to create commit abb21d9

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄br [a-branch-1]
┊● abb21d9 author 2000-01-01 00:00:00 +0000
┊│     add one  add three  add two
┊│     ab:kl A one
┊│     ab:or A three
┊│     ab:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
