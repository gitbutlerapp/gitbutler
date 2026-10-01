//! Smoke tests for the most common commands against a SHA-256 repository.

use snapbox::str;

use crate::utils::Sandbox;

fn sha256_one_stack() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("sha256-one-stack");
    env.setup_metadata(&["A"]);
    assert_eq!(
        env.open_repo().object_hash(),
        gix::hash::Kind::Sha256,
        "the fixture is a SHA-256 repository"
    );
    env
}

#[test]
fn status_shows_the_stack() {
    let env = sha256_one_stack();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywq add A
├╯
┊
┴ 7b2f7ac (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_creates_a_commit() {
    let env = sha256_one_stack();
    env.file("new.txt", "content\n");

    env.but("commit -b A -m 'add new.txt'").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   lxl add new.txt
┊●   ywq add A
├╯
┊
┴ 7b2f7ac (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse A").len(),
        gix::hash::Kind::Sha256.len_in_hex(),
        "the new commit has a SHA-256 id"
    );
}

#[test]
fn reword_rewrites_a_commit() {
    let env = sha256_one_stack();

    env.but("reword ywq -m 'add A, reworded'")
        .assert()
        .success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywq add A, reworded
├╯
┊
┴ 7b2f7ac (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn undo_restores_the_state_before_a_commit() {
    let env = sha256_one_stack();
    let tip_before_commit = env.invoke_git("rev-parse A");
    env.file("new.txt", "content\n");
    env.but("commit -b A -m 'add new.txt'").assert().success();

    env.but("undo").assert().success();

    assert_eq!(
        env.invoke_git("rev-parse A"),
        tip_before_commit,
        "undo moves the branch back to where it was"
    );
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   sy A new.txt
┊
┊╭┄ g0 [A]
┊●   ywq add A
├╯
┊
┴ 7b2f7ac (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}
