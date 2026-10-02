use anyhow::Context as _;
use snapbox::{IntoData as _, str};

use super::util::enter_edit_mode_with_conflicted_commit;
use crate::{command::util::sandbox_with_conflicted_commit, utils::Sandbox};

fn current_branch_name(env: &Sandbox) -> String {
    let repo = env.open_repo();
    repo.rev_parse_single("HEAD")
        .context("HEAD should resolve")
        .unwrap();
    repo.head_name()
        .unwrap()
        .map(|name| name.as_ref().shorten().to_string())
        .context("HEAD should point to a branch")
        .unwrap()
}

#[test]
fn resolve_status_and_finish_work_in_edit_mode() {
    let env = enter_edit_mode_with_conflicted_commit();

    env.but("resolve status")
        .assert()
        .success()
        .stderr_eq(str![""]);

    env.file("file.txt", "resolved content\n");
    env.invoke_git("add file.txt");

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![""])
        .stdout_eq(str![[r#"
✓ Conflict resolution finalized successfully!
The commit has been updated with your resolved changes.
No conflict markers remain in the resolved files.
Workspace restored; uncommitted changes intact: uncommitted.txt
No conflicted commits remain.

"#]]);

    assert_eq!(current_branch_name(&env), "gitbutler/workspace");
}

#[test]
fn resolve_finish_reports_leftover_markers_and_uncommitted_paths() {
    let env = enter_edit_mode_with_conflicted_commit();

    // A "resolution" that leaves conflict markers behind.
    env.file(
        "file.txt",
        "<<<<<<< ours\nline 2\n=======\nline two\n>>>>>>> theirs\n",
    );
    env.invoke_git("add file.txt");

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![""])
        .stdout_eq(str![[r#"
✓ Conflict resolution finalized successfully!
The commit has been updated with your resolved changes.
✗ file.txt still contains conflict markers — resolve it again if that was not intentional
Workspace restored; uncommitted changes intact: uncommitted.txt
No conflicted commits remain.

"#]]);

    assert_eq!(current_branch_name(&env), "gitbutler/workspace");
}

#[test]
fn resolve_finish_reports_every_remaining_conflicted_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env);
    let conflicted_bottom_commit = status["stacks"]
        .as_array()
        .context("status stacks should be an array")
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("bottom change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted bottom commit")
        .unwrap();

    env.but(format!("resolve {conflicted_bottom_commit}"))
        .assert()
        .success();
    env.file("bottom.txt", "resolved bottom\n");
    env.invoke_git("add bottom.txt");

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![""])
        .stdout_eq(str![[r#"
✓ Conflict resolution finalized successfully!
The commit has been updated with your resolved changes.
No conflict markers remain in the resolved files.
Workspace restored; no uncommitted changes.

Remaining conflicted commits (oldest first):
  Branch: A
    ● [..] [conflict] top change
Resolve the next commit with but resolve [..].

"#]]);
}

#[test]
fn resolve_finish_json_deduplicates_shared_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env);
    let branch = super::util::find_branch(&status, "A");
    let conflicted_top_commit = branch["commits"]
        .as_array()
        .context("branch commits should be an array")
        .unwrap()
        .iter()
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("top change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted top commit")
        .unwrap();

    env.but(format!("resolve {conflicted_top_commit}"))
        .assert()
        .success();
    env.file("top.txt", "resolved top\n");
    env.invoke_git("add top.txt");

    let output = super::util::but_std_cmd(&env, "--json resolve finish")
        .output()
        .unwrap();
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["total_remaining_conflicted_commits"], 1,
        "the shared bottom conflict should be counted once"
    );
    assert_eq!(
        json["resolution_queue"]
            .as_array()
            .context("resolution_queue should be an array")
            .unwrap()
            .len(),
        1,
        "the resolution queue should contain unique commits"
    );
    assert_eq!(
        json["count"], 0,
        "an existing conflict should not be classified as new"
    );
    assert_eq!(
        json["newly_conflicted_commits"]
            .as_array()
            .context("newly_conflicted_commits should be an array")
            .unwrap()
            .len(),
        0,
        "the legacy newly-conflicted field should remain present"
    );
}

#[test]
fn agent_resolve_finish_json_includes_result_and_status() {
    let env = enter_edit_mode_with_conflicted_commit();
    env.file("file.txt", "resolved content\n");
    env.invoke_git("add file.txt");

    let mut command = super::util::but_std_cmd(&env, "--json resolve finish --status-after");
    command.env("AI_AGENT", "codex");
    let output = command.output().unwrap();
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["result"].is_object(),
        "agent output should retain the resolve result"
    );
    assert!(
        json["status"]["stacks"].is_array(),
        "agent output should include resulting workspace status"
    );
}

#[test]
fn agent_resolve_finish_json_status_tracks_rebased_conflict_queue() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env);
    let branch = super::util::find_branch(&status, "A");
    let commits = branch["commits"]
        .as_array()
        .context("branch commits should be an array")
        .unwrap();
    let conflicted_top_commit = commits
        .iter()
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("top change")
        })
        .context("should find the conflicted top commit")
        .unwrap();
    let top_commit_id_before = conflicted_top_commit["commitId"]
        .as_str()
        .context("top commit should have a full commit ID")
        .unwrap()
        .to_owned();
    let conflicted_bottom_commit = status["stacks"]
        .as_array()
        .context("status stacks should be an array")
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("bottom change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted bottom commit")
        .unwrap()
        .to_owned();

    env.but(format!("resolve {conflicted_bottom_commit}"))
        .assert()
        .success();
    env.file("bottom.txt", "resolved bottom\n");
    env.invoke_git("add bottom.txt");

    let mut command = super::util::but_std_cmd(&env, "--json resolve finish --status-after");
    command.env("AI_AGENT", "codex");
    let output = command.output().unwrap();
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let queue = json["result"]["resolution_queue"]
        .as_array()
        .context("resolution_queue should be an array")
        .unwrap();
    assert_eq!(
        queue.len(),
        1,
        "only the rebased top conflict should remain"
    );
    let queued_top = &queue[0];
    assert!(
        queued_top["commit_message"]
            .as_str()
            .is_some_and(|message| message.ends_with("top change")),
        "the queue should identify the remaining top conflict"
    );
    assert_ne!(
        queued_top["commit_id"].as_str(),
        Some(top_commit_id_before.as_str()),
        "finishing the bottom conflict should rebase the top commit"
    );
    assert_eq!(
        json["result"]["total_remaining_conflicted_commits"], 1,
        "the result should count the remaining top conflict once"
    );

    let status_branch = super::util::find_branch(&json["status"], "A");
    let status_top = status_branch["commits"]
        .as_array()
        .context("status branch commits should be an array")
        .unwrap()
        .iter()
        .find(|commit| {
            commit["message"]
                .as_str()
                .is_some_and(|message| message.ends_with("top change"))
        })
        .context("status should contain the rebased top commit")
        .unwrap();
    assert_eq!(
        status_top["conflicted"].as_bool(),
        Some(true),
        "the rebased top commit should remain conflicted"
    );
    assert_eq!(
        status_top["commitId"], queued_top["commit_id"],
        "status and the result queue should expose the same fresh commit ID"
    );
    assert_eq!(
        status_top["cliId"], queued_top["commit_short_id"],
        "status and the result queue should expose the same actionable selector"
    );
}

#[test]
fn resolve_cancel_works_in_edit_mode() {
    let env = enter_edit_mode_with_conflicted_commit();

    env.but("resolve cancel --force")
        .assert()
        .stderr_eq(str![""])
        .success();
    assert_eq!(current_branch_name(&env), "gitbutler/workspace");
}

#[test]
fn resolve_cancel_requires_force_when_changes_were_made() {
    let env = enter_edit_mode_with_conflicted_commit();

    env.file("file.txt", "resolved content with additional edits\n");

    env.but("resolve cancel")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. There are changes that differ from the original commit you were editing. Canceling will drop those changes.

If you want to go through with this, please re-run with `--force`.

If you want to keep the changes you have made, consider finishing the resolution and then moving the changes with `but squash`.

"#]]);

    env.but("resolve cancel --force")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env), "gitbutler/workspace");
}

/// A sandbox with real checkout-produced conflicts on `commit.txt` and `second.txt`
/// (`UU`) and an unrelated uncommitted change to `other.txt`.
fn sandbox_with_conflicted_uncommitted_files() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("commit.txt", "first\n");
    env.file("second.txt", "first\n");
    env.but("commit -b A -m first").assert().success();
    env.file("commit.txt", "second\n");
    env.file("second.txt", "second\n");
    env.but("commit -b A -m second").assert().success();
    let commit_id = env.invoke_git("rev-parse refs/heads/A");
    env.file("commit.txt", "would conflict\n");
    env.file("second.txt", "would conflict\n");
    env.but(format!("discard {commit_id}")).assert().success();
    env.file("other.txt", "unrelated\n");
    assert_eq!(
        env.invoke_git("ls-files --unmerged").lines().count(),
        6,
        "both files carry base/ours/theirs stages"
    );
    env
}

#[test]
fn resolve_marks_several_conflicted_files_resolved_including_deleted() {
    let env = sandbox_with_conflicted_uncommitted_files();
    env.remove_file("commit.txt");

    env.but("resolve commit.txt second.txt --json")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "resolved_files": [
    "commit.txt",
    "second.txt"
  ]
}

"#]]);
    assert_eq!(
        env.invoke_git("ls-files --unmerged"),
        "",
        "no unmerged entries are left"
    );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   ts D commit.txt
┊   xw A other.txt
┊   or M second.txt
┊
┊╭┄ g0 [A]
┊●   sxy first
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn resolve_refuses_mixed_and_ai_targets_for_conflicted_files() {
    let env = sandbox_with_conflicted_uncommitted_files();

    env.but("resolve commit.txt other.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. 'other.txt' is not a conflicted uncommitted file; `but resolve` takes either one commit or only conflicted files (see `but status`).

"#]]);
    env.but("resolve commit.txt --ai")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. Conflicted uncommitted files can only be marked as resolved: `but resolve <path>...`

"#]]);
    assert_eq!(
        env.invoke_git("ls-files --unmerged").lines().count(),
        6,
        "nothing was resolved"
    );
}

/// Enter edit mode on branch A's commit after integrating the target of `scenario`
/// conflicted it on submodule `sm`. Entering edit mode removes the `sm` worktree.
fn enter_edit_mode_with_submodule_conflict(scenario: &str) -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow(scenario);
    env.setup_metadata_at_target(&["A"], "refs/heads/base");
    env.invoke_git("remote set-url origin .");
    env.but("pull").assert().success();
    let conflicted = env.invoke_git("rev-parse A");
    env.but(format!("resolve {conflicted}")).assert().success();
    assert_eq!(current_branch_name(&env), "gitbutler/edit");
    env
}

/// Check out the submodule commit at `branch` of its source into `sm`.
fn check_out_submodule(env: &Sandbox, branch: &str) -> String {
    let commit = env.invoke_git(&format!("-C sm-source rev-parse {branch}"));
    env.file("sm/.git", "gitdir: ../.git/modules/sm\n");
    env.invoke_git(&format!("-C sm checkout -q -f {commit}"));
    commit
}

/// The conflicted and resolved paths `but resolve status` reports.
fn resolve_status_paths(env: &Sandbox) -> (Vec<String>, Vec<String>) {
    let output = env.but("--json resolve status").assert().success();
    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let paths = |key: &str| -> Vec<String> {
        serde_json::from_value(json[key].clone()).expect("status lists paths")
    };
    (paths("conflicted_files"), paths("resolved_files"))
}

/// The refs, HEAD, index, worktree and edit-mode metadata a refused finish must leave alone.
fn repo_state(env: &Sandbox) -> String {
    let mut state = [
        "symbolic-ref HEAD",
        "for-each-ref",
        "ls-files --stage",
        "status --porcelain --ignore-submodules=none",
    ]
    .map(|args| env.invoke_git(args))
    .join("\n");
    for file in ["edit_mode_metadata.toml", "virtual_branches.toml"] {
        state += &std::fs::read_to_string(env.projects_root().join(".git/gitbutler").join(file))
            .expect("GitButler keeps its metadata in .git/gitbutler");
    }
    state
}

#[test]
fn resolve_finish_refuses_unresolved_submodule_conflict() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    assert_eq!(
        env.invoke_git("ls-files --stage -- sm")
            .lines()
            .map(|line| line.split_whitespace().next().unwrap())
            .collect::<Vec<_>>(),
        ["160000"; 3],
        "the edit-mode index carries base, ours and theirs gitlink stages"
    );
    env.file("README", "resolved\n");
    let before = repo_state(&env);

    env.but("resolve finish")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. Unresolved submodule conflicts: "sm"
Put the wanted submodule commit or file at each path and run `git add -- <path>`, remove it with `git rm -- <path>`, or discard the resolution with `but resolve cancel --force`.

"#]]);
    env.but("--json resolve finish")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. Unresolved submodule conflicts: "sm"
...
"#]]);

    assert_eq!(repo_state(&env), before, "a refused finish changes nothing");
    assert_eq!(
        resolve_status_paths(&env),
        (vec!["sm".into()], vec!["README".into()]),
        "status reports what finish refuses on, and README as resolved by its edit"
    );

    let tip = env.invoke_git("rev-parse A");
    env.but("resolve cancel").assert().failure();
    env.but("resolve cancel --force").assert().success();
    assert_eq!(
        (current_branch_name(&env), env.invoke_git("rev-parse A")),
        ("gitbutler/workspace".into(), tip),
        "the escape the refusal names leaves edit mode and keeps the commit"
    );
}

#[test]
fn resolve_finish_records_the_selected_submodule_side() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    let upstream_side = check_out_submodule(&env, "b");
    env.but("resolve finish").assert().failure();
    env.invoke_git("add -- sm");
    env.file("README", "resolved\n");
    assert_eq!(
        resolve_status_paths(&env),
        (vec![], vec!["README".into(), "sm".into()]),
        "a staged gitlink is resolved"
    );

    env.but("resolve finish").assert().success();
    assert_eq!(current_branch_name(&env), "gitbutler/workspace");
    assert_eq!(
        env.invoke_git("rev-parse A:sm"),
        upstream_side,
        "the selected upstream submodule commit is recorded"
    );
}

#[test]
fn resolve_finish_keeps_the_branch_submodule_side_and_warns_about_text_markers() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    let branch_side = check_out_submodule(&env, "c");
    env.invoke_git("add -- sm");
    assert_eq!(
        resolve_status_paths(&env),
        (vec!["README".into()], vec!["sm".into()]),
        "only README still has conflict markers"
    );

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![""])
        .stdout_eq(str![[r#"
✓ Conflict resolution finalized successfully!
The commit has been updated with your resolved changes.
✗ README still contains conflict markers — resolve it again if that was not intentional
...
"#]]);
    assert_eq!(
        env.invoke_git("rev-parse A:sm"),
        branch_side,
        "the selected branch submodule commit is recorded"
    );
}

#[test]
fn resolve_finish_refuses_empty_or_missing_submodule_until_removal_is_staged() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    env.file("README", "resolved\n");
    let tip = env.invoke_git("rev-parse A");

    std::fs::create_dir(env.projects_root().join("sm")).unwrap();
    env.invoke_git_fails(
        "add -- sm",
        "Git does not stage a submodule without a commit",
    );
    env.but("resolve finish").assert().failure();
    std::fs::remove_dir(env.projects_root().join("sm")).unwrap();
    env.but("resolve finish").assert().failure();
    assert_eq!(
        (current_branch_name(&env), env.invoke_git("rev-parse A")),
        ("gitbutler/edit".into(), tip),
        "the refusals stay in edit mode and keep the commit"
    );

    env.invoke_git("rm -q -- sm");
    assert_eq!(resolve_status_paths(&env).0, Vec::<String>::new());
    env.but("resolve finish").assert().success();
    assert_eq!(
        env.invoke_git("ls-tree --name-only A"),
        ".gitmodules\nREADME",
        "the staged removal is recorded"
    );
}

#[test]
fn resolve_finish_refuses_file_vs_submodule_conflict_until_staged() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-file-vs-submodule-conflict");
    assert!(
        !env.projects_root().join("sm").exists(),
        "nothing is checked out at the conflicted path"
    );
    assert_eq!(resolve_status_paths(&env).0, ["sm"]);
    env.but("resolve finish").assert().failure();

    env.file("sm", "file-on-A\n");
    env.but("resolve finish").assert().failure();
    env.invoke_git("add -- sm");
    assert_eq!(resolve_status_paths(&env), (vec![], vec!["sm".into()]));
    env.but("resolve finish").assert().success();
    assert_eq!(
        env.invoke_git("cat-file -p A:sm"),
        "file-on-A",
        "the staged file side is recorded"
    );
}

#[test]
fn resolve_finish_checks_submodule_conflicts_under_the_worktree_lock() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    let branch_side = check_out_submodule(&env, "c");
    env.invoke_git("add -- sm");
    env.file("README", "resolved\n");
    let tip = env.invoke_git("rev-parse A");

    // Hold the inter-process write lock as another GitButler operation would.
    let mut lock = but_core::sync::LockFile::open(
        env.projects_root()
            .join(".git/gitbutler/gitbutler.write-lock"),
    )
    .unwrap();
    lock.lock().unwrap();
    let finish = super::util::but_std_cmd(&env, "resolve finish")
        .spawn()
        .unwrap();
    // Give finish time to get as far as it can without the lock; the outcome
    // must not depend on how far that is.
    std::thread::sleep(std::time::Duration::from_secs(1));
    let (base, upstream) = (
        env.invoke_git("-C sm-source rev-parse main"),
        env.invoke_git("-C sm-source rev-parse b"),
    );
    env.invoke_bash(format!(
        "printf '0 {zero} 0\\tsm\\n160000 {base} 1\\tsm\\n160000 {upstream} 2\\tsm\\n160000 {branch_side} 3\\tsm\\n' | git update-index --index-info",
        zero = "0".repeat(40)
    ));
    lock.unlock().unwrap();

    let output = finish.wait_with_output().unwrap();
    assert!(
        !output.status.success(),
        "finish checks the conflicts it saves under one lock"
    );
    assert_eq!(
        (current_branch_name(&env), env.invoke_git("rev-parse A")),
        ("gitbutler/edit".into(), tip),
        "the refusal stays in edit mode and keeps the commit"
    );
}

#[test]
fn resolve_keeps_submodule_paths_that_differ_in_non_utf8_bytes_apart() {
    let env = enter_edit_mode_with_submodule_conflict("resolve-submodule-conflict");
    env.file("README", "resolved\n");
    // `but` cannot create conflicts at non-UTF-8 paths, so write their stages directly.
    let commit = env.invoke_git("-C sm-source rev-parse b");
    env.invoke_bash(format!(
        "for p in $'sm\\xfe' $'sm\\xff'; do printf '160000 {commit} 2\\t%s\\n' \"$p\"; done | git update-index --index-info"
    ));
    let before = repo_state(&env);

    env.but("resolve finish")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. Unresolved submodule conflicts: "sm", "sm\xfe", "sm\xff"
Put the wanted submodule commit or file at each path and run `git add -- <path>`, remove it with `git rm -- <path>`, or discard the resolution with `but resolve cancel --force`.

"#]].raw());
    assert_eq!(repo_state(&env), before, "a refused finish changes nothing");
    assert_eq!(
        resolve_status_paths(&env).0.len(),
        3,
        "each conflicted submodule is listed once"
    );
}

#[test]
fn resolve_status_keeps_unreadable_text_conflicts_remaining() {
    let env = enter_edit_mode_with_conflicted_commit();
    env.remove_file("file.txt");
    env.file("file.txt/nested", "nested\n");

    assert_eq!(
        resolve_status_paths(&env).0,
        ["file.txt"],
        "a directory at a conflicted file path is not a resolution"
    );
}

#[test]
fn cannot_checkout_a_conflicted_commit_in_single_branch_mode() {
    let env = sandbox_with_conflicted_commit();
    env.but("config feature single-branch enable")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   nyo A-change (no changes) {conflicted}
├╯
┊
┴ bdfcf28 (common base, main, origin/main) 2000-01-02 main-change

Hint: run `but help` for all commands

"#]]);

    let head_before = env.invoke_git("rev-parse HEAD");
    let head_ref_before = env.invoke_git("symbolic-ref HEAD");

    env.but("switch A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot check out a conflicted commit.

Hint: Run `but switch --workspace` and start conflict resolution with `but resolve`

"#]]);

    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head_before,
        "rejecting a conflicted checkout leaves the HEAD commit unchanged"
    );
    assert_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        head_ref_before,
        "rejecting a conflicted checkout leaves the checked-out branch unchanged"
    );
}
