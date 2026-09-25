use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn switch_requires_single_branch_feature() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("config feature single-branch disable")
        .assert()
        .success();

    env.but("pick d3e2ba3 -b picked --switch")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: `--switch` requires the `single-branch` feature to be enabled

Hint: Enable the feature with `but config feature single-branch enable`

"#]]);
}

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
fn pick_commit_to_new_branch_with_switch_in_single_branch_mode() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("switch A").assert().success();

    env.but("pick d3e2ba3 -b picked --switch")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'picked' to create olw

"#]]);

    // Picking with --switch creates an independent branch at the target, not above A.
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "picked",
        "the new branch must be checked out"
    );
    assert_eq!(
        env.invoke_git("rev-parse picked^"),
        env.invoke_git("rev-parse gitbutler/target"),
        "the picked commit must be directly above the target"
    );
    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ pi [picked]
┊●   olw add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn failed_pick_switch_to_existing_branch_rolls_back_materialization() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("checkout -b source");
    env.file("picked.txt", "picked content\n");
    env.invoke_git("add picked.txt");
    env.invoke_git("commit -m 'source commit'");
    env.invoke_git("checkout main");
    env.file("first", "Some text\n");
    env.but("commit -b foo -m 'add first'").assert().success();
    env.but("branch new other").assert().success();
    env.file("first", "changes\n");

    // Materializing the pick succeeds, but the remaining modification blocks checkout.
    let source = env.invoke_git("rev-parse source");
    env.but(format!("pick {source} -b other --switch"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not safely check out 'refs/heads/other' from [..] to [..]

Caused by:
    Uncommitted files would be overwritten by checkout: "first"

"#]]);

    // The unselected modification must survive; picked.txt must not leak into the worktree.
    env.but("diff").assert().success().stdout_eq(str![[r#"
──────────────╮
 lz:7 M first │
──────────────╯

@@ -1,1 +1,1 @@
───────────────
1 ┊   │ -Some text
  ┊ 1 │ +changes

"#]]);
    // The destination remains empty instead of retaining the unsuccessfully picked commit.
    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   lz M first
┊
┊╭┄ ot [other] (no commits)
├╯
┊
┊╭┄ fo [foo]
┊●   ppu add first
┊│     ppu:l A first
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
    // Workspace and destination refs return to their original commits; source remains untouched.
    snapbox::assert_data_eq!(
        env.git_log(),
        str![[r#"
*   3fcc02f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
| * 4db633b (foo) add first
|/  
| * 181ba7d (source) source commit
|/  
* b1540e5 (origin/main, origin/HEAD, other, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn failed_pick_switch_restores_ad_hoc_stack_refs() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.invoke_git("checkout -b source");
    env.file("picked.txt", "picked content\n");
    env.invoke_git("add picked.txt");
    env.invoke_git("commit -m 'source commit'");
    env.invoke_git("checkout main");
    env.but("branch new bottom").assert().success();
    env.but("commit --empty -m bottom").assert().success();
    env.but("branch new middle --above bottom")
        .assert()
        .success();
    env.but("commit --empty -m middle").assert().success();
    env.but("branch new top --above middle").assert().success();
    env.file("first.txt", "original\n");
    env.but("commit -m 'add first'").assert().success();
    env.file("first.txt", "modified\n");

    // Inserting on bottom rewrites middle and top before checkout fails.
    let source = env.invoke_git("rev-parse source");
    env.but(format!("pick {source} -b bottom --switch"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not safely check out 'refs/heads/bottom' from [..] to [..]

Caused by:
    Uncommitted files would be overwritten by checkout: "first.txt"

"#]]);

    // The blocking modification survives, without adding picked.txt to the worktree.
    env.but("diff").assert().success().stdout_eq(str![[r#"
──────────────────╮
 zo:f M first.txt │
──────────────────╯

@@ -1,1 +1,1 @@
───────────────
1 ┊   │ -original
  ┊ 1 │ +modified

"#]]);
    // No picked commit remains on bottom, and middle is still between bottom and top.
    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   zo M first.txt
┊
┊╭┄ to [top]
┊●   muz add first
┊│     muz:z A first.txt
┊│
┊├┄ mi [middle]
┊●   xpx middle (no changes)
┊│
┊├┄ bo [bottom]
┊●   lsm bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
    // Rollback restores all ad-hoc refs, not just HEAD, and leaves source alone.
    snapbox::assert_data_eq!(
        env.git_log(),
        str![[r#"
* 181ba7d (source) source commit
| * d607d72 (HEAD -> top) add first
| * beaf835 (middle) middle
| * 119880a (bottom) bottom
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
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

#[test]
fn pick_commit_above_branch_with_new_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    // Keep unapply from switching to single-branch mode.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A", "B"]);
    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --above A -b top")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'top' to create olw

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   olw add B
┊│
┊├┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_commit_below_branch_with_new_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    // Keep unapply from switching to single-branch mode.
    env.but("config feature single-branch disable")
        .assert()
        .success();
    env.setup_metadata(&["A", "B"]);
    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --below A -b bottom")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'bottom' to create olw

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│
┊├┄ bo [bottom]
┊●   olw add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pick_relative_to_branch_rejects_existing_new_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    for side in ["--above", "--below"] {
        env.but(format!("pick d3e2ba3 {side} A -b B"))
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: A branch named 'B' is already applied

"#]]);
    }

    env.but("unapply B").assert().success();

    for side in ["--above", "--below"] {
        env.but(format!("pick d3e2ba3 {side} A -b B"))
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: A branch named 'B' exists but is not applied

"#]]);
    }
}

#[test]
fn pick_relative_to_commit_rejects_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    for command in [
        "pick d3e2ba3 --above tpm -b new-branch",
        "pick d3e2ba3 --above tpm -b",
        "pick d3e2ba3 --below tpm -b new-branch",
        "pick d3e2ba3 --below tpm -b",
    ] {
        env.but(command).assert().failure().stderr_eq(str![[r#"
Error: Cannot use `-b/--branch` when committing relative to commits

"#]]);
    }
}

#[test]
fn pick_relative_to_worktree_rejects_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    super::util::enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    super::util::add_worktree_with_commit(&env, "wt-inside", "A");

    for command in [
        "pick d3e2ba3 --below wt-inside -b new-branch",
        "pick d3e2ba3 --below wt-inside -b",
    ] {
        env.but(command).assert().failure().stderr_eq(str![[r#"
Error: Cannot use `-b/--branch` when committing relative to worktrees

"#]]);
    }
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
