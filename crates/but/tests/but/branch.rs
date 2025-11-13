use snapbox::str;

use crate::utils::{Sandbox, setup_metadata};

#[test]
fn new_outputs_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
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
fn new_with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
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
    env.but("branch new --json --anchor 9477ae7 my-anchored-feature")
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
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "my-feature"
}

"#]]);
    env.but("branch new --json --anchor 9477ae7 my-anchored-feature")
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

#[test]
fn apply_local_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a local branch
    env.invoke_git("checkout main -b feature-branch");
    env.file("feature.txt", "feature content");
    env.invoke_git("add feature.txt");
    env.invoke_git("commit -m 'Add feature'");
    env.invoke_git("checkout gitbutler/workspace");

    // Apply the local branch
    env.but("branch apply feature-branch")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied branch 'feature-branch' to workspace

"#]]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *   2668088 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 2bd8abe (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn apply_local_branch_with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a local branch
    env.invoke_git("checkout main -b feature-branch");
    env.file("feature.txt", "feature content");
    env.invoke_git("add feature.txt");
    env.invoke_git("commit -m 'Add feature'");
    env.invoke_git("checkout gitbutler/workspace");

    // Apply with JSON output
    env.but("--json branch apply feature-branch")
        .assert()
        .success()
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *   2668088 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 2bd8abe (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn apply_remote_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a remote branch reference
    env.invoke_git("checkout origin/main");
    env.file("remote-feature.txt", "remote feature content");
    env.invoke_git("add remote-feature.txt");
    env.invoke_git("commit -m 'Add remote feature'");
    env.invoke_git("update-ref refs/remotes/origin/remote-feature HEAD");
    env.invoke_git("checkout gitbutler/workspace");

    // Apply the remote branch
    env.but("branch apply remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied remote branch 'remote-feature' to workspace

"#]]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *   dae304e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * ded8a9b (origin/remote-feature, remote-feature) Add remote feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn apply_nonexistent_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to apply a branch that doesn't exist
    env.but("branch apply nonexistent-branch")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Could not find branch 'nonexistent-branch' in local repository

"#]]);

    Ok(())
}

#[test]
fn apply_nonexistent_branch_with_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to apply a branch that doesn't exist with JSON output
    env.but("--json branch apply nonexistent-branch")
        .assert()
        .success()
        .stderr_eq(str![]);
    // Note: Currently the apply function doesn't output anything with JSON when branch not found
    // This might be improved to output an error in JSON format

    Ok(())
}

#[test]
fn apply_branch_idempotent() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a local branch
    env.invoke_git("checkout main -b feature-branch");
    env.file("feature.txt", "feature content");
    env.invoke_git("add feature.txt");
    env.invoke_git("commit -m 'Add feature'");
    env.invoke_git("checkout gitbutler/workspace");

    // Apply the branch first time
    env.but("branch apply feature-branch")
        .assert()
        .success()
        .stderr_eq(str![]);

    // Apply the same branch again - should be idempotent
    env.but("branch apply feature-branch")
        .assert()
        .success()
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *   2668088 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 2bd8abe (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn apply_multiple_branches_sequentially() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create first branch from main
    env.invoke_git("checkout main");
    env.invoke_git("checkout -b feature-1");
    env.file("feature1.txt", "feature 1 content");
    env.invoke_git("add feature1.txt");
    env.invoke_git("commit -m 'Add feature 1'");

    // Create second branch from main
    env.invoke_git("checkout main");
    env.invoke_git("checkout -b feature-2");
    env.file("feature2.txt", "feature 2 content");
    env.invoke_git("add feature2.txt");
    env.invoke_git("commit -m 'Add feature 2'");

    env.invoke_git("checkout gitbutler/workspace");

    // Apply both branches
    env.but("branch apply feature-1")
        .assert()
        .success()
        .stderr_eq(str![]);

    env.but("branch apply feature-2")
        .assert()
        .success()
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *-.   35fd560 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * b4c520d (feature-2) Add feature 2
    | * | c6549e1 (feature-1) Add feature 1
    | |/  
    * / 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");
    Ok(())
}
