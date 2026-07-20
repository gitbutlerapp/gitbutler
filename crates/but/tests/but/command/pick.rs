use snapbox::str;

use crate::utils::{CommandExt as _, Sandbox};

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
      "newCommitId": "b40d58bcb23bf959c85cef47249d7d263a2e9b0c",
      "newChangeId": "1"
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
      "newCommitId": "f033235315bbeb928633d5cad1926d91bf2b9dfb",
      "newChangeId": "1"
    },
    {
      "sourceCommitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
      "newCommitId": "10d0f0680d5ef69031deb2e94ba05e934d59b7c0",
      "newChangeId": "1"
    }
  ],
  "branch": "new-branch"
}

"#]]);
}

#[test]
fn pick_commit_to_new_branch_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
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
      "newCommitId": "f033235315bbeb928633d5cad1926d91bf2b9dfb",
      "newChangeId": "1"
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
Picked d3e2ba3 onto branch 'A' to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha b40d58b)
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
    env.setup_metadata(&["A"]);

    env.but("unapply A").assert().success();

    env.but("pick 9477ae7 -b new-branch")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked 9477ae7 onto new branch 'new-branch' to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ne [new-branch]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha f033235)
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
Picked d3e2ba3 to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha b40d58b)
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
Picked d3e2ba3 to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha c341b3d)
┊│     add A 
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 2174f2b)
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
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --above A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'a-branch-1' to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha b40d58b)
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
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("pick d3e2ba3 --below A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Picked d3e2ba3 onto new branch 'a-branch-1' to create 1

"#]]);

    env.but("status -v").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha c341b3d)
┊│     add A 
┊│
┊├┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 2174f2b)
┊│     add B 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
