use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn rejects_non_existent_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch delete no-such-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find branch: 'no-such-branch'

"#]])
        .stdout_eq(str![[]]);
}

#[test]
fn can_delete_branch_with_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A"]);

    env.but("branch delete A")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Deleted branch A

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [B]
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_delete_branch_with_commits_in_the_bottom_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete A")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Deleted branch A

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [C]
┊●   ec33a86 add C
┊│
┊├┄h0 [B]
┊●   05d3df1 add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_delete_branch_with_commits_in_the_middle_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete B")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Deleted branch B

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [C]
┊●   983f317 add C
┊│
┊├┄h0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_delete_branch_with_commits_in_the_top_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete C")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Deleted branch C

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [B]
┊●   582f37b add B
┊│
┊├┄h0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_delete_branches_via_short_code() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete g0")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Deleted branch C

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [B]
┊●   582f37b add B
┊│
┊├┄h0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
