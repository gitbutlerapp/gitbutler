use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn tear_off_by_name() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    env.setup_metadata(&["A", "B"])?;
    assert_initial_status(&env);

    env.but("branch move --unstack C")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Unstacked branch 'C'.

"#]])
        .stderr_eq(str![""]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes]
┊     no changes
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
├╯
┊
┊╭┄i0 [C]
┊●   31e83cd add C
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn tear_off_by_cli_id() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    env.setup_metadata(&["A", "B"])?;
    assert_initial_status(&env);

    env.but("branch move --unstack h0")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Unstacked branch 'C'.

"#]])
        .stderr_eq(str![""]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes]
┊     no changes
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
├╯
┊
┊╭┄i0 [C]
┊●   31e83cd add C
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn tear_off_single_branch_stack_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;
    env.setup_metadata(&["A", "B"])?;
    assert_initial_status(&env);

    env.but("branch move --unstack A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Unstacked branch 'A'.

"#]])
        .stderr_eq(str![""]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes]
┊     no changes
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [C]
┊●   3842fc0 add C
┊│
┊├┄i0 [B]
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn move_requires_target_branch_without_unstack() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;

    env.but("branch move A").assert().failure();

    Ok(())
}

#[test]
fn move_unstack_conflicts_with_target_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    )?;

    env.but("branch move --unstack A B").assert().failure();

    Ok(())
}

fn assert_initial_status(env: &Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes]
┊     no changes
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [C]
┊●   3842fc0 add C
┊│
┊├┄i0 [B]
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 [origin/main] 2000-01-02 add M

Hint: run `but help` for all commands

"#]])
        .stderr_eq(str![""]);
}
