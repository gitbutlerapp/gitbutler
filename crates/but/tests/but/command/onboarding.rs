use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn first_run_shows_metrics_message() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // The sandbox sets onboarding_complete: true by default to avoid polluting other tests.
    // Delete the settings file to simulate a fresh install (will be recreated with defaults).
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path)?;

    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.

"#]]);

    Ok(())
}

#[test]
fn second_run_is_silent() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // The sandbox sets onboarding_complete: true by default.
    // Delete the settings file to simulate a fresh install.
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path)?;

    // First run shows the message and sets onboarding_complete
    env.but("onboarding").assert().success().stdout_eq(snapbox::str![[r#"
GitButler uses metrics to help us know what is useful and improve it. Configure with `but config metrics`.

"#]]);

    // Second run should be completely silent (onboarding_complete is now true)
    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn json_mode_is_silent_but_marks_complete() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // The sandbox sets onboarding_complete: true by default.
    // Delete the settings file to simulate a fresh install.
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    std::fs::remove_file(&settings_path)?;

    // JSON mode should produce no output even on first run
    env.but("--json onboarding")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);

    // But it should still mark onboarding as complete, so human mode is also silent now
    env.but("onboarding")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![]);

    Ok(())
}
