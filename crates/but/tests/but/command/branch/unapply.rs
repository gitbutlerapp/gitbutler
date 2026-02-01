use snapbox::str;
use utils::create_local_branch_with_commit;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn single_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // First apply the branch
    env.but("branch apply").arg(branch_name).assert().success();

    insta::assert_snapshot!(env.git_log()?, @r"
    *   9d5d9e5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f9d5a6 (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    // Now unapply the branch
    env.but("branch unapply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with branches 'feature-branch' from workspace

"#]]);

    // Verify the branch is removed from workspace
    insta::assert_snapshot!(env.git_log()?, @"
    * 9f9d5a6 (feature-branch) Add feature
    | * 4ee40a2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn unapply_with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Apply the branch first
    env.but("branch apply").arg(branch_name).assert().success();

    // Unapply with JSON output
    env.but("--json branch unapply")
        .arg(branch_name)
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @"
    * 9f9d5a6 (feature-branch) Add feature
    | * 4ee40a2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn unapply_idempotent() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Apply then unapply
    env.but("branch apply").arg(branch_name).assert().success();

    env.but("branch unapply")
        .arg(branch_name)
        .assert()
        .success();

    // Unapplying again should be idempotent (success, no-op)
    env.but("branch unapply")
        .arg(branch_name)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Branch 'feature-branch' not found in any stack

"#]])
        .stderr_eq(str![]);

    Ok(())
}

#[test]
fn unapply_shell_format() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("branch apply").arg(branch_name).assert().success();

    // Unapply with shell format
    env.but("-f shell branch unapply")
        .arg(branch_name)
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![""]);

    Ok(())
}

#[test]
fn unapply_nonexistent_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to unapply a branch that doesn't exist
    env.but("branch unapply nonexistent-branch")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Branch 'nonexistent-branch' not found in any stack

"#]]);

    Ok(())
}

#[test]
fn unapply_nonexistent_branch_with_json() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to unapply a branch that doesn't exist with JSON output
    env.but("--json branch unapply nonexistent-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(str![""]);

    Ok(())
}

#[test]
fn unapply_branch_not_in_workspace() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Try to unapply a branch that exists but wasn't applied
    env.but("branch unapply")
        .arg(branch_name)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Branch 'feature-branch' not found in any stack

"#]])
        .stderr_eq(str![]);

    Ok(())
}

#[test]
fn unapply_remote_tracking_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    env.setup_metadata(&["A"])?;

    // Create a remote branch reference
    env.invoke_bash(
        r#"
    git checkout origin/main
    git commit -m 'Add remote feature' --allow-empty
    git update-ref refs/remotes/origin/remote-feature HEAD
    git checkout gitbutler/workspace
"#,
    );

    // Apply the remote branch
    env.but("branch apply origin/remote-feature")
        .assert()
        .success();

    insta::assert_snapshot!(env.git_log()?, @r"
    *   1bb7daf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    // Unapply the remote branch
    env.but("branch unapply origin/remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Branch 'origin/remote-feature' not found in any stack

"#]]);

    // Verify it was removed from workspace
    insta::assert_snapshot!(env.git_log()?, @r"
    *   1bb7daf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

mod utils {
    use crate::utils::Sandbox;

    pub fn create_local_branch_with_commit(env: &Sandbox, name: &str) {
        create_local_branch_with_commit_with_message(env, name, "Add feature")
    }

    pub fn create_local_branch_with_commit_with_message(
        env: &Sandbox,
        name: &str,
        commit_message: &str,
    ) {
        env.invoke_bash(format!(
            r#"
    git checkout main -b {name};
    git commit -m '{commit_message}' --allow-empty;
    git checkout gitbutler/workspace;
        "#
        ));
    }
}
