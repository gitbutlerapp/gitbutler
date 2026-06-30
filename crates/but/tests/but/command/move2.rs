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
Moved fe12bcd above commit 9ac4652

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
Moved 9ac4652 below commit fe12bcd

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
fn moving_commits_above_branch_creates_branch_above() {
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

    env.but("_move2 fe --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved fe12bcd to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   c6224e6 add first
┊│
┊├┄g0 [A]
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_above_branch_without_changing_relative_order_only_creates_branch() {
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

    env.but("_move2 9a --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652 to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   9ac4652 add second
┊│
┊├┄g0 [A]
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_below_branch_creates_branch_below() {
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

    env.but("_move2 9a --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652 to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   c6224e6 add first
┊│
┊├┄br [a-branch-1]
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_below_branch_without_changing_relative_order_only_creates_branch() {
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

    env.but("_move2 fe --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved fe12bcd to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│
┊├┄br [a-branch-1]
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_all_commits_above_branch_retains_branch() {
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

    env.but("_move2 9a fe --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652, fe12bcd to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   9ac4652 add second
┊●   fe12bcd add first
┊│
┊├┄g0 [A] (no commits)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_all_commits_below_branch_retains_branch() {
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

    env.but("_move2 9a fe --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652, fe12bcd to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   9ac4652 add second
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_above_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
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
┊╭┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 94 --above h0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9477ae7 to new branch 'a-branch-1' above branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄br [a-branch-1]
┊●   9477ae7 add A
┊│
┊├┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_below_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
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
┊╭┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 94 --below h0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9477ae7 to new branch 'a-branch-1' below branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄h0 [B] (no commits)
┊│
┊├┄br [a-branch-1]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn above_or_below_unapplied_or_non_existing_branch_errors() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
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
┊╭┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("unapply B").assert().success();

    env.but("_move2 94 --above B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'B'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("_move2 94 --below B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'B'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("_move2 94 --above no-such-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'no-such-branch'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("_move2 94 --below no-such-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'no-such-branch'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn move_to_tip_of_branch() {
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

    env.but("_move2 94 -b B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9477ae7 to the tip of branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄h0 [B]
┊●   22c3ce2 add A
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
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
┊╭┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 94 -b B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9477ae7 to the tip of branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄h0 [B]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_new_unstacked_branch() {
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

    env.but("_move2 9a --branch new-branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652 to new branch 'new-branch'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   fe12bcd add first
├╯
┊
┊╭┄ne [new-branch]
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_new_unstacked_branch_with_canned_name() {
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

    env.but("_move2 9a --branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 9ac4652 to new branch 'a-branch-1'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   fe12bcd add first
├╯
┊
┊╭┄br [a-branch-1]
┊●   ce8b324 add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_below_commit_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 9a:wu --below fe")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from 9ac4652 to new commit 8e35f84 below commit fe12bcd

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   01a55b8 add second (no changes)
┊●   12b9152 add first
┊│     12:lz A first
┊●   8e35f84 (no commit message)
┊│     8e:wu A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_above_commit_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 fe:lz --above 9a")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from fe12bcd to new commit c019027 above commit 9ac4652

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   c019027 (no commit message)
┊│     c0:lz A first
┊●   38b1f1a add second
┊│     38:wu A second
┊●   d8dfd0f add first (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_below_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 9a:wu --below A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from 9ac4652 to new commit 8e35f84 on new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   01a55b8 add second (no changes)
┊●   12b9152 add first
┊│     12:lz A first
┊│
┊├┄br [a-branch-1]
┊●   8e35f84 (no commit message)
┊│     8e:wu A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_above_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 fe:lz --above A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from fe12bcd to new commit c019027 on new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   c019027 (no commit message)
┊│     c0:lz A first
┊│
┊├┄g0 [A]
┊●   38b1f1a add second
┊│     38:wu A second
┊●   d8dfd0f add first (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_to_branch_tip_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 d3:pl --branch A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from d3e2ba3 to new commit be174de to the tip of branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   be174de (no commit message)
┊│     be:pl A B
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   5bbe27c add B (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_to_non_existing_branch_tip_creates_unstacked_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 9a:wu --branch new-branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from 9ac4652 to new commit 8e35f84 on new branch 'new-branch'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   810e515 add second (no changes)
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┊╭┄ne [new-branch]
┊●   8e35f84 (no commit message)
┊│     8e:wu A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_branch_without_argument_creates_unstacked_branch_with_canned_name_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 9a:wu --branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 changes from 9ac4652 to new commit 8e35f84 on new branch 'a-branch-1'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   810e515 add second (no changes)
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┊╭┄br [a-branch-1]
┊●   8e35f84 (no commit message)
┊│     8e:wu A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_should_be_order_independent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("new", "Some data");
    env.but("_commit2 -m 'Add new file'").assert().success();
    std::fs::rename(
        env.projects_root().join("new"),
        env.projects_root().join("moved"),
    )
    .unwrap();
    env.file("new/file", "Stuff");
    env.file("unrelated", "This should stay here :)");
    env.but("_commit2 -m 'Prepare for moves!'")
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e3d3e3a Prepare for moves!
┊│     e3:ul R moved
┊│     e3:py A new/file
┊│     e3:tt A unrelated
┊●   24ac1e5 Add new file
┊│     24:nx A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 e3:py e3:ul --above e3")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 2 changes from e3d3e3a to new commit 99ef17e above commit e3d3e3a

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   99ef17e (no commit message)
┊│     99:ul R moved
┊│     99:py A new/file
┊●   f94e59f Prepare for moves!
┊│     f9:tt A unrelated
┊●   24ac1e5 Add new file
┊│     24:nx A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("_move2 e3:ul e3:py --above e3")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 2 changes from e3d3e3a to new commit 99ef17e above commit e3d3e3a

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   99ef17e (no commit message)
┊│     99:ul R moved
┊│     99:py A new/file
┊●   f94e59f Prepare for moves!
┊│     f9:tt A unrelated
┊●   24ac1e5 Add new file
┊│     24:nx A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_from_multiple_source_commits_is_not_allowed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
┊│     d3:pl A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 94:tm d3:pl -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot move changes from multiple commits

Hint: Move changes from a single commit at first, then squash additional changes into the new commit

"#]]);
}

#[test]
fn mixing_commits_and_changes_is_not_allowed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│     9a:wu A second
┊●   fe12bcd add first
┊│     fe:lz A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 9a fe:lz -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Mixing source types is not allowed

Hint: You can only move one kind of source (e.g. commits) at a time

"#]]);
}

#[test]
fn targeting_unapplied_branch_errors() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
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
┊╭┄h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("unapply B").assert().success();

    env.but("_move2 94 --branch B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'B' exists but is not applied

"#]]);
}

#[test]
fn cannot_combine_targets() {
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

Usage: but _move2 <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn must_specify_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("_move2 dontcare")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]>

Usage: but _move2 <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn must_specify_source() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("_move2 -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <SOURCES>...

Usage: but _move2 <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]> <SOURCES>...

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

#[test]
fn cannot_move_from_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "some text");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   qs A file
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    env.but("_move2 qs -b A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'qs' for '<SOURCES>'

Cannot pass uncommitted file or hunk as source

Hint: Sources must be commits or committed files

"#]]);
    env.but("_move2 zz -b A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'zz' for '<SOURCES>'

Cannot pass uncommitted changes as source

Hint: Sources must be commits or committed files

"#]]);
}

#[test]
fn cannot_move_to_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&[]);

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
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_move2 94 --below zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected a commit or a branch, got uncommitted changes

"#]]);
}
