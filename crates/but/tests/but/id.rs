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
