use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn rejects_unnamed_segment_as_anchor() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    for command in ["branch new recovered -A g0", "branch new recovered -B g0"] {
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
fn outputs_branch_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );

    env.setup_metadata(&["A"]);

    env.but("branch new my-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'my-feature'

"#]]);

    env.but("branch new --above tpm my-anchored-feature")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'my-anchored-feature' above commit tpm

"#]]);
}

#[test]
fn rejects_anchor_outside_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("branch new --above A new-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'A'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn rejects_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);
}

#[test]
fn rejects_name_that_normalizes_to_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new HEAD-")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD-'

Invalid branch name: Could not turn "HEAD-" into a valid reference name

"#]]);
}

#[test]
fn rejects_name_that_normalizes_to_something_else_and_suggests_alternative() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new 'my branch'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'my branch'

Invalid branch name

Hint: Try 'my-branch' instead

"#]]);
}

#[test]
fn rejects_branch_name_already_applied_in_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' is already applied

"#]]);
}

#[test]
fn rejects_name_that_exists_outside_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    env.but("branch new A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: A branch named 'A' exists but is not applied

"#]]);
}

#[test]
fn with_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("--json branch new middle")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "middle"
}

"#]]);

    env.but("branch new --json bottom --below middle")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "bottom"
}

"#]]);

    env.but("branch new --json top --above middle")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
{
  "branch": "top"
}

"#]]);
}

#[test]
fn in_single_branch_mode_creating_stacked_branches() {
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

    env.but("branch new middle")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'middle'

"#]]);

    // NOTE: the fact that `┊●   ply add init` suddenly shows up appears to be a bug in but-graph.
    // At least according to gpt-5.6-sol
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ mi [middle] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new bottom --below middle")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'bottom' below branch 'middle'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new top --above middle")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'top' above branch 'middle'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new between-middle-and-top --above middle")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'between-middle-and-top' above branch 'middle'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ et [between-middle-and-top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> top, origin/main, origin/HEAD, middle, main, gitbutler/target, bottom, between-middle-and-top) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn in_single_branch_mode_create_new_branches_with_commits() {
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

    env.but("branch new middle")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'middle'

"#]]);

    env.but("commit --empty -b middle -m 'on middle'")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ mi [middle]
┊●   lsm on middle (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new top --above middle")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'top' above branch 'middle'

"#]]);

    env.but("commit --empty -b top -m 'on top'")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   qzl on top (no changes)
┊│
┊├┄ mi [middle]
┊●   lsm on middle (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 426ad51 (HEAD -> top) on top
* 8b08d79 (middle) on middle
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("branch new bottom --below middle")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'bottom' below branch 'middle'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   qzl on top (no changes)
┊│
┊├┄ mi [middle]
┊●   lsm on middle (no changes)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("commit --empty -b bottom -m 'on bottom'")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   qzl on top (no changes)
┊│
┊├┄ mi [middle]
┊●   l#0 on middle (no changes)
┊│
┊├┄ bo [bottom]
┊●   l#1 on bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new between-middle-and-top --above middle")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'between-middle-and-top' above branch 'middle'

"#]]);

    env.but("commit --empty -b between-middle-and-top -m 'on between-middle-and-top'")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   qzl on top (no changes)
┊│
┊├┄ et [between-middle-and-top]
┊●   pky on between-middle-and-top (no changes)
┊│
┊├┄ mi [middle]
┊●   l#0 on middle (no changes)
┊│
┊├┄ bo [bottom]
┊●   l#1 on bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 36d9d21 (HEAD -> top) on top
* 0daea9a (between-middle-and-top) on between-middle-and-top
* 2596ebb (middle) on middle
* ff665ad (bottom) on bottom
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn handles_path_prefix_collision() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // As ref A already exists, A/new collides with A due to the need to create a directory called A
    env.but("branch new A/new/branch")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch name 'A/new/branch' collides with existing branch 'A'

"#]]);
}

#[test]
fn creates_new_branches_on_top() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new").assert().success().stdout_eq(str![[r#"
Created branch 'a-branch-1'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new one")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'one'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┊╭┄ br [a-branch-1] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_above_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new bottom").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new top --above bottom")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'top' above branch 'bottom'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new middle --above bottom")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'middle' above branch 'bottom'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_above_non_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -b bottom --no-message").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ bo [bottom]
┊●   tqv (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new top --above bottom")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'top' above branch 'bottom'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ bo [bottom]
┊●   tqv (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_below_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new top").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new bottom --below top")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'bottom' below branch 'top'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new middle --below top")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'middle' below branch 'top'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_below_non_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -b top --no-message").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   tqv (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new bottom --below top")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'bottom' below branch 'top'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   tqv (no commit message) (no changes)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -b my-branch -m bottom").assert().success();
    env.but("commit -b my-branch -m middle").assert().success();
    env.but("commit -b my-branch -m top").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch]
┊●   zou top (no changes)
┊●   uxw middle (no changes)
┊●   tqv bottom (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new --above uxw")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'a-branch-1' above commit uxw

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch]
┊●   zou top (no changes)
┊│
┊├┄ br [a-branch-1]
┊●   uxw middle (no changes)
┊●   tqv bottom (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new --above zou")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'a-branch-2' above commit zou

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch] (no commits)
┊│
┊├┄ br [a-branch-2]
┊●   zou top (no changes)
┊│
┊├┄ ra [a-branch-1]
┊●   uxw middle (no changes)
┊●   tqv bottom (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_branch_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -b my-branch -m bottom").assert().success();
    env.but("commit -b my-branch -m middle").assert().success();
    env.but("commit -b my-branch -m top").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch]
┊●   zou top (no changes)
┊●   uxw middle (no changes)
┊●   tqv bottom (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new --below uxw")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'a-branch-1' below commit uxw

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch]
┊●   zou top (no changes)
┊●   uxw middle (no changes)
┊│
┊├┄ br [a-branch-1]
┊●   tqv bottom (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch new --below tqv")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'a-branch-2' below commit tqv

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ my [my-branch]
┊●   zou top (no changes)
┊●   uxw middle (no changes)
┊│
┊├┄ br [a-branch-1]
┊●   tqv bottom (no changes)
┊│
┊├┄ ra [a-branch-2] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_create_new_branches_above_merged_branches_but_not_below() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-merged-empty-branch");

    env.but("apply origin/document-but-pr-skill")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success();

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ do [document-but-pr-skill] (merged upstream) (no commits)
├╯
┊
┊● 55165db (upstream: origin/main) 1 new commit
├╯ 55165db (common base) 2000-01-02 merge document-but-pr-skill

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("branch new --above do")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'a-branch-1' above branch 'document-but-pr-skill'

"#]]);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
┊│
┊├┄ do [document-but-pr-skill] (merged upstream) (no commits)
├╯
┊
┊● 55165db (upstream: origin/main) 1 new commit
├╯ 55165db (common base) 2000-01-02 merge document-but-pr-skill

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("branch new --below do")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch 'document-but-pr-skill' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn cannot_create_branches_below_branches_merged_upstream() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-single-stack");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (merged upstream)
┊●   nyq A-change
├╯
┊
┊● 9354ac4 (upstream: origin/main) 1 new commit
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("branch new --below nyq")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn create_branch_using_old_anchor_flag() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("branch new bottom").assert().success();
    env.but("branch new middle --anchor bottom")
        .assert()
        .success();
    env.but("branch new top -a middle").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn in_single_branch_mode_creating_new_independent_branch_takes_you_to_workspace_mode() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    // at first we're not on a workspace
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> main, origin/main, origin/HEAD) M
* e31e6ca add init

"#]]
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    // creating a new branch just puts us on that branch
    env.but("branch new one")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created branch 'one'

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> one, origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    // creating a second branch puts us into a workspace with both branches applied
    env.but("branch new two")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'two'

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 8ad759d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* b1540e5 (origin/main, origin/HEAD, two, one, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ tw [two] (no commits)
├╯
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    // switching to a branch removes the workspace and checks out the branch
    env.but("switch one").assert().success();

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 8ad759d (gitbutler/workspace) GitButler Workspace Commit
|/
* b1540e5 (HEAD -> one, origin/main, origin/HEAD, two, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    // creating a new branch puts us back on a workspace with the previous and new branches applied
    env.but("branch new three")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created branch 'three'

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 9e991f4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* b1540e5 (origin/main, origin/HEAD, two, three, one, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ th [three] (no commits)
├╯
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn in_single_branch_mode_switching_to_stacked_branches_works() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new bottom").assert().success();

    env.but("branch new middle --above bottom")
        .assert()
        .success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> middle, origin/main, origin/HEAD, main, gitbutler/target, bottom) M
* e31e6ca add init

"#]]
    );

    env.but("switch bottom").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> bottom, origin/main, origin/HEAD, middle, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("branch new new-branch").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch] (no commits)
├╯
┊
┊╭┄ bo [bottom] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 10e74ab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* b1540e5 (origin/main, origin/HEAD, new-branch, middle, main, gitbutler/target, bottom) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn in_single_branch_mode_switching_to_stacked_branches_with_commits_works() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new bottom").assert().success();

    env.but("commit -m 'on bottom' -b bottom")
        .assert()
        .success();

    env.but("branch new middle --above bottom")
        .assert()
        .success();

    env.but("commit -m 'on middle' -b middle")
        .assert()
        .success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ mi [middle]
┊●   ylm on middle (no changes)
┊│
┊├┄ bo [bottom]
┊●   lsm on bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* c5ca33e (HEAD -> middle) on middle
* ff665ad (bottom) on bottom
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("switch bottom").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ bo [bottom]
┊●   lsm on bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* c5ca33e (middle) on middle
* ff665ad (HEAD -> bottom) on bottom
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("branch new new-branch").assert().success();

    env.but("commit -m 'on new-branch' -b new-branch")
        .assert()
        .success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch]
┊●   l#0 on new-branch (no changes)
├╯
┊
┊╭┄ bo [bottom]
┊●   l#1 on bottom (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   4fe5ec0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 1c8b0db (new-branch) on new-branch
| | * c5ca33e (middle) on middle
| |/  
| * ff665ad (bottom) on bottom
|/  
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn in_single_branch_mode_creating_and_switching_to_new_branches() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new one").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> one, origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("branch new two --switch").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ tw [two] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* b1540e5 (HEAD -> two, origin/main, origin/HEAD, one, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn in_single_branch_mode_creating_and_switching_to_new_branches_with_commits() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");

    env.but("branch new one").assert().success();
    env.but("commit -b one -m 'on one'").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ on [one]
┊●   lsm on one (no changes)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 26559e2 (HEAD -> one) on one
* b1540e5 (origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );

    env.but("branch new two --switch").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ tw [two] (no commits)
├╯
┊
┴ b1540e5 (common base) 2000-01-02 M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 26559e2 (one) on one
* b1540e5 (HEAD -> two, origin/main, origin/HEAD, main, gitbutler/target) M
* e31e6ca add init

"#]]
    );
}

#[test]
fn in_workspace_mode_creating_and_switching_to_new_branches() {
    let env = Sandbox::open_with_default_settings("two-stacks");

    env.but("branch new --switch").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*-.   6afce52 (gitbutler/workspace) GitButler Workspace Commit
|/ /  
| | * 9477ae7 (A) add A
| |/  
* / d3e2ba3 (B) add B
|/  
* 0dc3733 (HEAD -> a-branch-1, origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    env.but("branch new --switch").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-2] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*-.   6afce52 (gitbutler/workspace) GitButler Workspace Commit
|/ /  
| | * 9477ae7 (A) add A
| |/  
* / d3e2ba3 (B) add B
|/  
* 0dc3733 (HEAD -> a-branch-2, origin/main, origin/HEAD, main, gitbutler/target, a-branch-1) add M

"#]]
    );
}
