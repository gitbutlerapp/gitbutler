use crate::utils::Sandbox;
use snapbox::str;

#[cfg(not(feature = "legacy"))]
#[test]
fn looks_good_and_can_be_invoked_in_various_ways_non_legacy() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.but(None).assert().success().stdout_eq(snapbox::file![
        "snapshots/help/no-arg-no-legacy.stdout.term.svg"
    ]);
    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn looks_good_and_can_be_invoked_in_various_ways() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.but(None)
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help/no-arg.stdout.term.svg"]);

    env.but("-h")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help/no-arg.stdout.term.svg"]);

    env.but("--help")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/help/no-arg.stdout.term.svg"]);

    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn rub_looks_good() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // The help should be nice, as it's a complex command.
    env.but("rub --help")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/help/rub-long-help.stdout.term.svg"
        ]);
    env.but("rub -h")
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/help/rub-short-help.stdout.term.svg"
        ]);
    Ok(())
}

#[test]
fn nonexistent_path_shows_friendly_error() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Test that running `but <nonexistent-path>` shows a friendly error message
    // instead of a low-level filesystem error
    env.but("test")
        .assert()
        .failure()
        .stdout_eq("")
        .stderr_eq(str![[r#"
Error: "but test" is not a command. Type "but --help" to see all available commands.

"#]]);

    env.but("nonexistent-dir")
        .assert()
        .failure()
        .stdout_eq("")
        .stderr_eq(str![[r#"
Error: "but nonexistent-dir" is not a command. Type "but --help" to see all available commands.

"#]]);

    Ok(())
}
