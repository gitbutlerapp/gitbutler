use crate::utils::Sandbox;

#[test]
fn json_flag_can_be_placed_before_or_after_subcommand() -> anyhow::Result<()> {
    // TODO: use an actual repository here, but single-branch mode isn't really supported yet
    //       so everything fails anyway.
    let env = Sandbox::empty()?;

    // Test that --json flag works in both positions with help command (doesn't need a valid repo)
    env.but("--json completions --help").assert().success();

    env.but("completions --help --json").assert().success();

    #[cfg(feature = "legacy")]
    {
        use snapbox::str;

        use crate::utils::CommandExt;
        // Test with actual commands that need a repo (they'll fail but should accept the flag)
        // Before subcommand
        env.but("--json status")
            .allow_json()
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: No git repository found at .
Please run 'but setup' to initialize the project.

"#]]);

        // After subcommand
        env.but("status --json")
            .allow_json()
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: No git repository found at .
Please run 'but setup' to initialize the project.

"#]]);
    }

    Ok(())
}

#[test]
#[cfg(feature = "legacy")]
fn default_command_respects_c_flag_for_setup_checks() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.invoke_bash("git init repo");

    env.but("-C repo")
        .assert()
        .stderr_eq(snapbox::str![[r#"
Error: Setup required: No GitButler project found at repo

"#]])
        .failure();

    Ok(())
}

#[test]
#[cfg(feature = "legacy")]
fn default_alias_can_provide_c_flag_for_implicit_default_command() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.invoke_bash(
        r#"
git init .
git init repo
git config but.alias.default "-C repo status"
"#,
    );

    env.but("").assert().failure().stderr_eq(snapbox::str![[r#"
Error: Setup required: No GitButler project found at repo

"#]]);

    Ok(())
}

#[test]
#[cfg(feature = "legacy")]
fn explicit_c_flag_overrides_default_alias_c_flag() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.invoke_bash(
        r#"
git init .
git init repo
git init repo2
git config but.alias.default "-C repo status"
"#,
    );

    env.but("-C repo2").assert().failure().stderr_eq(snapbox::str![[r#"
Error: Setup required: No GitButler project found at repo2

"#]]);

    Ok(())
}
