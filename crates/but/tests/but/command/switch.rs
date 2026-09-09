use snapbox::{assert_data_eq, str};

use crate::utils::CommandExt as _;
use crate::utils::Sandbox;

fn status_json(env: &crate::utils::Sandbox) -> serde_json::Value {
    let output = env.but("--json status").allow_json().output().unwrap();
    serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("status output should be valid JSON: {err}"))
        .unwrap()
}

fn assert_workspace_status(env: &crate::utils::Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switches_to_existing_branch_by_short_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_workspace_status(&env);

    env.but("switch A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
}

#[test]
fn switches_to_existing_branch_by_full_ref() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_workspace_status(&env);

    env.but("switch refs/heads/A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
}

#[test]
fn switches_to_existing_branch_with_remote_like_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.invoke_git("branch origin/main main");

    env.but("switch origin/main")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'origin/main'

"#]]);

    assert_eq!(
        env.invoke_git("rev-parse --symbolic-full-name HEAD"),
        "refs/heads/origin/main"
    );
}

#[test]
fn switches_to_existing_branch_by_workspace_cli_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_workspace_status(&env);

    let status = status_json(&env);
    let branch_cli_id = status["stacks"][0]["branches"][0]["cliId"]
        .as_str()
        .expect("branch cli id should exist");

    env.but(format!("switch {branch_cli_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
}

#[test]
fn switching_to_lower_branch_only_shows_it_in_single_branch_mode() {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-two-dependent-branches",
    );
    env.setup_single_stack_metadata_at_target(&["B", "A"], "origin/main");

    // The managed workspace shows the complete stack before switching.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("switch A").assert().success();

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");
    // Switching to A should hide B above it from single-branch status.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switching_to_reordered_empty_branch_preserves_lower_branches() {
    let env = crate::utils::Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.but("branch new A").assert().success();
    env.but("branch new B").assert().success();
    env.but("branch new D --above A").assert().success();
    env.but("branch new C --above D").assert().success();
    env.but("move A --above D").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B] (no commits)
├╯
┊
┊╭┄ h0 [C] (no commits)
┊│
┊├┄ i0 [A] (no commits)
┊│
┊├┄ j0 [D] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("switch A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to branch 'A'

"#]]);
    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "A");

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
┊│
┊├┄ h0 [D] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switches_back_to_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.invoke_git("checkout A");

    env.but("switch --workspace")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref HEAD"),
        "gitbutler/workspace"
    );

    assert_workspace_status(&env);
}

#[test]
fn creates_named_branch_and_switches_to_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_workspace_status(&env);

    env.but("switch --new my-feature")
        .assert()
        .success()
        .stderr_eq(str![[r#"
⚠ `--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead

"#]])
        .stdout_eq(str![[r#"
Created branch 'my-feature'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "my-feature");
    assert_eq!(
        env.invoke_git("rev-parse my-feature"),
        env.invoke_git("rev-parse main")
    );
}

#[test]
fn creates_named_branch_with_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("--json switch --new my-feature")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![[r#"
⚠ `--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead

"#]])
        .stdout_eq(str![[r#"
{
  "type": "createdBranch",
  "branch": "my-feature"
}

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "my-feature");
}

#[test]
fn creates_generated_branch_and_switches_to_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    assert_workspace_status(&env);

    env.but("switch --new")
        .assert()
        .success()
        .stderr_eq(str![[r#"
⚠ `--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead

"#]])
        .stdout_eq(str![[r#"
Created branch 'a-branch-1'

"#]]);

    assert_eq!(env.invoke_git("rev-parse --abbrev-ref HEAD"), "a-branch-1");
    assert_eq!(
        env.invoke_git("rev-parse a-branch-1"),
        env.invoke_git("rev-parse main")
    );
}

#[test]
fn rejects_workspace_with_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("switch --workspace A")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: the argument '--workspace' cannot be used with '[TARGET]'

Usage: but switch <TARGET|--workspace|--new>

For more information, try '--help'.

"#]]);
}

#[test]
fn rejects_remote_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("switch origin/main")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Can only switch to local branches, got 'origin/main'

"#]]);
}

#[test]
fn rejects_non_branch_cli_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let status = status_json(&env);
    let commit_cli_id = status["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .expect("commit cli id should exist");

    env.but(format!("switch {commit_cli_id}"))
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Could not find branch: 'tpm'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn switching_to_workspace_creates_workspace_if_necessary() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    // no workspace exists
    assert_data_eq!(
        env.git_log(),
        str![[r#"
* b1540e5 (HEAD -> main, origin/main, origin/HEAD) M
* e31e6ca add init

"#]]
    );

    env.but("switch --workspace")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    // workspace exists
    assert_data_eq!(
        env.git_log(),
        str![[r#"
* 6c7afcb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn switching_back_to_workspace_after_committing_in_single_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("switch A").assert().success();
    env.but("commit -b A -m one").assert().success();
    env.but("commit -b A -m two").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ppx two (no changes)
┊●   orn one (no changes)
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
* e1df4fe (HEAD -> A) two
* 4a5164c one
| *   c128bce (gitbutler/workspace) GitButler Workspace Commit
| |/  
| |/  
|/|   
* | 9477ae7 add A
| * d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    env.but("switch --workspace").assert().success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ppx two (no changes)
┊●   orn one (no changes)
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
*   6154bb8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
| * d3e2ba3 (B) add B
* | e1df4fe (A) two
* | 4a5164c one
* | 9477ae7 add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn switching_back_to_workspace_when_already_in_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Already on workspace

"#]]);
}

#[test]
fn switching_back_to_workspace_that_conflicts_with_checked_out_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.but("commit -b one -m 'add one'").assert().success();

    env.file("two", "two content");
    env.but("commit -b two -m 'add two'").assert().success();

    env.file("three", "three content");
    env.but("commit -b three -m 'add three'").assert().success();

    env.but("switch one").assert().success();
    env.file("two", "different two content");
    env.but("commit -m 'add two'").assert().success();

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Switched to workspace

⚠ Failed to apply 'two' due to conflicts with existing stack

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ th [three]
┊●   owr add three
┊│     owr:o A three
├╯
┊
┊╭┄ on [one]
┊●   llm add two
┊│     llm:t A two
┊●   onv add one
┊│     onv:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switching_back_to_workspace_that_conflicts_with_multiple_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.but("commit -b one -m 'add one'").assert().success();

    env.file("two", "two content");
    env.but("commit -b two -m 'add two'").assert().success();

    env.file("three", "three content");
    env.but("commit -b three -m 'add three'").assert().success();

    env.but("switch one").assert().success();
    env.file("two", "different two content");
    env.but("commit -m 'add two'").assert().success();

    env.but("switch three").assert().success();
    env.file("two", "different two content yet again");
    env.but("commit -m 'add two'").assert().success();

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Switched to workspace

⚠ Failed to apply 'two', 'one' due to conflicts with existing stacks

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ th [three]
┊●   uov add two
┊│     uov:t A two
┊●   owr add three
┊│     owr:o A three
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn switching_back_to_empty_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new --switch").assert().success();
    env.but("commit --no-message").assert().success();

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   tqv (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
* de5dad6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f41568d (a-branch-1) 
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn switching_back_to_workspace_from_main_when_workspace_exists() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("switch main").assert().success();

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
* 0bbfbfd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn switching_back_to_workspace_from_main_with_existing_empty_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("switch main").assert().success();

    env.but("switch --workspace")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Switched to workspace

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
* ff4136d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn switching_back_to_workspace_from_main_with_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");

    env.but("commit -b one -m 'add one'").assert().success();
    env.but("switch main").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // intentionally dont commit this file to cause conflicts
    env.file("one", "different content of one");

    env.but("switch --workspace")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Uncommitted files would be overwritten by checkout: "one"

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   kl A one
┊
┊╭┄ ma [main] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    assert_data_eq!(
        env.git_log(),
        str![[r#"
* b34435b (gitbutler/workspace) GitButler Workspace Commit
* 72aceac (one) add one
* 0dc3733 (HEAD -> main, origin/main, origin/HEAD, gitbutler/target) add M

"#]]
    );
}
