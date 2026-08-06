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

Hint: Run `but status` for applicable targets.

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
Discarded branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   lrm add B
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
Discarded branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
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
Discarded branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [A]
┊●   tpm add A
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
Discarded branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
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
Discarded branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
