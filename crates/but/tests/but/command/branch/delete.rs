use crate::utils::{CommandExt, Sandbox};
use snapbox::str;

#[test]
fn rejects_non_existent_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch delete no-such-branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find branch: 'no-such-branch'

Hint: Run `but status` for applicable targets.

"#]])
        .stdout_eq(str![[]]);
}

#[test]
fn can_delete_branch_with_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A"]);

    env.but("branch delete A")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B] [HEAD]
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // With one lane left the workspace serves no purpose, so `HEAD` moves onto it, as unapply does.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* d3e2ba3 (HEAD -> B) add B
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn can_delete_branch_with_commits_in_the_bottom_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete A")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [C] [HEAD]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* ec33a86 (HEAD -> C) add C
* 05d3df1 (B) add B
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn can_delete_branch_with_commits_in_the_middle_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete B")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [C] [HEAD]
┊●   wlx add C
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 983f317 (HEAD -> C) add C
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn can_delete_branch_with_commits_in_the_top_of_a_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete C")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B] [HEAD]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 582f37b (HEAD -> B) add B
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn can_delete_branches_via_short_code() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("branch delete g0")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [B] [HEAD]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn removing_checked_out_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new one").assert().success();

    env.but("branch delete one").assert().success();

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

#[test]
fn deleting_the_last_lane_keeps_the_workspace_when_checkout_would_overwrite_changes() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("branch new empty").assert().success();
    env.invoke_bash("git switch main && echo target > collision && git add collision && git commit -m target && git switch empty && echo local > collision");
    env.but("switch --workspace").assert().success();

    env.but("branch delete empty").assert().success();

    // Checking out the target would overwrite `collision`, so the workspace stays.
    snapbox::assert_data_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        str!["refs/heads/gitbutler/workspace"]
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("collision")).unwrap(),
        "local\n",
        "the uncommitted file is preserved"
    );
}

#[test]
fn can_delete_unapplied_branch_by_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("unapply A").assert().success();

    env.but("branch delete A")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'A'

"#]]);

    // `A` and its commit are gone, the applied `B` is untouched.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* d3e2ba3 (HEAD -> B) add B
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    env.but("undo").assert().success();

    // Undo brings the unapplied branch back with its commit.
    snapbox::assert_data_eq!(env.invoke_git("log --oneline -1 A"), str!["9477ae7 add A"]);
}

#[test]
fn can_delete_applied_and_unapplied_branches_together() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.invoke_git("branch unapplied A");

    env.but("--json branch delete unapplied B")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
{
  "type": "branches",
  "branches": [
    "unapplied",
    "B"
  ]
}

"#]]);

    // With one lane left the workspace serves no purpose, so `HEAD` moves onto it, as unapply does.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 9477ae7 (HEAD -> A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn unapplied_branch_checked_out_in_another_worktree_is_kept() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let worktree = env.app_data_dir().join("worktree");
    env.invoke_git(&format!(
        "worktree add -q -b elsewhere {} main",
        worktree.display()
    ));

    env.but("branch delete elsewhere")
        .assert()
        .failure()
        .stdout_eq(str![[""]])
        .stderr_eq(str![[r#"
Error: Refusing to delete a branch that is checked out. Worktrees are: [..]

"#]]);

    snapbox::assert_data_eq!(
        env.invoke_git("branch --list elsewhere"),
        str!["+ elsewhere"]
    );
}

#[test]
fn refuses_to_delete_the_workspace_branch_by_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("branch delete gitbutler/workspace")
        .assert()
        .failure()
        .stdout_eq(str![[""]])
        .stderr_eq(str![[r#"
Error: Cannot delete GitButler technical branch 'gitbutler/workspace'

"#]]);

    snapbox::assert_data_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        str!["refs/heads/gitbutler/workspace"]
    );
}
