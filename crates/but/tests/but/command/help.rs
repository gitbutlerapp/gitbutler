use snapbox::str;

use crate::utils::Sandbox;

#[cfg(feature = "legacy")]
#[test]
fn rub_looks_good() -> anyhow::Result<()> {
    use crate::utils::CommandExt;
    let env = Sandbox::empty()?;

    // The help should be nice, as it's a complex command.
    env.but("rub --help")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/help/rub-long-help.stdout.term.svg"
        ]);
    env.but("rub -h")
        .with_color_for_svg()
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

    env.but("nonexistent-directory-entry")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: "but nonexistent-directory-entry" is not a command. Type "but --help" to see all available commands.

"#]]);

    Ok(())
}
