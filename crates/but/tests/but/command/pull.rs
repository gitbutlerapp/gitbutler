use std::ops::DerefMut;

use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    ref_metadata::{StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch},
};
use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn pull_prunes_integrated_stack_and_keeps_remaining_stack_parent() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-one-of-two-stacks-integrated",
    );
    env.setup_metadata_at_target(&["A", "B"], "origin/main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄h0 [B]
┊●   d3e2ba3 add B
├╯
┊
┊● 26ecc90 (upstream) ⏫ 2 commits
├╯ 26ecc90 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [B]
┊◐   5542cc3 add B
├╯
┊
┴ 26ecc90 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    assert!(
        !git_ref_exists(&env, "refs/heads/A")?,
        "the branch already integrated into the target should be removed"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/B")?,
        "the remaining stack should stay in the workspace"
    );

    let workspace_parents = rev_parse_all(&env, "gitbutler/workspace^@")?;
    assert_eq!(
        workspace_parents.len(),
        1,
        "the workspace should have exactly the remaining stack as parent"
    );
    assert_eq!(
        workspace_parents[0],
        rev_parse(&env, "B")?,
        "the remaining stack should remain the workspace parent"
    );
    assert_ne!(
        workspace_parents[0],
        rev_parse(&env, "origin/main")?,
        "the workspace should not be reparented directly to the target while a stack remains"
    );
    assert_eq!(
        status_stack_count(&env)?,
        1,
        "exactly the remaining stack should stay applied"
    );

    Ok(())
}

#[test]
fn pull_prunes_integrated_branch_from_partial_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    setup_single_stack_metadata_at_target(&env, &["A", "C"], "refs/heads/base")?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   99cf923 add A
┊│
┊├┄h0 [C] (merged upstream)
┊●   e5378e0 add C
├╯
┊
┊● d4cb681 (upstream) ⏫ 2 commits
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   3a110f4 add A
├╯
┊
┴ d4cb681 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    assert!(
        git_ref_exists(&env, "refs/heads/A")?,
        "the remaining top branch should stay in the workspace"
    );
    assert_eq!(
        status_branch_names(&env)?,
        vec!["A"],
        "workspace status should contain only the rebased top branch after pruning the integrated lower branch"
    );
    assert_eq!(
        status_stack_count(&env)?,
        1,
        "the partially integrated stack should remain applied through its top branch"
    );

    Ok(())
}

#[test]
fn pull_keeps_empty_branch_above_merged_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "upstream-merged-branch-below-empty-branch",
    );
    setup_single_stack_metadata_at_target(&env, &["top", "bottom"], "refs/heads/main")?;
    env.invoke_git("remote set-url origin .");

    env.but("pull").assert().success();

    assert_eq!(
        status_branch_names(&env)?,
        vec!["top"],
        "pull should prune only the genuinely merged lower branch and preserve the empty top branch"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/top")?,
        "the empty top branch was not merged itself and must survive"
    );
    assert!(
        !git_ref_exists(&env, "refs/heads/bottom")?,
        "the lower branch landed upstream and should be removed"
    );

    Ok(())
}

#[test]
fn pull_check_uses_workspace_dry_run_for_partial_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    setup_single_stack_metadata_at_target(&env, &["A", "C"], "refs/heads/base")?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   99cf923 add A
┊│
┊├┄h0 [C] (merged upstream)
┊●   e5378e0 add C
├╯
┊
┊● d4cb681 (upstream) ⏫ 2 commits
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull --check")
        .assert()
        .success()
        .stdout_eq(str![[r#"

Base branch:	origin/main
Upstream:	2 new commits on origin/main

  d4cb681 add upstream[..]
  a4cc6be merge C[..]

Branch Status
  [ok] A
  [integrated] C

Run `but pull` to update your branches

"#]]);

    assert!(
        git_ref_exists(&env, "refs/heads/C")?,
        "dry-run check should not remove the integrated lower branch"
    );
    assert_eq!(
        status_branch_names(&env)?,
        vec!["A", "C"],
        "dry-run check should leave both stack branches in workspace status"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   99cf923 add A
┊│
┊├┄h0 [C] (merged upstream)
┊●   e5378e0 add C
├╯
┊
┊● d4cb681 (upstream) ⏫ 2 commits (checked [..])
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    Ok(())
}

#[test]
fn pull_check_reports_conflicted_branches_as_rebasable() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-conflicted");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");
    env.invoke_git("remote set-url origin .");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9283fc1 A-change
├╯
┊
┊● bdfcf28 (upstream) ⏫ 1 commit
├╯ efc9211 (common base) 2000-01-02 base

Hint: run `but help` for all commands

"#]]);

    let output = env
        .but("--format json pull --check")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output: serde_json::Value = serde_json::from_slice(&output)?;
    let branch_status = output["branchStatuses"]
        .as_array()
        .and_then(|statuses| statuses.iter().find(|status| status["name"] == "A"))
        .expect("pull check should report branch A status");

    assert_eq!(
        branch_status["status"], "conflicted",
        "conflicted dry-run branch should be reported as conflicted"
    );
    assert_eq!(
        branch_status["rebasable"], true,
        "conflicted dry-run branch should remain rebasable"
    );

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   6e0f28b A-change (no changes) {conflicted}
├╯
┊
┴ bdfcf28 (common base) 2000-01-02 main-change

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn pull_reparents_workspace_to_target_after_all_stacks_integrate() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pull-two-integrated-stacks");
    env.setup_metadata_at_target(&["A", "B"], "origin/main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (no commits)
├╯
┊
┊╭┄h0 [B] (no commits)
├╯
┊
┊● 7e5d4e1 (upstream) ⏫ 3 commits
├╯ 7e5d4e1 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┴ 7e5d4e1 (common base) 2000-01-02 add upstream

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_eq!(
        rev_parse(&env, "gitbutler/workspace^")?,
        rev_parse(&env, "origin/main")?,
        "once all stacks are integrated, the workspace should be parented to the advanced target"
    );
    assert_eq!(
        status_stack_count(&env)?,
        0,
        "no stacks should remain applied once both are integrated"
    );

    Ok(())
}

#[test]
fn pull_reparents_empty_workspace_when_target_advances() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata_at_target(&[], "origin/main");
    env.invoke_git("remote set-url origin .");

    env.invoke_git("checkout main");
    env.file("upstream.txt", "upstream\n");
    env.invoke_git("add upstream.txt");
    env.invoke_git("commit -m upstream-change");
    env.invoke_git("checkout gitbutler/workspace");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┴ 526bb83 (common base) 2000-01-02 upstream-change

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_eq!(
        rev_parse(&env, "gitbutler/workspace^")?,
        rev_parse(&env, "origin/main")?,
        "an empty workspace should still move forward when the target advances"
    );

    Ok(())
}

#[test]
fn pull_does_not_report_branch_rebase_conflicts_as_worktree_conflicts() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");

    env.file("shared.txt", "local\nextra local work\n");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   ot M shared.txt
┊
┊╭┄g0 [A]
┊●   ba99744 local change
├╯
┊
┊● 7f73771 (upstream) ⏫ 1 commit
├╯ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    let output = env.but("pull").output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Rebase resulted in some conflicts"),
        "pull should proceed to the branch conflict workflow instead of stopping at the worktree gate; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("uncommitted changes in the worktree"),
        "branch rebase conflicts should not be reported as dirty worktree conflicts; stdout:\n{stdout}"
    );
    assert_eq!(
        status_branch_names(&env)?,
        vec!["A"],
        "conflicted branch should remain in the workspace after pull"
    );
    assert!(
        branch_has_conflicted_commit(&env, "A")?,
        "pull should materialize the rebase conflict on a commit inside branch A"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   ot M shared.txt
┊
┊╭┄g0 [A]
┊◐   705b03e local change (no changes) {conflicted}
├╯
┊
┴ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but diff` to see uncommitted changes and `but stage <file>` to stage them to a branch

"#]]);

    Ok(())
}

#[test]
fn pull_json_reports_branch_rebase_conflicts_as_successful_integration() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   ba99744 local change
├╯
┊
┊● 7f73771 (upstream) ⏫ 1 commit
├╯ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    let output = env
        .but("--format json pull")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output: serde_json::Value = serde_json::from_slice(&output)?;

    assert_eq!(
        output["status"], "completed_with_conflicts",
        "branch rebase conflicts should complete pull with conflicts instead of blocking integration"
    );
    assert_eq!(
        output["summary"]["branchesConflicted"], 1,
        "pull summary should count the branch that now contains a conflicted commit"
    );
    assert_eq!(
        output["conflicts"][0]["branch"], "A",
        "pull JSON should identify the branch that needs conflict resolution"
    );
    assert!(
        branch_has_conflicted_commit(&env, "A")?,
        "pull should leave the conflicted commit visible in branch status"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   705b03e local change (no changes) {conflicted}
├╯
┊
┴ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn pull_reports_conflict_in_lower_branch_of_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflict-in-lower-branch-of-stack",
    );
    setup_single_stack_metadata_at_target(&env, &["A", "B"], "main")?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   [..] top change
┊│
┊├┄h0 [B]
┊●   [..] bottom change
├╯
┊
┊● 7f73771 (upstream) ⏫ 1 commit
├╯ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    env.but("pull").assert().success().stdout_eq(str![[r#"

Found 1 upstream commits on origin/main
   [..] upstream change

Updating 2 active branches...

Rebase resulted in some conflicts

Summary
────────
  A - rebased
  B - conflicted

To resolve conflicts:
  1. Run `but status` to see conflicted commits
  2. Run `but resolve <commit>` to enter resolution mode on any conflicted commit
  3. Edit files to resolve the conflicts
  4. Run `but resolve finish` to finalize the resolution

To undo this operation:
  Run `but undo`

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   4a2dd6a top change
┊│
┊├┄h0 [B]
┊◐   d5fa75c bottom change (no changes) {conflicted}
├╯
┊
┴ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn pull_reports_conflicts_in_multiple_branches_of_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    setup_single_stack_metadata_at_target(&env, &["A", "B"], "main")?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   [..] top change
┊│
┊├┄h0 [B]
┊●   [..] bottom change
├╯
┊
┊● [..] (upstream) ⏫ 1 commit
├╯ [..] (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    env.but("pull").assert().success().stdout_eq(str![[r#"

Found 1 upstream commits on origin/main
   [..] upstream change

Updating 2 active branches...

Rebase resulted in some conflicts

Summary
────────
  A - conflicted
  B - conflicted

To resolve conflicts:
  1. Run `but status` to see conflicted commits
  2. Run `but resolve <commit>` to enter resolution mode on any conflicted commit
  3. Edit files to resolve the conflicts
  4. Run `but resolve finish` to finalize the resolution

To undo this operation:
  Run `but undo`

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   09bcfd9 top change (no changes) {conflicted}
┊│
┊├┄h0 [B]
┊◐   7ac1f5a bottom change (no changes) {conflicted}
├╯
┊
┴ e4933d8 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
#[ignore = "documents bug 2: but pull still silently writes conflict markers into uncommitted files in this scenario"]
fn pull_reports_uncommitted_conflicts_instead_of_silently_writing_markers() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");
    disable_cv3_for_legacy_uncommitted_conflict_materialization(&env)?;

    env.file("shared.txt", "local\nextra local work\n");

    env.but("pull").assert().failure().stdout_eq(str![[r#"

Found 1 upstream commits on origin/main
   [..] upstream change

There are uncommitted changes in the worktree that conflict with the updates:
  shared.txt
Please commit or stash them and try again.

"#]]);

    let shared = std::fs::read_to_string(env.projects_root().join("shared.txt"))?;
    assert!(
        !shared.contains("<<<<<<<"),
        "pull must not write conflict markers into uncommitted files without reporting them"
    );

    Ok(())
}

fn setup_single_stack_metadata_at_target(
    env: &Sandbox,
    branch_names: &[&str],
    target_spec: &str,
) -> anyhow::Result<()> {
    let mut meta = env.meta();
    let mut ws = meta.workspace(r(WORKSPACE_REF_NAME))?;
    let repo = env.open_repo();
    let ws_data = ws.deref_mut();
    ws_data.stacks = vec![WorkspaceStack {
        id: StackId::from_number_for_testing(0),
        branches: branch_names
            .iter()
            .map(|branch_name| WorkspaceStackBranch {
                ref_name: r(&format!("refs/heads/{branch_name}")).to_owned(),
                archived: false,
            })
            .collect(),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    }];
    let mut project_meta = ws.project_meta();
    project_meta.target_commit_id = Some(repo.rev_parse_single(target_spec)?.detach());
    ws.set_project_meta(project_meta);
    let project_meta = ws.project_meta();
    meta.set_workspace(&ws)?;
    project_meta.persist_to_local_config(&repo)?;
    Ok(())
}

fn disable_cv3_for_legacy_uncommitted_conflict_materialization(
    env: &Sandbox,
) -> anyhow::Result<()> {
    let settings_path = env.app_data_dir().join("gitbutler/settings.json");
    let mut settings: serde_json::Value = serde_json::from_slice(&std::fs::read(&settings_path)?)?;
    settings["featureFlags"]["cv3"] = serde_json::json!(false);
    std::fs::write(settings_path, serde_json::to_vec_pretty(&settings)?)?;
    Ok(())
}

fn git_ref_exists(env: &Sandbox, ref_name: &str) -> anyhow::Result<bool> {
    Ok(std::process::Command::new("git")
        .args(["show-ref", "--verify", "--quiet", ref_name])
        .current_dir(env.projects_root())
        .status()?
        .success())
}

fn rev_parse(env: &Sandbox, spec: &str) -> anyhow::Result<String> {
    let values = rev_parse_all(env, spec)?;
    let [value] = values.as_slice() else {
        anyhow::bail!("expected exactly one rev for {spec}, got {values:?}");
    };
    Ok(value.clone())
}

fn rev_parse_all(env: &Sandbox, spec: &str) -> anyhow::Result<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", spec])
        .current_dir(env.projects_root())
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .map(str::to_owned)
        .collect())
}

fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}

fn status_stack_count(env: &Sandbox) -> anyhow::Result<usize> {
    let status = status_json(env)?;
    Ok(status["stacks"].as_array().map_or(0, Vec::len))
}

fn status_branch_names(env: &Sandbox) -> anyhow::Result<Vec<String>> {
    let status = status_json(env)?;
    Ok(status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .filter_map(|branch| branch["name"].as_str().map(str::to_owned))
        .collect())
}

fn branch_has_conflicted_commit(env: &Sandbox, branch_name: &str) -> anyhow::Result<bool> {
    let status = status_json(env)?;
    Ok(status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .filter(|branch| branch["name"].as_str() == Some(branch_name))
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .any(|commit| commit["conflicted"].as_bool() == Some(true)))
}

fn status_json(env: &Sandbox) -> anyhow::Result<serde_json::Value> {
    let output = env
        .but("status --format json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    Ok(serde_json::from_slice(&output)?)
}
