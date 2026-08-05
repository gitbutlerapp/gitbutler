use bstr::ByteSlice;
use snapbox::IntoData;
use snapbox::str;

use crate::command::branch::apply::utils::create_local_branch_with_commit_with_message;
use crate::command::util;
use crate::utils::{CommandExt, Sandbox};

use utils::create_local_branch_with_commit;

#[cfg(not(feature = "legacy"))]
#[test]
fn single_branch() {
    let env = Sandbox::open_with_default_settings("one-fork");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r"
    * bf53300 (A) add A
    | * b1540e5 (HEAD -> main) M
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    "]]
    );

    env.but("apply A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied branch 'main' to workspace
Applied branch 'A' to workspace

"#]]);

    snapbox::assert_data_eq!(
        env.workspace_debug_at_head().unwrap(),
        snapbox::str![[r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:2:A on e31e6ca {1}
    │   └── 📙:2:A
    │       └── ·bf53300 (🏘️)
    └── ≡📙:1:main on e31e6ca {2}
        └── 📙:1:main
            └── ·b1540e5 (🏘️)
    "]]
    );

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r"
    *   d87b903 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * bf53300 (A) add A
    * | b1540e5 (main) M
    |/  
    | * 0e391b2 (origin/B) add B
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    "]]
        .raw()
    );

    env.but("apply origin/B")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Applied remote branch 'origin/B' to workspace

"#]])
        .stderr_eq(str![""]);
    snapbox::assert_data_eq!(
        env.workspace_debug_at_head().unwrap(),
        snapbox::str![[r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓! on e31e6ca
    ├── ≡📙:3:B <> origin/B →:4: on e31e6ca {1}
    │   └── 📙:3:B <> origin/B →:4:
    │       └── ❄️0e391b2 (🏘️)
    ├── ≡📙:2:A on e31e6ca {2}
    │   └── 📙:2:A
    │       └── ·bf53300 (🏘️)
    └── ≡📙:1:main on e31e6ca {3}
        └── 📙:1:main
            └── ·b1540e5 (🏘️)
    "]]
    );

    // TODO: should be success and create a local tracking branch.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r"
    *-.   7bcf528 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 0e391b2 (origin/B, B) add B
    | * | bf53300 (A) add A
    | |/  
    * / b1540e5 (main) M
    |/  
    * e31e6ca (origin/main, origin/HEAD) add init
    "]]
        .raw()
    );
}

#[test]
fn local_branch() {
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

    // Apply the local branch
    env.but("apply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied branch 'feature-branch' to workspace

"#]]);
    env.but("apply")
        .arg(branch_name)
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Branch 'feature-branch' is already in the workspace; nothing changed

"#]]);

    // It's idempotent
    env.but("apply feature-branch")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Branch 'feature-branch' is already in the workspace; nothing changed

"#]]);

    // It actually applied the branch, by merging it in.
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
}

#[test]
fn local_branch_with_json_output() {
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

    create_local_branch_with_commit(&env, "feature-branch");

    // Apply with JSON output
    env.but("--json apply feature-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "status": "applied",
  "workspaceChanged": true,
  "appliedBranches": [
    "refs/heads/feature-branch"
  ],
  "workspaceRefCreated": false,
  "conflictingStacks": []
}

"#]])
        .stderr_eq(str![]);

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
}

#[test]
fn remote_branch_creates_local_tracking_branch_automatically() {
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

    // Apply the remote branch, by its shortest name only.
    env.but("apply origin/remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied remote branch 'origin/remote-feature' to workspace

"#]]);

    // It created a local tracking branch.
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
}

#[test]
fn remote_branch_short_name_resolves_to_unique_remote_tracking_branch() {
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

    // Create a remote-only branch reference.
    env.invoke_bash(
        r#"
    git checkout origin/main
    git commit -m 'Add remote feature' --allow-empty
    git update-ref refs/remotes/origin/remote-feature HEAD
    git checkout gitbutler/workspace
"#,
    );

    // Apply the remote branch by its bare name.
    env.but("apply remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied remote branch 'origin/remote-feature' to workspace

"#]]);

    // It created the same local tracking branch as the qualified form.
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
}

#[test]
fn remote_branch_short_name_requires_disambiguation_across_multiple_remotes() {
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

    // Create two configured remotes that both expose the same short branch name.
    env.invoke_bash(
        r#"
    git remote add upstream .
    git checkout origin/main
    git commit -m 'Add remote feature' --allow-empty
    git update-ref refs/remotes/origin/remote-feature HEAD
    git update-ref refs/remotes/upstream/remote-feature HEAD
    git checkout gitbutler/workspace
"#,
    );

    env.but("apply remote-feature")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Failed to apply branch

Caused by:
    The reference 'remote-feature' did not exist

"#]])
        .stdout_eq(str![""]);

    env.but("apply origin/remote-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Applied remote branch 'origin/remote-feature' to workspace

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   1bb7daf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * ba02e5f (upstream/remote-feature, origin/remote-feature, remote-feature) Add remote feature
* | 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn concurrent_apply_of_independent_branches_succeeds() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    create_local_branch_with_commit(&env, "feature-branch-a");
    create_local_branch_with_commit_with_message(&env, "feature-branch-b", "Add other feature");

    let child_a = util::but_std_cmd(&env, "apply feature-branch-a")
        .spawn()
        .unwrap();
    let child_b = util::but_std_cmd(&env, "apply feature-branch-b")
        .spawn()
        .unwrap();

    let out_a = child_a.wait_with_output().unwrap();
    let out_b = child_b.wait_with_output().unwrap();

    assert!(
        out_a.status.success(),
        "apply feature-branch-a failed: {}",
        out_a.stderr.as_bstr()
    );
    assert!(
        out_b.status.success(),
        "apply feature-branch-b failed: {}",
        out_b.stderr.as_bstr()
    );

    let status = util::status_json(&env);
    util::find_branch(&status, "feature-branch-a");
    util::find_branch(&status, "feature-branch-b");
}

#[test]
fn nonexistent_branch() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");

    // Try to apply a branch that doesn't exist
    env.but("apply nonexistent-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Failed to apply branch

Caused by:
    The reference 'nonexistent-branch' did not exist

"#]])
        .stdout_eq(str![""]);
}

#[test]
fn nonexistent_branch_with_json() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");

    // Try to apply a branch that doesn't exist with JSON output
    env.but("--json apply nonexistent-branch")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Failed to apply branch

Caused by:
    The reference 'nonexistent-branch' did not exist

"#]]);
    // Note: Currently the apply function doesn't output anything with JSON when branch not found
    // This might be improved to output an error in JSON format
}

#[test]
fn multiple_branches_sequentially() {
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

    let f1 = "feature-1";
    create_local_branch_with_commit_with_message(&env, f1, "Add feature 1");
    let f2 = "feature-2";
    create_local_branch_with_commit_with_message(&env, f2, "Add feature 2");

    // Apply both branches
    env.but("apply")
        .arg(f1)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Applied branch 'feature-1' to workspace

"#]])
        .stderr_eq(str![]);

    env.but("apply")
        .arg(f2)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Applied branch 'feature-2' to workspace

"#]])
        .stderr_eq(str![]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*-.   7044ae9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 4e81b31 (feature-2) Add feature 2
| * | 9c2fe5c (feature-1) Add feature 1
| |/  
* / 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn apply_branch_conflicting_with_workspace_reports_error() {
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

    env.invoke_bash(
        r#"
    git checkout main -b conflicting-branch;
    echo 'conflicting-A-content' > A;
    git add A;
    git commit -m 'Add conflicting A';
    git checkout gitbutler/workspace;
    "#,
    );

    // It's notable that this behaviour is different from what the GUI does, which
    // unapplies all conflicting instead.
    env.but("apply conflicting-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to apply branch: 'conflicting-branch' conflicts with existing stack: A

"#]])
        .stdout_eq(str![""]);

    env.but("apply conflicting-branch")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to apply branch: 'conflicting-branch' conflicts with existing stack: A

"#]])
        .stdout_eq(str![""]);
}

#[test]
fn apply_branch_conflicting_with_workspace_reports_json_error() {
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

    env.invoke_bash(
        r#"
    git checkout main -b conflicting-branch;
    echo 'conflicting-A-content' > A;
    git add A;
    git commit -m 'Add conflicting A';
    git checkout gitbutler/workspace;
    "#,
    );

    env.but("apply conflicting-branch --json")
        .assert()
        .failure()
        // intentionally prints to stdout since that makes it easier for tools to consume the JSON
        // output
        .stdout_eq(str![[r#"
{
  "status": "conflictAborted",
  "workspaceChanged": true,
  "appliedBranches": [],
  "workspaceRefCreated": false,
  "conflictingStacks": [
    "refs/heads/A"
  ]
}

"#]])
        .stderr_eq(str![""]);
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
