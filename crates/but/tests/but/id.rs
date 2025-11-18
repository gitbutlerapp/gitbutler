use crate::utils::{Sandbox, setup_metadata};

#[test]
fn multiple_zeroes_as_unassigned_area() -> anyhow::Result<()> {
    let env = Sandbox::open_scenario_with_target_and_default_settings("two-stacks")?;

    // Must set metadata to match the scenario
    setup_metadata(&env, &["A", "B"])?;

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
    setup_metadata(&env, &["A", "B"])?;

    env.but("branch new branch001").assert().success();

    // Ensure that the ID of the unassigned area has enough 0s to be unambiguous.
    env.but("status")
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::str!["[..]000[..]Unassigned Changes[..]\n..."]);

    Ok(())
}
