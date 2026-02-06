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
    env.but("apply").arg(branch_name).assert().success();

    insta::assert_snapshot!(env.git_log()?, @r"
    *   9d5d9e5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f9d5a6 (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    // Now unapply the branch using the new `but unapply` command
    env.but("unapply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with branches 'feature-branch' from workspace

"#]]);

    // Verify the branch is removed from workspace
    insta::assert_snapshot!(env.git_log()?, @r"
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
    env.but("apply").arg(branch_name).assert().success();

    // Unapply with JSON output using the new `but unapply` command
    env.but("--json unapply")
        .arg(branch_name)
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
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
    env.but("apply").arg(branch_name).assert().success();

    env.but("unapply").arg(branch_name).assert().success();

    // Unapplying again should fail because the branch is not in any applied stack
    env.but("unapply")
        .arg(branch_name)
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unapply branch. Branch 'feature-branch' not found in any applied stack

"#]])
        .stdout_eq(str![]);

    Ok(())
}

#[test]
fn unapply_shell_format() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Unapply with shell format using the new `but unapply` command
    // Shell format outputs one branch name per line
    env.but("-f shell unapply")
        .arg(branch_name)
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
feature-branch

"#]]);

    Ok(())
}

#[test]
fn unapply_nonexistent_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to unapply a branch that doesn't exist - should fail with the new command
    env.but("unapply nonexistent-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unapply branch. Branch 'nonexistent-branch' not found in any applied stack

"#]])
        .stdout_eq(str![]);

    Ok(())
}

#[test]
fn unapply_nonexistent_branch_with_json() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to unapply a branch that doesn't exist with JSON output - should fail
    env.but("--json unapply nonexistent-branch")
        .allow_json()
        .assert()
        .failure();

    Ok(())
}

#[test]
fn unapply_branch_not_in_workspace() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Try to unapply a branch that exists but wasn't applied - should fail
    env.but("unapply")
        .arg(branch_name)
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to unapply branch. Branch 'feature-branch' not found in any applied stack

"#]])
        .stdout_eq(str![]);

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
    env.but("apply origin/remote-feature").assert().success();

    insta::assert_snapshot!(env.git_log()?, @r"
    *   1bb7daf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    // Unapply the remote branch by its local name (remote-feature, not origin/remote-feature)
    env.but("unapply remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with branches 'remote-feature' from workspace

"#]]);

    // Verify it was removed from workspace
    insta::assert_snapshot!(env.git_log()?, @r"
    * 4ee40a2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    | * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn unapply_using_cli_branch_id() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Get the CLI ID from status --json
    let status_output = env.but("status --json").allow_json().output()?;
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;

    // Find the branch's CLI ID - JSON uses camelCase and "branches" not "heads"
    let stacks = status["stacks"].as_array().expect("stacks should be an array");
    let mut cli_id = None;
    for stack in stacks {
        if let Some(branches) = stack["branches"].as_array() {
            for branch in branches {
                if branch["name"].as_str() == Some(branch_name) {
                    cli_id = Some(branch["cliId"].as_str().unwrap().to_string());
                    break;
                }
            }
        }
    }
    let cli_id = cli_id.expect("should find the branch CLI ID");

    // Unapply using the CLI ID
    env.but("unapply")
        .arg(&cli_id)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with branches 'feature-branch' from workspace

"#]]);

    // Verify the branch is no longer in workspace
    let status_output = env.but("status --json").allow_json().output()?;
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let stacks = status["stacks"].as_array().expect("stacks should be an array");

    // The feature-branch should no longer be in any stack
    for stack in stacks {
        if let Some(branches) = stack["branches"].as_array() {
            for branch in branches {
                assert_ne!(
                    branch["name"].as_str(),
                    Some(branch_name),
                    "branch should not be in workspace"
                );
            }
        }
    }

    Ok(())
}

#[test]
fn unapply_using_cli_stack_id() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Get the stack CLI ID from status --json
    let status_output = env.but("status --json").allow_json().output()?;
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;

    // Find the stack's CLI ID for the feature-branch - JSON uses camelCase and "branches" not "heads"
    let stacks = status["stacks"].as_array().expect("stacks should be an array");
    let mut stack_cli_id = None;
    for stack in stacks {
        if let Some(branches) = stack["branches"].as_array() {
            for branch in branches {
                if branch["name"].as_str() == Some(branch_name) {
                    stack_cli_id = Some(stack["cliId"].as_str().unwrap().to_string());
                    break;
                }
            }
        }
    }
    let stack_cli_id = stack_cli_id.expect("should find the stack CLI ID");

    // Unapply using the stack CLI ID
    env.but("unapply")
        .arg(&stack_cli_id)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with branches 'feature-branch' from workspace

"#]]);

    Ok(())
}

#[test]
fn unapply_json_output_validation() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Unapply with JSON output and validate structure
    let output = env.but("--json unapply").arg(branch_name).allow_json().output()?;

    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    // Validate JSON structure
    assert_eq!(json["unapplied"], serde_json::json!(true));
    let branches = json["branches"].as_array().expect("branches should be an array");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0], serde_json::json!("feature-branch"));

    Ok(())
}

mod utils {
    use crate::utils::Sandbox;

    pub fn create_local_branch_with_commit(env: &Sandbox, name: &str) {
        create_local_branch_with_commit_with_message(env, name, "Add feature")
    }

    pub fn create_local_branch_with_commit_with_message(env: &Sandbox, name: &str, commit_message: &str) {
        env.invoke_bash(format!(
            r#"
    git checkout main -b {name};
    git commit -m '{commit_message}' --allow-empty;
    git checkout gitbutler/workspace;
        "#
        ));
    }
}
