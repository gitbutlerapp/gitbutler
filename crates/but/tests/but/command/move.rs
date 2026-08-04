use crate::{
    command::util::{
        branch_commit_cli_ids, commit_two_files_as_two_hunks_each,
        status_json_with_files as status_json,
    },
    utils::{CommandExt as _, Sandbox},
};

#[test]
fn move_commits_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("--json move zll --above ywx")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "commits",
  "commits": [
    {
      "sourceCommitId": "fe12bcd55e12fe5d43e54f44550d4c201f0ec770",
      "sourceChangeId": "zllwszkrzvwxozppxxkxpsnxopskvrsp",
      "newCommitId": "c6224e6e0af1ac247027c8f61ed6ef4037c2c230",
      "newChangeId": "zllwszkrzvwxozppxxkxpsnxopskvrsp"
    }
  ]
}

"#]]);
}

#[test]
fn move_committed_changes_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("--json move ywx:wu --branch new-branch")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "changes",
  "sourceCommitId": "9ac4652535fde457cb4cb3b36f0d9a64135de4c8",
  "sourceChangeId": "ywxsopnrxtuqozktnmnmwxmwlpxsokpn",
  "numChanges": 1,
  "newCommitId": "8e35f84e6f99cf09d1fa04c8df71d98b954865c5",
  "newChangeId": "1",
  "branch": "new-branch"
}

"#]]);
}

#[test]
fn stack_branch_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("--json move B --above A")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "stackBranch",
  "sourceBranch": "B",
  "targetBranch": "A"
}

"#]]);
}

#[test]
fn unstack_branch_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("--json move C --unstack")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "unstackBranch",
  "sourceBranch": "C"
}

"#]]);
}

#[test]
fn move_commit_above_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll --above ywx")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved zll above commit ywx

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   zll add first
┊●   ywx add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_below_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --below zll")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx below commit zll

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   zll add first
┊●   ywx add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_multiple_consecutive_commits_relative_to_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   usn add A13
┊●   opy add A12
┊●   opk add A11
┊●   vvl add A10
┊●   mzz add A9
┊●   vmw add A8
┊●   tpw add A7
┊●   lyq add A6
┊●   pyq add A5
┊●   mvv add A4
┊●   tvm add A3
┊●   sxq add A2
┊●   zpl add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    for (operator, target_cli_id) in [("--above", "lyq"), ("--below", "tpw")] {
        env.but("move vvl mzz")
            .arg(operator)
            .arg(target_cli_id)
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Moved vvl, mzz [..] commit [..]

"#]]);

        env.but("status")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   usn add A13
┊●   opy add A12
┊●   opk add A11
┊●   vmw add A8
┊●   tpw add A7
┊●   vvl add A10
┊●   mzz add A9
┊●   lyq add A6
┊●   pyq add A5
┊●   mvv add A4
┊●   tvm add A3
┊●   sxq add A2
┊●   zpl add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

        env.but("undo").assert().success();
    }
}

#[test]
fn move_multiple_non_consecutive_commits_in_arbitrary_order_relative_to_other_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   usn add A13
┊●   opy add A12
┊●   opk add A11
┊●   vvl add A10
┊●   mzz add A9
┊●   vmw add A8
┊●   tpw add A7
┊●   lyq add A6
┊●   pyq add A5
┊●   mvv add A4
┊●   tvm add A3
┊●   sxq add A2
┊●   zpl add A1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    for (operator, target_cli_id) in [("--above", "vmw"), ("--below", "mzz")] {
        // We pick the source commits in an "incorrect" order, but they should later be sorted correctly
        // via topological sort.
        //
        // Order as picked is: A7 A1 A5 --above A8, but we expect the commits to be applied from oldest
        // to newest, i.e. (A8) <- A1 <- A5 <- A7
        env.but("move tpw zpl pyq")
            .arg(operator)
            .arg(target_cli_id)
            .assert()
            .success()
            .stdout_eq(snapbox::str![["
Moved tpw, zpl, pyq [..] commit [..]

"]]);

        env.but("status")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   usn add A13
┊●   opy add A12
┊●   opk add A11
┊●   vvl add A10
┊●   mzz add A9
┊●   tpw add A7
┊●   pyq add A5
┊●   zpl add A1
┊●   vmw add A8
┊●   lyq add A6
┊●   mvv add A4
┊●   tvm add A3
┊●   sxq add A2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

        env.but("undo").assert().success();
    }
}

#[test]
fn moving_commits_above_branch_creates_branch_above() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved zll to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   zll add first
┊│
┊├┄ g0 [A]
┊●   ywx add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_above_branch_without_changing_relative_order_only_creates_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   ywx add second
┊│
┊├┄ g0 [A]
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_below_branch_creates_branch_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   zll add first
┊│
┊├┄ br [a-branch-1]
┊●   ywx add second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_commits_below_branch_without_changing_relative_order_only_creates_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved zll to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│
┊├┄ br [a-branch-1]
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_all_commits_above_branch_retains_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx zll --above g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx, zll to new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   ywx add second
┊●   zll add first
┊│
┊├┄ g0 [A] (no commits)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn moving_all_commits_below_branch_retains_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx zll --below g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx, zll to new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_above_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm --above h0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved tpm to new branch 'a-branch-1' above branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ br [a-branch-1]
┊●   tpm add A
┊│
┊├┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_commit_below_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm --below h0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved tpm to new branch 'a-branch-1' below branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ h0 [B] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn above_or_below_unapplied_or_non_existing_branch_errors() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("unapply B").assert().success();

    env.but("move tpm --above B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'B'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("move tpm --below B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'B'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("move tpm --above no-such-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'no-such-branch'

Hint: Run `but status` for applicable targets.

"#]]);

    env.but("move tpm --below no-such-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find anchor: 'no-such-branch'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn move_to_tip_of_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
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

    env.but("move tpm -b B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved tpm to the tip of branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ h0 [B]
┊●   tpm add A
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm -b B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved tpm to the tip of branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ h0 [B]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_new_unstacked_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --branch new-branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx to new branch 'new-branch'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch]
┊●   ywx add second
├╯
┊
┊╭┄ g0 [A]
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_to_tip_of_new_unstacked_branch_with_canned_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved ywx to new branch 'a-branch-1'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   ywx add second
├╯
┊
┊╭┄ g0 [A]
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_below_commit_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx:wu --below zll")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from ywx to new commit 1 below commit zll

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second (no changes)
┊●   zll add first
┊│     zll:l A first
┊●   1 (no commit message)
┊│     1:w A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_above_commit_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll:lz --above ywx")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from zll to new commit 1 above commit ywx

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊│     1:l A first
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_below_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx:wu --below A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from ywx to new commit 1 on new branch 'a-branch-1' below branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second (no changes)
┊●   zll add first
┊│     zll:l A first
┊│
┊├┄ br [a-branch-1]
┊●   1 (no commit message)
┊│     1:w A second
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_above_branch_creates_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll:lz --above A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from zll to new commit 1 on new branch 'a-branch-1' above branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message)
┊│     1:l A first
┊│
┊├┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_to_branch_tip_creates_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move lrm:pl --branch A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from lrm to new commit 1 to the tip of branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊│     1:p A B
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_to_non_existing_branch_tip_creates_unstacked_branch_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx:wu --branch new-branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from ywx to new commit 1 on new branch 'new-branch'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch]
┊●   1 (no commit message)
┊│     1:w A second
├╯
┊
┊╭┄ g0 [A]
┊●   ywx add second (no changes)
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_branch_without_argument_creates_unstacked_branch_with_canned_name_and_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx:wu --branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from ywx to new commit 1 on new branch 'a-branch-1'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message)
┊│     1:w A second
├╯
┊
┊╭┄ g0 [A]
┊●   ywx add second (no changes)
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_should_be_order_independent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("new", "Some data");
    env.but("commit -m 'Add new file'").assert().success();
    std::fs::rename(
        env.projects_root().join("new"),
        env.projects_root().join("moved"),
    )
    .unwrap();
    env.file("new/file", "Stuff");
    env.file("unrelated", "This should stay here :)");
    env.but("commit -m 'Prepare for moves!'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 Prepare for moves!
┊│     1#0:u R moved
┊│     1#0:p A new/file
┊│     1#0:t A unrelated
┊●   1#1 Add new file
┊│     1#1:n A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move 1#0:u 1#0:p --above 1#0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 2 changes from 1 to new commit 1 above commit 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 (no commit message)
┊│     1#0:u R moved
┊│     1#0:p A new/file
┊●   1#1 Prepare for moves!
┊│     1#1:t A unrelated
┊●   1#2 Add new file
┊│     1#2:n A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("move 1#0:p 1#0:u --above 1#0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 2 changes from 1 to new commit 1 above commit 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 (no commit message)
┊│     1#0:u R moved
┊│     1#0:p A new/file
┊●   1#1 Prepare for moves!
┊│     1#1:t A unrelated
┊●   1#2 Add new file
┊│     1#2:n A new
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_file_from_multiple_source_commits_is_not_allowed() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm:tm lrm:pl -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot move changes from multiple commits

Hint: Move changes from a single commit at first, then squash additional changes into the new commit

"#]]);
}

#[test]
fn move_branch_above_within_same_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move B --above C")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Stacked branch 'B' on top of branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [C]
┊●   wlx add C
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
#[ignore = "We can't move branches below other branches right now :( https://linear.app/gitbutler/issue/GB-1735/support-all-permutations-of-moving-branches-and-commits"]
fn move_branch_below_within_same_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [C]
┊●   aebb090 add C
┊│
┊├┄h0 [B]
┊●   582f37b add B
┊│
┊├┄i0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move C --below B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved branch 'C' below branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [B]
┊●   223f14d add B
┊│
┊├┄h0 [C]
┊●   983f317 add C
┊│
┊├┄i0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_branch_above_to_other_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
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

    env.but("move B --above A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Stacked branch 'B' on top of branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   lrm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn move_empty_branch_above_other_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move B --above A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Stacked branch 'B' on top of branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B] (no commits)
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
#[ignore = "We can't move branches below other branches right now :( https://linear.app/gitbutler/issue/GB-1735/support-all-permutations-of-moving-branches-and-commits"]
fn move_branch_below_to_other_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move A --below B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved branch 'A' below branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [B]
┊●   e776549 add B
┊│
┊├┄h0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_tip_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move C --unstack")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstacked branch 'C'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
┊│
┊├┄ h0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ i0 [C]
┊●   wlx add C
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_middle_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move B --unstack")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstacked branch 'B'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   wwm add B
├╯
┊
┊╭┄ h0 [C]
┊●   wlx add C
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_bottom_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move A --unstack")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstacked branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [C]
┊●   wlx add C
┊│
┊├┄ i0 [B]
┊●   wwm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new bottom").assert().success();
    env.but("branch new -a bottom top").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ bo [bottom] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move --unstack top")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstacked branch 'top'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ bo [bottom] (no commits)
├╯
┊
┊╭┄ to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_branch_using_branch_arg() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // `--branch` used synonumously with `--unstack`
    env.but("move A --branch")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unstacked branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [C]
┊●   wlx add C
┊│
┊├┄ i0 [B]
┊●   wwm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unstack_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll --unstack")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved zll to new branch 'a-branch-1'

"#]]);
}

#[test]
fn unstack_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊│     ywx:w A second
┊●   zll add first
┊│     zll:l A first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move zll:lz --unstack")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved 1 change from zll to new commit 1 on new branch 'a-branch-1'

"#]]);
}

#[test]
fn cannot_unstack_multiple_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("move A B --unstack")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Branches can only be moved one at a time

"#]]);
}

/// This is an API limitation and not a desirable behavior, but moving multiple branches at the same
/// time is so fringe that it's not worth investing time into right now.
#[test]
fn cannot_move_multiple_branches_at_once() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move A B --above C")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Branches can only be moved one at a time

"#]]);
}

#[test]
fn cannot_move_branch_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move C --below B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'B' for '--below'

Invalid target for branch source

Hint: Branches can only be moved with `--above <branch>` or `--branch <branch>` to stack or `--unstack` to unstack

"#]]);
}

#[test]
fn cannot_mix_sources() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm lrm:pl -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Mixing source types is not allowed

Hint: You can only move one kind of source (e.g. commits) at a time

"#]]);

    env.but("move lrm B --above A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Mixing source types is not allowed

Hint: You can only move one kind of source (e.g. commits) at a time

"#]]);

    env.but("move lrm:pl B --above A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<SOURCES>'

Mixing source types is not allowed

Hint: You can only move one kind of source (e.g. commits) at a time

"#]]);
}

#[test]
fn targeting_unapplied_branch_errors() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("unapply B").assert().success();

    env.but("move tpm --branch B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'B' exists but is not applied

"#]]);
}

#[test]
fn cannot_combine_targets() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("move ywx --below zll --above zll")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--below <BRANCH_OR_COMMIT>' cannot be used with '--above <BRANCH_OR_COMMIT>'

Usage: but move <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack> <SOURCES>...

For more information, try '--help'.

Examples:
  but move <child-branch> --above <parent-branch>   # stack a branch on top of another
  but move <commit> --below <other-commit>          # reorder commits
  but move <commit> --branch <branch>               # move a commit onto a branch
  but move <branch> --unstack                       # tear a branch off its stack

"#]]);
}

#[test]
fn must_specify_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("move dontcare")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack>

Usage: but move <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack> <SOURCES>...

For more information, try '--help'.

Examples:
  but move <child-branch> --above <parent-branch>   # stack a branch on top of another
  but move <commit> --below <other-commit>          # reorder commits
  but move <commit> --branch <branch>               # move a commit onto a branch
  but move <branch> --unstack                       # tear a branch off its stack

"#]]);
}

#[test]
fn must_specify_source() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("move -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <SOURCES>...

Usage: but move <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack> <SOURCES>...

For more information, try '--help'.

Examples:
  but move <child-branch> --above <parent-branch>   # stack a branch on top of another
  but move <commit> --below <other-commit>          # reorder commits
  but move <commit> --branch <branch>               # move a commit onto a branch
  but move <branch> --unstack                       # tear a branch off its stack

"#]]);
}

#[test]
fn source_cannot_be_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("move ywx --above ywx")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'ywx' for '--above'

Source cannot also be target

Hint: Trying to move items above 'ywx'? Remove 'ywx' from '<SOURCES>' and try again!

"#]]);

    env.but("move ywx --below ywx")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'ywx' for '--below'

Source cannot also be target

Hint: Trying to move items below 'ywx'? Remove 'ywx' from '<SOURCES>' and try again!

"#]]);

    env.but("move A --above A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'A' for '--above'

Source cannot also be target

"#]]);

    env.but("move A --branch A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'A' for '--branch'

Source cannot also be target

"#]]);
}

#[test]
fn cannot_move_from_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "some text");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   qs A file
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("move qs -b A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'qs' for '<SOURCES>'

Cannot pass uncommitted file or hunk as source

Hint: A source must be commit, committed file or branch

"#]]);
    env.but("move zz -b A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'zz' for '<SOURCES>'

Cannot pass uncommitted changes as source

Hint: A source must be commit, committed file or branch

"#]]);
}

#[test]
fn cannot_move_to_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move tpm --below zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected a commit or a branch, got uncommitted changes

"#]]);
}

#[test]
fn move_commit_to_branch_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env);
    let branch_a_commits_before = branch_commit_cli_ids(&before, "A");
    let source_cli_id = branch_a_commits_before[0].clone();
    let branch_b_count_before = branch_commit_cli_ids(&before, "B").len();

    env.but(format!("move {source_cli_id} --branch B"))
        .assert()
        .success();

    let after = status_json(&env);
    let branch_a_commits_after = branch_commit_cli_ids(&after, "A");
    let branch_b_commits_after = branch_commit_cli_ids(&after, "B");
    assert_eq!(
        branch_a_commits_after.len() + 1,
        branch_a_commits_before.len(),
        "moving one commit should decrease branch A's commit count by one"
    );
    assert_eq!(
        branch_b_commits_after.len(),
        branch_b_count_before + 1,
        "moving one commit should increase branch B's commit count by one"
    );
    assert!(
        !branch_a_commits_after.contains(&source_cli_id),
        "moved commit should no longer be present on branch A"
    );
    assert!(
        branch_b_commits_after.contains(&source_cli_id),
        "moved commit should be present on branch B"
    );
}

#[test]
fn move_multiple_commits_to_branch_tip_preserves_order() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a1.txt", "a2.txt", "first");
    commit_two_files_as_two_hunks_each(&env, "A", "a3.txt", "a4.txt", "second");

    let before = status_json(&env);
    let branch_a = branch_commit_cli_ids(&before, "A");
    let newer = branch_a[0].clone();
    let older = branch_a[1].clone();

    env.but(format!("move {older} {newer} -b B"))
        .assert()
        .success();

    let after = status_json(&env);
    let branch_b = branch_commit_cli_ids(&after, "B");
    assert_eq!(
        &branch_b[..2],
        &[newer, older],
        "moving a block to a branch tip should preserve its history order"
    );
}

#[test]
fn move_that_conflicts_warns_about_newly_conflicted_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-dependent-commits");
    env.setup_metadata(&["A"]);

    let status = status_json(&env);
    let commits = branch_commit_cli_ids(&status, "A");
    let (top, bottom) = (&commits[0], &commits[1]);

    // Swapping two commits that edit the same line leaves the rebased commit
    // conflicted; the command output must say so.
    env.but(format!("move {bottom} --above {top}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved [..] above commit [..]

⚠ This operation left a commit conflicted:
  ● [..] [conflict] set two
Resolve with but resolve, or back out with but undo.

"#]]);
}

#[test]
fn move_without_conflicts_prints_no_conflict_warning() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    // The commits touch different files, so the swap rebases cleanly.
    env.but("move zll --above ywx")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Moved [..] above commit [..]

"#]]);
}

#[test]
fn move_rejects_merged_upstream_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    // `nyq` is branch A's landed commit, `kyl` is branch B's live commit.
    // Landed history is rejected both as move source and as target branch.
    env.but("move nyq --branch B")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    env.but("move kyl --branch A")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn retired_syntax_gets_a_teaching_hint() {
    let env = Sandbox::empty();

    // The pre-revamp `but move <source> <target>` form placed the source
    // below the target; the hint suggests the flagged modern equivalent.
    env.but("move ab cd")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"

note: this invocation used retired `but move` syntax. The modern equivalent is:

    but move ab --below cd     if cd is a commit
    but move ab --branch cd    if cd is a branch
    but move ab --above cd     to stack a branch onto branch cd

See `but move --help` for details.
error: the following required arguments were not provided:
  <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack>

Usage: but move <--above <BRANCH_OR_COMMIT>|--below <BRANCH_OR_COMMIT>|--branch [<BRANCH>]|--unstack> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn move_onto_branch_with_dash_dash_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move B --branch A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Stacked branch 'B' on top of branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊●   lrm add B
┊│     lrm:p A B
┊│
┊├┄ h0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn cannot_move_onto_new_branch_with_dash_dash_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("move A --branch new-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'new-branch' not found

Hint: `--branch` can only move branches onto existing branches

"#]]);
}
