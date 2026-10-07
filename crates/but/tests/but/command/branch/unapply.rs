use bstr::ByteSlice;
use snapbox::IntoData;
use snapbox::str;
use utils::create_local_branch_with_commit;

use crate::{
    command::util,
    utils::{CommandExt, Sandbox},
};

#[test]
fn unapplying_a_worktree_branch_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    util::enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    util::add_worktree_with_commit(&env, "wt-feature", "A");

    env.but("unapply wt-feature")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch wt-feature belongs to worktree wt-feature, which cannot be unapplied

Hint: Use `but worktree remove wt-feature` to remove the worktree

"#]])
        .stdout_eq(str![]);

    // The worktree's branch and its commit are untouched.
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt:@ [uncommitted] {wt-feature} (no changes)
┊┊├┄ wt [wt-feature]
┊┊●   nsn add W
┊├╯
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn anonymous_segment_reports_recovery_workflow() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    env.but("unapply g0")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot operate on anonymous branch 'g0'

Hint: Name it with `but reword g0` first! Note that the short ID is likely to change when the branch is named.

"#]])
        .stdout_eq(str![]);

    let anonymous_commit = env.invoke_git("rev-parse gitbutler/workspace^");
    env.but("branch new recovered --above sxu")
        .assert()
        .success();
    env.but("unapply recovered").assert().success();
    assert_eq!(
        env.invoke_git("rev-parse recovered"),
        anonymous_commit,
        "recovery branch should preserve the anonymous commit"
    );
}

#[test]
fn single_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // First apply the branch
    env.but("apply").arg(branch_name).assert().success();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   9d5d9e5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 9f9d5a6 (feature-branch) Add feature
* | 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );

    // Now unapply the branch using the new `but unapply` command
    env.but("unapply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with 'feature-branch' from workspace

"#]]);

    // Verify the branch is removed from workspace
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 9477ae7 (HEAD -> A) add A
| * 9f9d5a6 (feature-branch) Add feature
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn unapply_with_json_output() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

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

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 9477ae7 (HEAD -> A) add A
| * 9f9d5a6 (feature-branch) Add feature
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn unapply_idempotent() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

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
Error: Could not find source: 'feature-branch'

Hint: Run `but status` for applicable targets.

"#]])
        .stdout_eq(str![]);
}

#[test]
fn unapply_nonexistent_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");

    // Try to unapply a branch that doesn't exist - should fail with the new command
    env.but("unapply nonexistent-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find source: 'nonexistent-branch'

Hint: Run `but status` for applicable targets.

"#]])
        .stdout_eq(str![]);
}

#[test]
fn unapply_nonexistent_branch_with_json() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");

    // Try to unapply a branch that doesn't exist with JSON output - should fail
    env.but("--json unapply nonexistent-branch")
        .allow_json()
        .assert()
        .failure();
}

#[test]
fn unapply_branch_not_in_workspace() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let branch_name = "feature-branch";
    create_local_branch_with_commit(&env, branch_name);

    // Try to unapply a branch that exists but wasn't applied - should fail
    env.but("unapply")
        .arg(branch_name)
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find source: 'feature-branch'

Hint: Run `but status` for applicable targets.

"#]])
        .stdout_eq(str![]);
}

#[test]
fn unapply_remote_tracking_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

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

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   1bb7daf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
* | 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );

    // Unapply the remote branch by its local name (remote-feature, not origin/remote-feature)
    env.but("unapply remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Unapplied stack with 'remote-feature' from workspace

"#]]);

    // Verify it was removed from workspace
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 9477ae7 (HEAD -> A) add A
| * ba02e5f (origin/remote-feature, remote-feature) Add remote feature
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn concurrent_unapply_of_independent_branches_succeeds() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    // Keep the managed workspace: in single-branch mode unapply would check out a plain branch.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A"]);

    create_local_branch_with_commit(&env, "feature-branch-a");

    env.but("apply feature-branch-a").assert().success();

    let child_a = util::but_std_cmd(&env, "unapply A").spawn().unwrap();
    let child_b = util::but_std_cmd(&env, "unapply feature-branch-a")
        .spawn()
        .unwrap();

    let out_a = child_a.wait_with_output().unwrap();
    let out_b = child_b.wait_with_output().unwrap();

    assert!(
        out_a.status.success(),
        "unapply A failed: {}",
        out_a.stderr.as_bstr()
    );
    assert!(
        out_b.status.success(),
        "unapply feature-branch-a failed: {}",
        out_b.stderr.as_bstr()
    );

    let status = util::status_json(&env);
    let is_applied = |branch_name| {
        status["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|stack| stack["branches"].as_array().unwrap())
            .any(|branch| branch["name"].as_str() == Some(branch_name))
    };
    assert!(!is_applied("A"), "A should no longer be applied");
    assert!(
        !is_applied("feature-branch-a"),
        "feature-branch-a should no longer be applied"
    );
}

#[test]
fn unapply_using_cli_branch_id() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Get the CLI ID from status --json
    let status_output = env.but("status --json").allow_json().output().unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout).unwrap();

    // Find the branch's CLI ID - JSON uses camelCase and "branches" not "heads"
    let stacks = status["stacks"]
        .as_array()
        .expect("stacks should be an array");
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
Unapplied stack with 'feature-branch' from workspace

"#]]);

    // Verify the branch is no longer in workspace
    let status_output = env.but("status --json").allow_json().output().unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout).unwrap();
    let stacks = status["stacks"]
        .as_array()
        .expect("stacks should be an array");

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
}

#[test]
fn unapply_using_cli_stack_id() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Get the stack CLI ID from status --json
    let status_output = env.but("status --json").allow_json().output().unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout).unwrap();

    // Find the stack's CLI ID for the feature-branch - JSON uses camelCase and "branches" not "heads"
    let stacks = status["stacks"]
        .as_array()
        .expect("stacks should be an array");
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
Unapplied stack with 'feature-branch' from workspace

"#]]);
}

#[test]
fn unapply_json_output_validation() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let branch_name = "feature-branch";
    utils::create_local_branch_with_commit(&env, branch_name);

    // Apply the branch
    env.but("apply").arg(branch_name).assert().success();

    // Unapply with JSON output and validate structure
    let output = env
        .but("--json unapply")
        .arg(branch_name)
        .allow_json()
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    // Validate JSON structure
    let branches = json["branches"]
        .as_array()
        .expect("branches should be an array");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0], serde_json::json!("feature-branch"));
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

#[test]
fn unapply_last_branch_uses_the_targets_local_tracking_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("branch -m main integration");
    env.but("commit -b one -m one").assert().success();
    let branch_before = env.invoke_git("rev-parse one");

    env.but("unapply one").assert().success();

    assert_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        "refs/heads/integration",
        "the local tracking branch need not share the target's name"
    );
    assert_eq!(
        env.invoke_git("rev-parse one"),
        branch_before,
        "unapply preserves the branch and its commits"
    );
}

#[test]
fn unapply_last_branch_without_local_tracking_branch_fails() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m one").assert().success();
    env.invoke_git("branch -D main");
    let before = env.git_log();

    env.but("unapply one")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot unapply the last branch because the target has no local tracking branch

"#]]);

    snapbox::assert_data_eq!(env.git_log(), before);
}

#[test]
fn unapply_last_branch_is_disabled_without_single_branch_feature() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m one").assert().success();
    env.but("config feature single-branch disable")
        .assert()
        .success();
    let before = env.git_log();

    env.but("unapply one").assert().failure().stderr_eq(str![[r#"
Error: Setup required: Not currently on a gitbutler/* branch. - run `but setup` to configure the project

"#]]);

    snapbox::assert_data_eq!(env.git_log(), before);
}

#[test]
fn unapply_last_branch_can_be_undone() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m one").assert().success();
    env.but("commit -b two -m two").assert().success();
    env.but("unapply two").assert().success();
    let before = env.git_log();

    env.but("unapply one").assert().success();
    env.but("undo").assert().success();

    snapbox::assert_data_eq!(env.git_log(), before);
    env.but("switch --workspace").assert().success();
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one]
┊●   lsm one (no changes)
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unapply_last_branch_preserves_conflicting_uncommitted_changes() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("commit -b one -m one").assert().success();
    env.but("commit -b two -m two").assert().success();
    env.but("unapply two").assert().success();
    env.invoke_bash("git switch main && echo target > collision && git add collision && git commit -m target && git switch one && echo local > collision");
    let before = env.git_log();
    let status_before = env.git_status();

    env.but("unapply one")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Uncommitted files would be overwritten by checkout: "collision"

"#]]);

    snapbox::assert_data_eq!(env.git_log(), before);
    snapbox::assert_data_eq!(env.git_status(), status_before);
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("collision")).unwrap(),
        "local\n",
        "failed checkout must preserve the untracked file"
    );
    env.but("switch --workspace").assert().success();
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   qx A collision
┊
┊╭┄ on [one]
┊●   lsm one (no changes)
├╯
┊
┴ b1540e5 (common base, origin/main) 2000-01-02 M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn unapply_last_branch_with_existing_workspace_can_be_undone() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("one.txt", "work on one\n");
    env.but("commit -b one -m one").assert().success();
    env.but("switch --workspace").assert().success();
    env.but("switch one").assert().success();
    let before = env.git_log();

    env.but("unapply one").assert().success();
    env.but("undo").assert().success();
    // Undo restores both the checkout and the saved workspace commit.
    snapbox::assert_data_eq!(env.git_log(), before.into_data().raw());
    env.but("switch --workspace").assert().success();
    // Undo also restores one's applied state, so it is included on workspace recreation.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 53ed094 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9939bf0 (one) one
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );
    assert!(
        env.projects_root().join("one.txt").exists(),
        "undo must restore the unapplied branch's work"
    );
}

#[test]
fn unapply_last_branch_checkout_failure_preserves_existing_workspace() {
    use but_core::RefMetadata as _;

    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("one.txt", "work on one\n");
    env.but("commit -b one -m one").assert().success();
    env.but("switch --workspace").assert().success();
    env.but("switch one").assert().success();
    env.invoke_bash("git switch main && echo target > collision && git add collision && git commit -m target && git switch one");
    env.file("collision", "local\n");
    let before = env.git_log();
    let metadata_before = format!(
        "{:?}",
        env.meta()
            .workspace(but_core::WORKSPACE_REF_NAME.try_into().unwrap())
            .unwrap()
            .stacks
    );

    env.but("unapply one")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Uncommitted files would be overwritten by checkout: "collision"

"#]]);

    // Neither the checkout nor the saved workspace may change on checkout failure.
    snapbox::assert_data_eq!(env.git_log(), before.into_data().raw());
    snapbox::assert_data_eq!(
        format!(
            "{:?}",
            env.meta()
                .workspace(but_core::WORKSPACE_REF_NAME.try_into().unwrap())
                .unwrap()
                .stacks
        ),
        metadata_before
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("collision")).unwrap(),
        "local\n",
        "failed checkout must preserve the conflicting untracked file"
    );
    assert!(
        env.projects_root().join("one.txt").exists(),
        "failed unapply must preserve the checked-out branch's files"
    );
}

#[test]
fn unapply_checked_out_branch_preserves_other_remembered_applied_branches() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("one.txt", "work on one\n");
    env.but("commit -b one -m one").assert().success();
    env.file("two.txt", "work on two\n");
    env.but("commit -b two -m two").assert().success();
    env.but("switch one").assert().success();
    // Both branches are remembered in the retained workspace before unapply.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   593bd9e (gitbutler/workspace) GitButler Workspace Commit
|\  
| * 9939bf0 (HEAD -> one) one
* | 2c9e5ce (two) two
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );

    env.but("unapply one").assert().success();
    env.but("switch --workspace").assert().success();
    assert!(
        !env.projects_root().join("one.txt").exists(),
        "the explicitly unapplied branch must not return"
    );
    assert!(
        env.projects_root().join("two.txt").exists(),
        "the other remembered applied branch must still be restored"
    );
    // The rebuilt workspace contains only two, while one remains available to reapply.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 75ac05b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 2c9e5ce (two) two
| * 9939bf0 (one) one
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );
}

#[test]
fn unapply_last_branch_does_not_restore_its_work_when_switching_to_existing_workspace() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.file("one.txt", "work on one\n");
    let file = env.projects_root().join("one.txt");
    env.but("commit -b one -m one").assert().success();

    env.but("switch --workspace").assert().success();
    env.but("switch one").assert().success();
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] [HEAD]
┊●   qmv one
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
    // The managed workspace ref still exists while HEAD is on one.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 78a1aad (gitbutler/workspace) GitButler Workspace Commit
* 9939bf0 (HEAD -> one) one
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );

    // The saved workspace is emptied immediately, without checking it out or deleting one.
    env.but("unapply one").assert().success();
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 86b310d (gitbutler/workspace) GitButler Workspace Commit
| * 9939bf0 (one) one
|/  
* b1540e5 (HEAD -> main, origin/main, origin/HEAD, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );
    assert!(
        !file.exists(),
        "unapply must remove the branch's committed file from the worktree"
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    // Returning to the workspace must not bring one's commit back into its ancestry.
    env.but("switch --workspace").assert().success();
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 86b310d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 9939bf0 (one) one
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );
    assert!(
        !file.exists(),
        "switching back to an existing workspace must not restore the unapplied branch's file"
    );
}

#[test]
fn unapplying_takes_you_from_workspace_to_sbm() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("commit -b one -m one").assert().success();

    env.but("commit -b two -m two").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ tw [two]
┊●   l#0 two (no changes)
├╯
┊
┊╭┄ on [one]
┊●   l#1 one (no changes)
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply two").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] [HEAD]
┊●   lsm one (no changes)
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply one").assert().success();
    assert_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        "refs/heads/main",
        "unapplying the last branch checks out the target's local tracking branch"
    );

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* d0e8c26 (one) one
| * 22324ee (two) two
|/  
* b1540e5 (HEAD -> main, origin/main, origin/HEAD, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );

    env.but("switch --workspace").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* be6b55d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * d0e8c26 (one) one
|/  
| * 22324ee (two) two
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
        .raw()
    );
}

#[test]
fn unapplying_the_last_branch_checks_out_the_target() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new one").assert().success();

    env.but("switch --workspace").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base, main, origin/main) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply one").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base, main, origin/main, HEAD) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}
