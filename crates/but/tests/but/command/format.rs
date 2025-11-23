use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn json_flag_can_be_placed_before_or_after_subcommand() -> anyhow::Result<()> {
    // TODO: use an actual repository here, but single-branch mode isn't really supported yet
    //       so everything fails anyway.
    let env = Sandbox::empty()?;

    // Test that --json flag works in both positions with help command (doesn't need a valid repo)
    env.but("--json completions --help").assert().success();

    env.but("completions --help --json").assert().success();

    // Test with actual commands that need a repo (they'll fail but should accept the flag)
    // Before subcommand
    env.but("--json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents[..]

"#]]);

    // After subcommand
    env.but("status --json")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents[..]

"#]]);

    Ok(())
}
