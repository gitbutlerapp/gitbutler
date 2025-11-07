use crate::utils::{Sandbox, setup_metadata};

#[test]
fn branch_new_outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a new branch and capture the output
    let output = env
        .but("branch new my-feature")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output)?;

    // The output should be just the branch name (with newline)
    assert_eq!(output_str.trim(), "my-feature");

    Ok(())
}

#[test]
fn branch_new_with_anchor() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("one-stack")?;
    setup_metadata(&env, &["A"])?;

    // Create a new branch with an anchor (using longer ID)
    let output = env
        .but("branch new my-feature --anchor 9477ae7")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output)?;

    // The output should be just the branch name (with newline)
    assert_eq!(output_str.trim(), "my-feature");

    Ok(())
}
