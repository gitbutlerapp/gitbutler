use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    env.setup_metadata(&["A"])?;

    env.but("branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch my-feature

"#]]);

    env.but("branch new --anchor 9477ae7 my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch my-anchored-feature

"#]]);
    Ok(())
}

#[test]
fn with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    env.setup_metadata(&["A"])?;

    // Test JSON output without anchor
    env.but("--json branch new my-feature")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);

    // Test JSON output with anchor
    env.but("branch new --json --anchor 9477ae7 my-anchored-feature")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-anchored-feature",
  "anchor": "9477ae7"
}

"#]]);

    // Test JSON output when branch already exists - it's idempotent.
    env.but("branch --json new my-feature")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);
    env.but("branch new --json --anchor 9477ae7 my-anchored-feature")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-anchored-feature",
  "anchor": "9477ae7"
}

"#]]);

    // TODO: on error
    // On error, we indicate this both by exit code and by json output to stdout
    // so tools would be able to detect it that way.
    Ok(())
}
