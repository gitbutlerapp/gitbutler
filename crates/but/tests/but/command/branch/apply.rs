use crate::utils::{Sandbox, setup_metadata};
use snapbox::str;

use crate::command::branch::apply::utils::create_local_branch_with_commit_with_message;
use utils::create_local_branch_with_commit;

#[test]
fn local_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    setup_metadata(&env, &["A"])?;

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Apply the local branch
    env.but("branch apply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied branch 'feature-branch' to workspace

"#]]);
    // It's idempotent and can produce a shell value.
    env.but("-f shell branch apply feature-branch")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
refs/heads/feature-branch

"#]]);

    // It actually applied the branch, by merging it in.
    insta::assert_snapshot!(env.git_log()?, @r"
    *   9d5d9e5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f9d5a6 (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn local_branch_with_json_output() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    setup_metadata(&env, &["A"])?;

    create_local_branch_with_commit(&env, "feature-branch");

    // Apply with JSON output
    env.but("--json branch apply feature-branch")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "name": {
    "full": "refs/heads/feature-branch"
  },
  "target_id": "9f9d5a694afe171f5f9c72f8cf06db6210c3cf43",
  "target_ref": null
}

"#]])
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *   9d5d9e5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f9d5a6 (feature-branch) Add feature
    * | 9477ae7 (A) add A
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn remote_branch_creates_local_tracking_branch_automatically() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    setup_metadata(&env, &["A"])?;

    // Create a remote branch reference
    env.invoke_bash(
        r#"
    git checkout origin/main
    git commit -m 'Add remote feature' --allow-empty
    git update-ref refs/remotes/origin/remote-feature HEAD
    git checkout gitbutler/workspace
"#,
    );

    // Apply the remote branch, by its shortest name only.
    env.but("branch apply origin/remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied remote branch 'origin/remote-feature' to workspace

"#]]);

    // It created a local tracking branch.
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

#[test]
fn nonexistent_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to apply a branch that doesn't exist
    env.but("branch apply nonexistent-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: The reference 'nonexistent-branch' did not exist

"#]])
        .stdout_eq(str![""]);

    Ok(())
}

#[test]
fn nonexistent_branch_with_json() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;

    // Try to apply a branch that doesn't exist with JSON output
    env.but("--json branch apply nonexistent-branch")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: The reference 'nonexistent-branch' did not exist

"#]]);
    // Note: Currently the apply function doesn't output anything with JSON when branch not found
    // This might be improved to output an error in JSON format

    Ok(())
}

#[test]
fn multiple_branches_sequentially() -> anyhow::Result<()> {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M
");

    setup_metadata(&env, &["A"])?;

    let f1 = "feature-1";
    create_local_branch_with_commit_with_message(&env, f1, "Add feature 1");
    let f2 = "feature-2";
    create_local_branch_with_commit_with_message(&env, f2, "Add feature 2");

    // Apply both branches
    env.but("branch apply")
        .arg(f1)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Applied branch 'feature-1' to workspace

"#]])
        .stderr_eq(str![]);

    env.but("branch apply")
        .arg(f2)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Applied branch 'feature-2' to workspace

"#]])
        .stderr_eq(str![]);

    insta::assert_snapshot!(env.git_log()?, @r"
    *-.   7044ae9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 4e81b31 (feature-2) Add feature 2
    | * | 9c2fe5c (feature-1) Add feature 1
    | |/  
    * / 9477ae7 (A) add A
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
