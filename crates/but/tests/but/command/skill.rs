use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn skill_check_local_outside_repo_fails() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("skill check --local")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Cannot check local installations: not in a git repository.
Use --global to check global installations, or run from within a repository.

"#]]);
    Ok(())
}

#[test]
fn skill_check_json_output_is_valid() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Check with --global to avoid needing a repo context
    // The JSON output should always be valid even if no skills are found
    let output = env
        .but("skill check --global --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify it's valid JSON
    let json: serde_json::Value = serde_json::from_slice(&output)?;

    // Verify the expected structure
    assert!(json.get("cli_version").is_some(), "should have cli_version");
    assert!(json.get("skills").is_some(), "should have skills array");
    assert!(json.get("outdated_count").is_some(), "should have outdated_count");

    Ok(())
}
