use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
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
✓ Created branch my-feature

"#]]);

    env.but("branch new --anchor 9477ae7 my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
✓ Created branch my-anchored-feature stacked on [..]

"#]]);
    Ok(())
}

#[test]
fn rejects_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_normalizes_to_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new HEAD-")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD-'

Invalid branch name: Could not turn "HEAD-" into a valid reference name

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_normalizes_to_something_else_and_suggests_alternative() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new 'my branch'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'my branch'

Invalid branch name

Hint: Try 'my-branch' instead

"#]]);

    Ok(())
}

#[test]
fn rejects_branch_name_already_applied_in_workspace() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' already exists

"#]]);

    Ok(())
}

#[test]
fn rejects_name_that_exists_outside_workspace() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.but("unapply A").assert().success();

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' already exists

"#]]);

    Ok(())
}

#[test]
fn with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Test JSON output without anchor
    env.but("--format json branch new my-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);

    // Test JSON output with anchor
    env.but("branch new --format json --anchor 9477ae7 my-anchored-feature")
        .allow_json()
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
