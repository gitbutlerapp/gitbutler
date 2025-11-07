use crate::utils::{Sandbox, setup_metadata};
use snapbox::str;

#[test]
fn branch_new_outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    env.but("branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
my-feature

"#]]);

    env.but("branch new --anchor 9477ae7 my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
my-anchored-feature

"#]]);
    Ok(())
}

#[test]
fn branch_new_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Test JSON output without anchor
    env.but("--json branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);

    // Test JSON output with anchor
    env.but("--json branch new --anchor 9477ae7 my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-anchored-feature",
  "anchor": "9477ae7"
}

"#]]);

    // Test JSON output when branch already exists
    env.but("--json branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "error": "Branch 'my-feature' already exists"
}

"#]]);
    Ok(())
}
