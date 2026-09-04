use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn rejects_unnamed_segment_as_source_or_target() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    for command in ["pick g0 -A tpm", "pick tpm -A g0", "pick tpm -B g0"] {
        env.but(command)
            .assert()
            .failure()
            .stdout_eq(str![])
            .stderr_eq(str![[r#"
Error: Cannot operate on anonymous branch 'g0'

Hint: Name it with `but reword g0` first! Note that the short ID is likely to change when the branch is named.

"#]]);
    }
}

#[test]
fn pick_commit_to_existing_branch_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("--json pick d3e2ba3")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "commits": [
    {
      "sourceCommitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
      "newCommitId": "377d9b6254686a0db564f05d10335964f5f6b6b8",
      "newChangeId": "olwryqwqsuwkkoquvrzqrylopwzxuwym"
    }
  ]
}

"#]]);
}

#[test]
fn pick_rejects_non_commit_object() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let tree = env.invoke_git("rev-parse 9477ae7^{tree}");

    env.but(format!("pick {tree} --branch new-branch"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: '[..]' is not a commit

"#]]);
}

#[test]
fn pick_duplicate_sources_outputs_each_commit_once() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("--json pick 9477ae7 9477ae7 d3e2ba3 --branch new-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "commits": [
    {
      "sourceCommitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
      "newCommitId": "099246abb19bef74be7ffaff15cb87ecc6336280",
      "newChangeId": "oxttmukspktmpswzopzsllotqtnrrlpz"
    },
    {
      "sourceCommitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
      "newCommitId": "bffaa089ac8489464f35f0180a4e982987cd47e3",
      "newChangeId": "olwryqwqsuwkkoquvrzqrylopwzxuwym"
    }
  ],
  "branch": "new-branch"
}

"#]]);
}

#[test]
fn pick_commit_to_new_branch_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    // Keep the managed workspace: in single-branch mode unapply would check out a plain branch.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A"]);

    env.but("unapply A").assert().success();

    env.but("--json pick 9477ae7 --branch new-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "commits": [
    {
      "sourceCommitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
      "newCommitId": "099246abb19bef74be7ffaff15cb87ecc6336280",
      "newChangeId": "oxttmukspktmpswzopzsllotqtnrrlpz"
    }
  ],
  "branch": "new-branch"
}

"#]]);
}

#[test]
fn pick_commit_to_default_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto branch 'A' to create olw

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● olw author 2000-01-01 00:00:00 +0000 (sha 377d9b6)
┊│     add B 
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 9477ae7)
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_to_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    // Keep the managed workspace: in single-branch mode unapply would check out a plain branch.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A"]);

    env.but("unapply A").assert().success();

    env.but("pick 9477ae7 -b new-branch")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked 9477ae7 onto new branch 'new-branch' to create oxt

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch]
┊● oxt author 2000-01-01 00:00:00 +0000 (sha 099246a)
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --above 9477ae7")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 to create olw

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● olw author 2000-01-01 00:00:00 +0000 (sha 377d9b6)
┊│     add B 
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 9477ae7)
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --below 9477ae7")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 to create olw

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha fb0504e)
┊│     add A 
┊● olw author 2000-01-01 00:00:00 +0000 (sha 1ad814c)
┊│     add B 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_above_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    // Keep the managed workspace: in single-branch mode unapply would check out a plain branch.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --above A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'a-branch-1' to create olw

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● olw author 2000-01-01 00:00:00 +0000 (sha 377d9b6)
┊│     add B 
┊│
┊├┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 9477ae7)
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_below_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    // Keep the managed workspace: in single-branch mode unapply would check out a plain branch.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --below A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'a-branch-1' to create olw

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha fb0504e)
┊│     add A 
┊│
┊├┄ br [a-branch-1]
┊● olw author 2000-01-01 00:00:00 +0000 (sha 1ad814c)
┊│     add B 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

/// A commit owned by a linked worktree can be cherry-picked onto another stack's branch by
/// its ID: the pick is a copy, so the worktree's history and checkout stay untouched.
#[test]
fn pick_a_worktree_commit_onto_a_workspace_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    super::util::enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    super::util::add_worktree_with_commit(&env, "wt-feature", "A");

    env.but("pick 580bef0 -b B")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Picked 580bef0 onto branch 'B' to create qnu

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   c2cbac1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 0f3edd1 (B) add W
* | d3e2ba3 add B
| | * 580bef0 (wt-feature) add W
| |/  
| * 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

/// A worktree is also a pick destination: the copy lands on the tip of the branch that
/// worktree has checked out, and the source branch stays untouched.
#[test]
fn pick_a_commit_onto_a_worktree_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    super::util::enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    super::util::add_worktree_with_commit(&env, "wt-inside", "A");

    env.but("pick lrm -b wt-inside")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto branch 'wt-inside' to create olw

"#]]);

    // The copy sits on wt-inside's tip; B keeps its own "add B" and the workspace is unchanged.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | d3e2ba3 (B) add B
| | * 7bc6f49 (wt-inside) add B
| | * 580bef0 add W
| |/  
| * 9477ae7 (A) add A
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}
