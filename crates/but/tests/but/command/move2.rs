use crate::utils::Sandbox;

#[test]
fn move_commit_above_other_commit() {
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

    env.but("_move2 fe --above 9a")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved fe12bcd above 9ac4652

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   c6224e6 add first
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_below_other_commit() {
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

    env.but("_move2 9a --below fe")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652 below fe12bcd

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   c6224e6 add first
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_multiple_consecutive_commits_relative_to_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   5c88a8e add A13
┊●   a18ea48 add A12
┊●   0c0fcbf add A11
┊●   c472887 add A10
┊●   8188106 add A9
┊●   769f4a8 add A8
┊●   2a98cfc add A7
┊●   d60e311 add A6
┊●   c67c49e add A5
┊●   23c280d add A4
┊●   5c7c6d7 add A3
┊●   1299ac9 add A2
┊●   0748e42 add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    for (operator, target_commit) in [("--above", "c4"), ("--below", "0c")] {
        env.but("_move2 2a 76")
            .arg(operator)
            .arg(target_commit)
            .assert()
            .success()
            .stdout_eq(snapbox::str![["
Moved 2a98cfc, 769f4a8 [..]

"]]);

        env.but("status")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   86218a9 add A13
┊●   4ba5683 add A12
┊●   33c2cee add A11
┊●   894e57b add A8
┊●   1c425d1 add A7
┊●   73d652c add A10
┊●   a6a6cd1 add A9
┊●   d60e311 add A6
┊●   c67c49e add A5
┊●   23c280d add A4
┊●   5c7c6d7 add A3
┊●   1299ac9 add A2
┊●   0748e42 add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

        env.but("undo").assert().success();
    }
}

#[test]
fn move_multiple_non_consecutive_commits_in_arbitrary_order_relative_to_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   5c88a8e add A13
┊●   a18ea48 add A12
┊●   0c0fcbf add A11
┊●   c472887 add A10
┊●   8188106 add A9
┊●   769f4a8 add A8
┊●   2a98cfc add A7
┊●   d60e311 add A6
┊●   c67c49e add A5
┊●   23c280d add A4
┊●   5c7c6d7 add A3
┊●   1299ac9 add A2
┊●   0748e42 add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    for (operator, target_commit) in [("--above", "76"), ("--below", "81")] {
        // We pick the source commits in an "incorrect" order, but they should later be sorted correctly
        // via topological sort.
        //
        // Order as picked is: A7 A1 A5 --above A8, but we expect the commits to be applied from oldest
        // to newest, i.e. (A8) <- A1 <- A5 <- A7
        env.but("_move2 2a 07 c6")
            .arg(operator)
            .arg(target_commit)
            .assert()
            .success()
            .stdout_eq(snapbox::str![["
Moved 2a98cfc, 0748e42, c67c49e [..]

"]]);

        env.but("status")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   6f23074 add A13
┊●   ae30331 add A12
┊●   ad601eb add A11
┊●   bfcf0d6 add A10
┊●   a5511fb add A9
┊●   79d9420 add A7
┊●   651dbaf add A5
┊●   33e2190 add A1
┊●   ddfd694 add A8
┊●   88fbf4b add A6
┊●   4868a7b add A4
┊●   c05b7a8 add A3
┊●   b7e9e54 add A2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

        env.but("undo").assert().success();
    }
}

#[test]
fn cannot_combine_above_and_below() {
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

    env.but("_move2 9a --below fe --above fe")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--below <BRANCH_OR_COMMIT>' cannot be used with '--above <BRANCH_OR_COMMIT>'

Usage: but _move2 --below <BRANCH_OR_COMMIT> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn source_cannot_be_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("_move2 9a --above 9a")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input '9a' for '--above'

Source cannot also be target

Hint: Trying to move items above '9a'? Remove '9a' from '<SOURCES>' and try again!

"#]]);

    env.but("_move2 9a --below 9a")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input '9a' for '--below'

Source cannot also be target

Hint: Trying to move items below '9a'? Remove '9a' from '<SOURCES>' and try again!

"#]]);
}
