use anyhow::Context as _;
use snapbox::str;

use super::util::enter_edit_mode_with_conflicted_commit;
use crate::utils::Sandbox;

fn current_branch_name(env: &Sandbox) -> anyhow::Result<String> {
    let repo = env.open_repo();
    repo.rev_parse_single("HEAD")
        .context("HEAD should resolve")?;
    repo.head_name()?
        .map(|name| name.as_ref().shorten().to_string())
        .context("HEAD should point to a branch")
}

#[test]
fn resolve_status_and_finish_work_in_edit_mode() -> anyhow::Result<()> {
    let env = enter_edit_mode_with_conflicted_commit()?;

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

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_finish_reports_leftover_markers_and_uncommitted_paths() -> anyhow::Result<()> {
    let env = enter_edit_mode_with_conflicted_commit()?;

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

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_finish_reports_every_remaining_conflicted_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env)?;
    let conflicted_bottom_commit = status["stacks"]
        .as_array()
        .context("status stacks should be an array")?
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("bottom change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted bottom commit")?;

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

    Ok(())
}

#[test]
fn resolve_finish_json_deduplicates_shared_conflicts() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env)?;
    let branch = super::util::find_branch(&status, "A")?;
    let conflicted_top_commit = branch["commits"]
        .as_array()
        .context("branch commits should be an array")?
        .iter()
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("top change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted top commit")?;

    env.but(format!("resolve {conflicted_top_commit}"))
        .assert()
        .success();
    env.file("top.txt", "resolved top\n");
    env.invoke_git("add top.txt");

    let output = super::util::but_std_cmd(&env, "--json resolve finish").output()?;
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json["total_remaining_conflicted_commits"], 1,
        "the shared bottom conflict should be counted once"
    );
    assert_eq!(
        json["resolution_queue"]
            .as_array()
            .context("resolution_queue should be an array")?
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
            .context("newly_conflicted_commits should be an array")?
            .len(),
        0,
        "the legacy newly-conflicted field should remain present"
    );

    Ok(())
}

#[test]
fn agent_resolve_finish_json_includes_result_and_status() -> anyhow::Result<()> {
    let env = enter_edit_mode_with_conflicted_commit()?;
    env.file("file.txt", "resolved content\n");
    env.invoke_git("add file.txt");

    let mut command = super::util::but_std_cmd(&env, "--json resolve finish --status-after");
    command.env("AI_AGENT", "codex");
    let output = command.output()?;
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(
        json["result"].is_object(),
        "agent output should retain the resolve result"
    );
    assert!(
        json["status"]["stacks"].is_array(),
        "agent output should include resulting workspace status"
    );

    Ok(())
}

#[test]
fn agent_resolve_finish_json_status_tracks_rebased_conflict_queue() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    let status = super::util::status_json(&env)?;
    let branch = super::util::find_branch(&status, "A")?;
    let commits = branch["commits"]
        .as_array()
        .context("branch commits should be an array")?;
    let conflicted_top_commit = commits
        .iter()
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("top change")
        })
        .context("should find the conflicted top commit")?;
    let top_commit_id_before = conflicted_top_commit["commitId"]
        .as_str()
        .context("top commit should have a full commit ID")?
        .to_owned();
    let conflicted_bottom_commit = status["stacks"]
        .as_array()
        .context("status stacks should be an array")?
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .flat_map(|branch| branch["commits"].as_array().into_iter().flatten())
        .find(|commit| {
            commit["conflicted"].as_bool() == Some(true)
                && commit["message"].as_str() == Some("bottom change")
        })
        .and_then(|commit| commit["cliId"].as_str())
        .context("should find the conflicted bottom commit")?
        .to_owned();

    env.but(format!("resolve {conflicted_bottom_commit}"))
        .assert()
        .success();
    env.file("bottom.txt", "resolved bottom\n");
    env.invoke_git("add bottom.txt");

    let mut command = super::util::but_std_cmd(&env, "--json resolve finish --status-after");
    command.env("AI_AGENT", "codex");
    let output = command.output()?;
    assert!(output.status.success(), "resolve finish should succeed");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let queue = json["result"]["resolution_queue"]
        .as_array()
        .context("resolution_queue should be an array")?;
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

    let status_branch = super::util::find_branch(&json["status"], "A")?;
    let status_top = status_branch["commits"]
        .as_array()
        .context("status branch commits should be an array")?
        .iter()
        .find(|commit| {
            commit["message"]
                .as_str()
                .is_some_and(|message| message.ends_with("top change"))
        })
        .context("status should contain the rebased top commit")?;
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

    Ok(())
}

#[test]
fn resolve_cancel_works_in_edit_mode() -> anyhow::Result<()> {
    let env = enter_edit_mode_with_conflicted_commit()?;

    env.but("resolve cancel --force")
        .assert()
        .stderr_eq(str![""])
        .success();
    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_cancel_requires_force_when_changes_were_made() -> anyhow::Result<()> {
    let env = enter_edit_mode_with_conflicted_commit()?;

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

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}
