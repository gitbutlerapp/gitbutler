use crate::utils::Sandbox;

#[test]
fn split_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "contents of one");
    env.file("two", "contents of two");

    env.but("commit -m original").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 original
┊│     1:k A one
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split 1:k")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from 1 to new commit 1 above commit 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 (no commit message)
┊│     1#0:k A one
┊●   1#1 original
┊│     1#1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn cannot_split_sources_from_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "contents of one");
    env.file("two", "contents of two");

    env.but("commit -m original one").assert().success();
    env.but("commit -m original two").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 original
┊│     1#0:t A two
┊●   1#1 original
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("split 1#0:t 1#1:k")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Can only split files from one commit. Got 1 and 1

"#]]);
}
