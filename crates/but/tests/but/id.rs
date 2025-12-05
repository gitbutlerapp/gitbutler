// TODO: turn these into integration-level tests of the `IdMap` type directly, don't invoke it indirectly.
use crate::utils::Sandbox;

#[test]
fn multiple_zeroes_as_unassigned_area() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    // Check that 000 is interpreted as the unassigned area.
    env.but("rub 000 000")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![
            "Rubbed the wrong way. Operation doesn't make sense. \
Source[..]the unassigned area[..]target[..]the unassigned area[..]"
        ]);
    Ok(())
}

#[test]
fn unassigned_area_id_is_unambiguous() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    env.but("branch new branch001").assert().success();

    // Ensure that the ID of the unassigned area has enough 0s to be unambiguous.
    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::str!["[..]000[..]Unassigned Changes[..]\n..."]);

    Ok(())
}

#[test]
fn branch_avoid_nonalphanumeric() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    env.but("branch new x-yz").assert().success();

    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::str![
            "...
[..]yz[..]x-yz[..]
..."
        ]);

    Ok(())
}

#[test]
fn branch_avoid_hexdigit() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    env.but("branch new 0ax").assert().success();

    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::str![
            "...
[..]ax[..]0ax[..]
..."
        ]);

    Ok(())
}

#[test]
fn branch_cannot_generate_id() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"])?;

    // Exercise the case where we cannot generate an ID for a branch (any ID we
    // generate would also match supersubstring).
    env.but("branch new substring").assert().success();
    env.but("branch new supersubstring").assert().success();

    // The ID of the substring is a hash and cannot be asserted, so only
    // assert the supersubstring. It is "up" because "su" would conflict with
    // "substring".
    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::str![
            "...
[..]tp[..]substring[..]
...
[..]up[..]supersubstring[..]
..."
        ]);

    Ok(())
}
