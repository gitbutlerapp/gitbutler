use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

fn single_branch_integration_scenario() -> Sandbox {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.but("config feature single-branch enable")
        .assert()
        .success();
    env.but("status").env("NO_BG_TASKS", "1").assert().success();
    let repo = env.open_repo();
    let mut project_meta = env.project_meta();
    project_meta.target_ref = Some("refs/remotes/origin/main".try_into().unwrap());
    project_meta.target_commit_id = Some(
        repo.rev_parse_single("refs/remotes/origin/main")
            .unwrap()
            .detach(),
    );
    project_meta.persist(&repo).unwrap();
    env.invoke_git(
        "config --replace-all remote.origin.fetch +refs/heads/main:refs/remotes/origin/main",
    );
    env.invoke_git("remote set-url origin .");
    env
}

fn commit_file(env: &Sandbox, branch: &str) {
    env.file(format!("{branch}.txt"), format!("{branch}\n"));
    env.but(format!("commit -b {branch} -m 'add {branch}'"))
        .assert()
        .success();
}

fn merge_into_upstream(env: &Sandbox, branch: &str, add_upstream_commit: bool) {
    let head = env.invoke_git("symbolic-ref --short HEAD");
    env.invoke_git("checkout main");
    env.invoke_git(&format!("merge --no-ff -m 'merge {branch}' {branch}"));
    if add_upstream_commit {
        env.file("upstream.txt", "upstream\n");
        env.invoke_git("add upstream.txt");
        env.invoke_git("commit -m 'add upstream'");
    }
    env.invoke_git(&format!("checkout {head}"));
    env.invoke_git("fetch origin");
}

#[test]
fn single_branch_pull_replaces_a_fully_integrated_checkout() {
    let env = single_branch_integration_scenario();
    env.but("branch new A").assert().success();
    commit_file(&env, "A");
    let old_head = rev_parse(&env, "A");
    merge_into_upstream(&env, "A", true);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (merged upstream)
┊●   1 add A
├╯
┊
┊● f12cbfa (upstream: origin/main) 2 new commits
├╯ 85efbe4 (common base) 2000-01-02 M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull --check")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"

Base branch:	origin/main
Upstream:	2 new commits on origin/main

  f12cbfa add upstream[..]
  b17b7d2 merge A[..]

Branch Status
  [integrated] A

Run `but pull` to update your branches

"#]]);
    assert_eq!(
        rev_parse(&env, "HEAD"),
        old_head,
        "pull --check is a dry run"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/A"),
        "pull --check must not remove the integrated branch"
    );

    env.but("pull").assert().success();

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
├╯
┊
┴ f12cbfa (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);
    assert!(
        !git_ref_exists(&env, "refs/heads/A"),
        "pull should remove the fully integrated branch"
    );
    assert_ne!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "A",
        "the removed checkout should be replaced"
    );
    assert_eq!(
        rev_parse(&env, "HEAD"),
        rev_parse(&env, "origin/main"),
        "the replacement checkout should point at the advanced target"
    );
    assert!(!git_ref_exists(&env, but_core::WORKSPACE_REF_NAME));
}

#[test]
fn single_branch_pull_prunes_an_integrated_lower_branch() {
    let env = single_branch_integration_scenario();
    env.but("branch new C").assert().success();
    commit_file(&env, "C");
    env.but("branch new A").assert().success();
    commit_file(&env, "A");
    let old_head = rev_parse(&env, "A");
    let old_lower = rev_parse(&env, "C");
    merge_into_upstream(&env, "C", true);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 add A
┊│
┊├┄ h0 [C] (merged upstream)
┊●   1#1 add C
├╯
┊
┊● 1a9cade (upstream: origin/main) 2 new commits
├╯ 85efbe4 (common base) 2000-01-02 M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull --check")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"

Base branch:	origin/main
Upstream:	2 new commits on origin/main

  1a9cade add upstream[..]
  c251e1d merge C[..]

Branch Status
  [ok] A
  [integrated] C

Run `but pull` to update your branches

"#]]);
    assert_eq!(rev_parse(&env, "A"), old_head, "pull --check is a dry run");
    assert_eq!(rev_parse(&env, "C"), old_lower, "pull --check is a dry run");

    env.but("pull").assert().success();

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add A
├╯
┊
┴ 1a9cade (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);
    assert!(
        !git_ref_exists(&env, "refs/heads/C"),
        "pull should remove the integrated lower branch"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "A",
        "the surviving top branch should stay checked out"
    );
    assert_eq!(
        rev_parse(&env, "A^"),
        rev_parse(&env, "origin/main"),
        "the surviving branch should be rebased onto the advanced target"
    );
    assert!(!git_ref_exists(&env, but_core::WORKSPACE_REF_NAME));
}

#[test]
fn single_branch_pull_keeps_an_empty_branch_above_an_integrated_branch() {
    let env = single_branch_integration_scenario();
    env.but("branch new bottom").assert().success();
    commit_file(&env, "bottom");
    env.but("branch new top").assert().success();
    let old_tip = rev_parse(&env, "top");
    merge_into_upstream(&env, "bottom", false);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ bo [bottom] (merged upstream)
┊●   1 add bottom
├╯
┊
┊● 9f8a4d4 (upstream: origin/main) 1 new commit
├╯ 85efbe4 (common base) 2000-01-02 M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull --check")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"

Base branch:	origin/main
Upstream:	1 new commits on origin/main

  9f8a4d4 merge bottom[..]

Branch Status
  [ok] top
  [integrated] bottom

Run `but pull` to update your branches

"#]]);
    assert_eq!(rev_parse(&env, "top"), old_tip, "pull --check is a dry run");
    assert_eq!(
        rev_parse(&env, "bottom"),
        old_tip,
        "pull --check must preserve the integrated lower branch"
    );

    env.but("pull").assert().success();

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
├╯
┊
┴ 9f8a4d4 (common base) 2000-01-02 merge bottom

Hint: run `but help` for all commands

"#]]);
    assert!(
        !git_ref_exists(&env, "refs/heads/bottom"),
        "pull should remove the integrated lower branch"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/top"),
        "the local-only empty top branch should survive"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "top",
        "the empty top branch should stay checked out"
    );
    assert_eq!(
        rev_parse(&env, "top"),
        rev_parse(&env, "origin/main"),
        "the empty top branch should advance to the target"
    );
    assert!(!git_ref_exists(&env, but_core::WORKSPACE_REF_NAME));
}

/// An unreachable remote that is not the target's must not block pulling: `fetch_from_remotes`
/// only fails when the target's own fetch remote failed, so a dead unrelated remote (old fork,
/// deleted mirror) is tolerated.
#[test]
fn pull_ignores_unreachable_unrelated_remote() {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches");
    env.but("setup").assert().success();
    env.invoke_git("remote add broken /nonexistent/path/broken.git");

    env.but("pull").assert().success();
}

#[test]
fn pull_prunes_integrated_stack_and_keeps_remaining_stack_parent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-one-of-two-stacks-integrated",
    );
    env.setup_metadata_at_target(&["A", "B"], "origin/main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┊● 26ecc90 (upstream: origin/main) 2 new commits
├╯ 26ecc90 (common base) 2000-01-02 add upstream

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [B]
┊◐   lrm add B
├╯
┊
┴ 26ecc90 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    assert!(
        !git_ref_exists(&env, "refs/heads/A"),
        "the branch already integrated into the target should be removed"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/B"),
        "the remaining stack should stay in the workspace"
    );

    let workspace_parents = rev_parse_all(&env, "gitbutler/workspace^@");
    assert_eq!(
        workspace_parents.len(),
        1,
        "the workspace should have exactly the remaining stack as parent"
    );
    assert_eq!(
        workspace_parents[0],
        rev_parse(&env, "B"),
        "the remaining stack should remain the workspace parent"
    );
    assert_ne!(
        workspace_parents[0],
        rev_parse(&env, "origin/main"),
        "the workspace should not be reparented directly to the target while a stack remains"
    );
    assert_eq!(
        status_stack_count(&env),
        1,
        "exactly the remaining stack should stay applied"
    );
}

#[test]
fn pull_prunes_integrated_branch_from_partial_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "C"], "refs/heads/base");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ozt add A
┊│
┊├┄ h0 [C] (merged upstream)
┊●   rkq add C
├╯
┊
┊● d4cb681 (upstream: origin/main) 2 new commits
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   ozt add A
├╯
┊
┴ d4cb681 (common base) 2000-01-02 add upstream

Hint: run `but help` for all commands

"#]]);

    assert!(
        git_ref_exists(&env, "refs/heads/A"),
        "the remaining top branch should stay in the workspace"
    );
    assert_eq!(
        status_branch_names(&env),
        vec!["A"],
        "workspace status should contain only the rebased top branch after pruning the integrated lower branch"
    );
    assert_eq!(
        status_stack_count(&env),
        1,
        "the partially integrated stack should remain applied through its top branch"
    );
}

#[test]
fn pull_keeps_empty_branch_above_merged_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "upstream-merged-branch-below-empty-branch",
    );
    env.setup_single_stack_metadata_at_target(&["top", "bottom"], "refs/heads/main");
    env.invoke_git("remote set-url origin .");

    env.but("pull").assert().success();

    assert_eq!(
        status_branch_names(&env),
        vec!["top"],
        "pull should prune only the genuinely merged lower branch and preserve the empty top branch"
    );
    assert!(
        git_ref_exists(&env, "refs/heads/top"),
        "the empty top branch was not merged itself and must survive"
    );
    assert!(
        !git_ref_exists(&env, "refs/heads/bottom"),
        "the lower branch landed upstream and should be removed"
    );
}

#[test]
fn pull_check_uses_workspace_dry_run_for_partial_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "C"], "refs/heads/base");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ozt add A
┊│
┊├┄ h0 [C] (merged upstream)
┊●   rkq add C
├╯
┊
┊● d4cb681 (upstream: origin/main) 2 new commits
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
        git_ref_exists(&env, "refs/heads/C"),
        "dry-run check should not remove the integrated lower branch"
    );
    assert_eq!(
        status_branch_names(&env),
        vec!["A", "C"],
        "dry-run check should leave both stack branches in workspace status"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ozt add A
┊│
┊├┄ h0 [C] (merged upstream)
┊●   rkq add C
├╯
┊
┊● d4cb681 (upstream: origin/main) 2 new commits (checked 0 seconds ago)
├╯ 0dc3733 (common base) 2000-01-02 add M

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

#[test]
fn pull_check_reports_conflicted_branches_as_rebasable() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-conflicted");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");
    env.invoke_git("remote set-url origin .");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   nyo A-change
├╯
┊
┊● bdfcf28 (upstream: origin/main) 1 new commit
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);

    let output = env
        .but("--json pull --check")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output: serde_json::Value = serde_json::from_slice(&output).unwrap();
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
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   nyo A-change (no changes) {conflicted}
├╯
┊
┴ bdfcf28 (common base) 2000-01-02 main-change

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pull_reparents_workspace_to_target_after_all_stacks_integrate() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pull-two-integrated-stacks");
    env.setup_metadata_at_target(&["A", "B"], "origin/main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (no commits)
├╯
┊
┊╭┄ h0 [B] (no commits)
├╯
┊
┊● 7e5d4e1 (upstream: origin/main) 3 new commits
├╯ 7e5d4e1 (common base) 2000-01-02 add upstream

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┴ 7e5d4e1 (common base) 2000-01-02 add upstream

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_eq!(
        rev_parse(&env, "gitbutler/workspace^"),
        rev_parse(&env, "origin/main"),
        "once all stacks are integrated, the workspace should be parented to the advanced target"
    );
    assert_eq!(
        status_stack_count(&env),
        0,
        "no stacks should remain applied once both are integrated"
    );
}

#[test]
fn pull_reparents_empty_workspace_when_target_advances() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata_at_target(&[], "origin/main");
    env.invoke_git("remote set-url origin .");

    env.invoke_git("checkout main");
    env.file("upstream.txt", "upstream\n");
    env.invoke_git("add upstream.txt");
    env.invoke_git("commit -m upstream-change");
    env.invoke_git("checkout gitbutler/workspace");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("pull").assert().success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┴ 526bb83 (common base) 2000-01-02 upstream-change

Hint: run `but branch new` to create a new branch to work on

"#]]);

    assert_eq!(
        rev_parse(&env, "gitbutler/workspace^"),
        rev_parse(&env, "origin/main"),
        "an empty workspace should still move forward when the target advances"
    );
}

#[test]
fn pull_does_not_report_branch_rebase_conflicts_as_worktree_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");

    env.file("shared.txt", "local\nunchanged\nextra local work\n");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊   ot M shared.txt
┊
┊╭┄ g0 [A]
┊●   vxp local change
├╯
┊
┊● 247c151 (upstream: origin/main) 1 new commit
├╯ 247c151 (common base) 2000-01-02 upstream change

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let output = env.but("pull").output().unwrap();
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
        status_branch_names(&env),
        vec!["A"],
        "conflicted branch should remain in the workspace after pull"
    );
    assert!(
        branch_has_conflicted_commit(&env, "A"),
        "pull should materialize the rebase conflict on a commit inside branch A"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊   ot M shared.txt
┊
┊╭┄ g0 [A]
┊◐   vxp local change (no changes) {conflicted}
├╯
┊
┴ 247c151 (common base) 2000-01-02 upstream change

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn pull_json_reports_branch_rebase_conflicts_as_successful_integration() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   vxp local change
├╯
┊
┊● 247c151 (upstream: origin/main) 1 new commit
├╯ 247c151 (common base) 2000-01-02 upstream change

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);

    let output = env
        .but("--json pull")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output: serde_json::Value = serde_json::from_slice(&output).unwrap();

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
        branch_has_conflicted_commit(&env, "A"),
        "pull should leave the conflicted commit visible in branch status"
    );

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   vxp local change (no changes) {conflicted}
├╯
┊
┴ 247c151 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pull_reports_conflict_in_lower_branch_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflict-in-lower-branch-of-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "B"], "main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   [..] top change
┊│
┊├┄ h0 [B]
┊●   [..] bottom change
├╯
┊
┊● 7f73771 (upstream: origin/main) 1 new commit
├╯ 7f73771 (common base) 2000-01-02 upstream change

Hint: origin/main moved ahead; run `but pull` to update the workspace

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
      rou [conflict] bottom change

To resolve conflicts:
  1. Start with: `but resolve rou`. Worktree files show no conflict markers until this checks the commit out
  2. Edit files to resolve the conflicts
  3. Run `but resolve finish` to finalize the resolution

To undo this operation:
  Run `but undo`

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   rvk top change
┊│
┊├┄ h0 [B]
┊◐   rou bottom change (no changes) {conflicted}
├╯
┊
┴ 7f73771 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pull_reports_conflicts_in_multiple_branches_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "B"], "main");

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   [..] top change
┊│
┊├┄ h0 [B]
┊●   [..] bottom change
├╯
┊
┊● e4933d8 (upstream: origin/main) 1 new commit
├╯ [..] (common base) 2000-01-02 upstream change

Hint: origin/main moved ahead; run `but pull` to update the workspace

"#]]);

    env.but("pull").assert().success().stdout_eq(str![[r#"

Found 1 upstream commits on origin/main
   [..] upstream change

Updating 2 active branches...

Rebase resulted in some conflicts

Summary
────────
  A - conflicted
      trk [conflict] bottom change
      wmr [conflict] top change
  B - conflicted
      trk [conflict] bottom change

To resolve conflicts:
  1. Start with: `but resolve trk`. Worktree files show no conflict markers until this checks the commit out
  2. Edit files to resolve the conflicts
  3. Run `but resolve finish` to finalize the resolution

To undo this operation:
  Run `but undo`

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   wmr top change (no changes) {conflicted}
┊│
┊├┄ h0 [B]
┊◐   trk bottom change (no changes) {conflicted}
├╯
┊
┴ e4933d8 (common base) 2000-01-02 upstream change

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn pull_does_not_write_conflict_markers_into_uncommitted_files() {
    // A dirty worktree edit on the same path a branch rebase conflicts on used to be at risk of
    // silently receiving conflict markers. Modern integration materializes the conflict onto the
    // commit instead, so the uncommitted file must be left untouched.
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-branch-and-dirty-worktree-conflict",
    );
    env.setup_metadata_at_target(&["A"], "main");

    env.file("shared.txt", "local\nunchanged\nextra local work\n");

    env.but("pull").assert().success();

    let shared = std::fs::read_to_string(env.projects_root().join("shared.txt")).unwrap();
    assert!(
        !shared.contains("<<<<<<<"),
        "pull must not write conflict markers into uncommitted files; got:\n{shared}"
    );
    assert!(
        branch_has_conflicted_commit(&env, "A"),
        "the rebase conflict should be materialized on branch A's commit, not the worktree file"
    );
}

#[test]
fn pull_reports_worktree_conflict_paths() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-dirty-worktree-conflicts-with-clean-rebase",
    );
    env.setup_metadata_at_target(&["A"], "main");

    // The branch rebases cleanly, but this uncommitted edit conflicts with the upstream change
    // to `shared.txt` on the resulting workspace head.
    env.file("shared.txt", "dirty local\nmore local work\n");

    env.but("pull").assert().success().stdout_eq(str![[r#"

Found 1 upstream commits on origin/main
   [..] upstream change

Updating 1 active branches...

Rebase successful

Summary
────────
  A - rebased

To undo this operation:
  Run `but undo`

⚠ A conflict occurred during checkout. Run `but status` for more information.

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊    shared.txt {conflicted}
┊
┊╭┄ g0 [A]
┊◐   zyx add A
├╯
┊
┴ 7f73771 (common base) 2000-01-02 upstream change
⚠ Uncommitted file conflicts: choose the desired file state, then run `git add -- <path>`.

Hint: run `but help` for all commands

"#]]);
}

fn git_ref_exists(env: &Sandbox, ref_name: &str) -> bool {
    env.open_repo()
        .try_find_reference(ref_name)
        .unwrap()
        .is_some()
}

fn rev_parse(env: &Sandbox, spec: &str) -> String {
    let values = rev_parse_all(env, spec);
    let [value] = values.as_slice() else {
        panic!("expected exactly one rev for {spec}, got {values:?}");
    };
    value.clone()
}

fn rev_parse_all(env: &Sandbox, spec: &str) -> std::vec::Vec<std::string::String> {
    env.invoke_git(&format!("rev-parse {spec}"))
        .lines()
        .map(str::to_owned)
        .collect()
}

fn status_stack_count(env: &Sandbox) -> usize {
    let status = status_json(env);
    status["stacks"].as_array().map_or(0, Vec::len)
}

fn status_branch_names(env: &Sandbox) -> Vec<String> {
    let status = status_json(env);
    status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .filter_map(|branch| branch["name"].as_str().map(str::to_owned))
        .collect()
}

fn branch_has_conflicted_commit(env: &Sandbox, branch_name: &str) -> bool {
    let status = status_json(env);
    status["stacks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .filter(|branch| branch["name"].as_str() == Some(branch_name))
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .any(|commit| commit["conflicted"].as_bool() == Some(true))
}

fn status_json(env: &Sandbox) -> serde_json::Value {
    let output = env
        .but("status --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}
