use crate::utils::Sandbox;
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
fn rejects_the_target_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch delete main")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot delete 'main': it is the workspace target branch.

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
fn can_delete_unapplied_branch_and_undo_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("unapply B").assert().success();

    env.but("branch delete B")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'B'

"#]]);

    // `B` and its commit are gone, `A` is untouched.
    snapbox::assert_data_eq!(
        env.git_log(),
        str![[r#"
* 9477ae7 (HEAD -> A) add A
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    // Undo brings `B` back on its commit.
    env.but("undo").assert().success();
    snapbox::assert_data_eq!(
        env.git_log(),
        str![[r#"
* 9477ae7 (HEAD -> A) add A
| * d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn can_delete_applied_and_unapplied_branches_together() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("unapply B").assert().success();

    env.but("branch delete A B")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branches 'A', 'B'

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        str![[r#"
* 0dc3733 (HEAD -> main, origin/main, origin/HEAD, gitbutler/target) add M

"#]]
    );
}

#[test]
fn refuses_a_branch_checked_out_in_a_linked_worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("worktree list").assert().success();
    crate::command::util::add_worktree_with_commit(&env, "wt", "main");

    for name in ["wt", "refs/heads/wt"] {
        env.but(format!("branch delete {name}"))
            .assert()
            .failure()
            .stdout_eq(str![[]])
            .stderr_eq(str![[r#"
Error: Cannot delete 'wt': it is checked out in worktree wt

Hint: Run `but worktree remove wt` first, then `but branch delete wt`

"#]]);
    }

    // The worktree's branch and its commit are untouched.
    snapbox::assert_data_eq!(
        env.invoke_git("log --oneline wt"),
        str![[r#"
[..] add W
0dc3733 add M
"#]]
    );

    // Following the hint deletes the branch.
    env.but("worktree remove wt").assert().success();
    env.but("branch delete wt")
        .assert()
        .success()
        .stderr_eq(str![[""]])
        .stdout_eq(str![[r#"
Discarded branch 'wt'

"#]]);
    snapbox::assert_data_eq!(env.invoke_git("branch --list wt"), str![""]);
}
