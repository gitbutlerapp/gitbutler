//! Integration tests for `but uncommit` with multiple committed-file sources.
//!
//! These exercise the multi-source uncommit path, where several committed files
//! (potentially from different commits and branches, in any order) are handed to
//! the backend in a single batched operation. Each test asserts the `but status`
//! tree and the file contents of the affected commits, both before and after the
//! uncommit.

use snapbox::str;

use crate::{
    command::util::{
        branch_commit_cli_ids, commit_file_with_worktree_changes_as_two_hunks,
        commit_two_files_as_two_hunks_each, status_json_with_files as status_json,
    },
    utils::{CommandExt, Sandbox},
};

/// Return the committed-file CLI id (e.g. `e8:nk`) for `file_path` in the commit
/// at `commit_index` (newest-first) on `branch_name`.
fn committed_file_id_in_commit(
    status: &serde_json::Value,
    branch_name: &str,
    commit_index: usize,
    file_path: &str,
) -> Option<String> {
    status["stacks"]
        .as_array()?
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == branch_name)?["commits"]
        .as_array()?
        .get(commit_index)?["changes"]
        .as_array()?
        .iter()
        .find_map(|change| {
            (change["filePath"].as_str().unwrap() == file_path)
                .then(|| change["cliId"].as_str().unwrap().to_string())
        })
}

/// Whether `file_path` currently appears among the uncommitted changes.
fn uncommitted_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
    status["uncommittedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

/// Read the contents of `file_path` as it exists in the commit named by
/// `revspec` (e.g. `A`, `A~1`). Returns `None` when the file is absent from that
/// commit's tree.
fn commit_file_content(env: &Sandbox, revspec: &str) -> Option<String> {
    let repo = env.open_repo();
    let object = repo
        .rev_parse_single(revspec.as_bytes())
        .ok()?
        .object()
        .ok()?;
    Some(String::from_utf8_lossy(&object.data).into_owned())
}

/// Read the contents of a file in the working directory.
fn worktree_file_content(env: &Sandbox, path: &str) -> String {
    std::fs::read_to_string(env.projects_root().join(path)).expect("worktree file should exist")
}

fn branch_commits_contain_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .filter(|branch| branch["name"].as_str().unwrap() == branch_name)
        .flat_map(|branch| branch["commits"].as_array().unwrap().iter())
        .flat_map(|commit| commit["changes"].as_array().unwrap().iter())
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

fn committed_file_cli_id_for_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> Option<String> {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == branch_name)
        .and_then(|branch| {
            branch["commits"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|commit| {
                    commit["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find_map(|change| {
                            (change["filePath"].as_str().unwrap() == file_path)
                                .then(|| change["cliId"].as_str().unwrap().to_string())
                        })
                })
        })
}

#[test]
fn uncommit_different_files_from_different_commits_same_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // Two commits on branch A, each introducing a different file.
    env.file("c1.txt", "c1 content\n");
    env.but("commit A -m 'add c1'").assert().success();
    env.file("c2.txt", "c2 content\n");
    env.but("commit A -m 'add c2'").assert().success();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1#0 add c2
┊│     1#0:w A c2.txt
┊●   1#1 add c1
┊│     1#1:l A c1.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // c2 lives in the newest commit (index 0), c1 in the older one (index 1).
    let before = status_json(&env)?;
    let c1_id =
        committed_file_id_in_commit(&before, "A", 1, "c1.txt").expect("c1.txt committed-file id");
    let c2_id =
        committed_file_id_in_commit(&before, "A", 0, "c2.txt").expect("c2.txt committed-file id");

    // Commit contents before uncommitting.
    assert_eq!(
        commit_file_content(&env, "A:c2.txt").as_deref(),
        Some("c2 content\n")
    );
    assert_eq!(
        commit_file_content(&env, "A~1:c1.txt").as_deref(),
        Some("c1 content\n")
    );

    // Uncommit both, passing the older (parent) commit's file first to prove the
    // backend sorts child-to-parent and rebases once, without stale commit IDs.
    env.but(format!("uncommit {c1_id},{c2_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Uncommitted changes

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   lsk A c1.txt
┊   wyx A c2.txt
┊
┊╭┄g0 [A]
┊●   1#0 add c2 (no changes)
┊●   1#1 add c1 (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit <branch> -m "message" --changes <id>` to commit them

"#]]);

    let after = status_json(&env)?;
    assert!(uncommitted_contains_file(&after, "c1.txt"));
    assert!(uncommitted_contains_file(&after, "c2.txt"));

    // Both files were removed from the commit trees but remain in the worktree.
    assert_eq!(commit_file_content(&env, "A:c2.txt"), None);
    assert_eq!(commit_file_content(&env, "A~1:c1.txt"), None);
    assert_eq!(worktree_file_content(&env, "c1.txt"), "c1 content\n");
    assert_eq!(worktree_file_content(&env, "c2.txt"), "c2 content\n");

    Ok(())
}

#[test]
fn uncommit_different_files_from_the_same_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // A single commit on branch A introducing two files. The CLI groups both
    // committed-file ids into one source for that commit.
    env.file("c1.txt", "c1 content\n");
    env.file("c2.txt", "c2 content\n");
    env.but("commit A -m 'add c1 and c2'").assert().success();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1 add c1 and c2
┊│     1:l A c1.txt
┊│     1:w A c2.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Both files live in the same (newest) commit.
    let before = status_json(&env)?;
    let c1_id =
        committed_file_id_in_commit(&before, "A", 0, "c1.txt").expect("c1.txt committed-file id");
    let c2_id =
        committed_file_id_in_commit(&before, "A", 0, "c2.txt").expect("c2.txt committed-file id");

    assert_eq!(
        commit_file_content(&env, "A:c1.txt").as_deref(),
        Some("c1 content\n")
    );
    assert_eq!(
        commit_file_content(&env, "A:c2.txt").as_deref(),
        Some("c2 content\n")
    );

    // Uncommit both files from the one commit in a single call.
    env.but(format!("uncommit {c1_id},{c2_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Uncommitted changes

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   lsk A c1.txt
┊   wyx A c2.txt
┊
┊╭┄g0 [A]
┊●   1 add c1 and c2 (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit <branch> -m "message" --changes <id>` to commit them

"#]]);

    let after = status_json(&env)?;
    assert!(uncommitted_contains_file(&after, "c1.txt"));
    assert!(uncommitted_contains_file(&after, "c2.txt"));

    // Both files were removed from the commit tree but remain in the worktree.
    assert_eq!(commit_file_content(&env, "A:c1.txt"), None);
    assert_eq!(commit_file_content(&env, "A:c2.txt"), None);
    assert_eq!(worktree_file_content(&env, "c1.txt"), "c1 content\n");
    assert_eq!(worktree_file_content(&env, "c2.txt"), "c2 content\n");

    Ok(())
}

#[test]
fn uncommit_same_file_from_different_commits_same_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // Different lengths prevent racy-clean false negatives on coarse-mtime filesystems.
    env.file("f.txt", "v1\n");
    env.but("commit A -m 'write v1'").assert().success();
    env.file("f.txt", "v22\n");
    env.but("commit A -m 'write v2'").assert().success();
    env.file("f.txt", "v333\n");
    env.but("commit A -m 'write v3'").assert().success();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1#0 write v3
┊│     1#0:s M f.txt
┊●   1#1 write v2
┊│     1#1:s M f.txt
┊●   1#2 write v1
┊│     1#2:s A f.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Commit contents before: newest-first is v3, v2, v1.
    assert_eq!(
        commit_file_content(&env, "A:f.txt").as_deref(),
        Some("v333\n")
    );
    assert_eq!(
        commit_file_content(&env, "A~1:f.txt").as_deref(),
        Some("v22\n")
    );
    assert_eq!(
        commit_file_content(&env, "A~2:f.txt").as_deref(),
        Some("v1\n")
    );

    let before = status_json(&env)?;
    let v1_id = committed_file_id_in_commit(&before, "A", 2, "f.txt").expect("v1 f.txt id");
    let v2_id = committed_file_id_in_commit(&before, "A", 1, "f.txt").expect("v2 f.txt id");
    let v3_id = committed_file_id_in_commit(&before, "A", 0, "f.txt").expect("v3 f.txt id");

    // Uncommit the file from all three commits in a deliberately shuffled order
    // (middle, top, bottom) to prove the input does not need to be sorted.
    env.but(format!("uncommit {v2_id},{v3_id},{v1_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Uncommitted changes

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   spm A f.txt
┊
┊╭┄g0 [A]
┊●   1#0 write v3 (no changes)
┊●   1#1 write v2 (no changes)
┊●   1#2 write v1 (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit <branch> -m "message" --changes <id>` to commit them

"#]]);

    let after = status_json(&env)?;
    assert!(uncommitted_contains_file(&after, "f.txt"));

    // The file is gone from every commit tree, and the worktree keeps the latest
    // (v3) content.
    assert_eq!(commit_file_content(&env, "A:f.txt"), None);
    assert_eq!(commit_file_content(&env, "A~1:f.txt"), None);
    assert_eq!(commit_file_content(&env, "A~2:f.txt"), None);
    assert_eq!(worktree_file_content(&env, "f.txt"), "v333\n");

    Ok(())
}

#[test]
fn uncommit_different_files_from_different_commits_different_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // One commit on each of the two parallel stacks, each adding its own file.
    env.file("fa.txt", "a content\n");
    env.but("commit A -m 'add fa'").assert().success();
    env.file("fb.txt", "b content\n");
    env.but("commit B -m 'add fb'").assert().success();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1#0 add fa
┊│     1#0:s A fa.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   1#1 add fb
┊│     1#1:q A fb.txt
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        commit_file_content(&env, "A:fa.txt").as_deref(),
        Some("a content\n")
    );
    assert_eq!(
        commit_file_content(&env, "B:fb.txt").as_deref(),
        Some("b content\n")
    );

    let before = status_json(&env)?;
    let fa_id = committed_file_id_in_commit(&before, "A", 0, "fa.txt").expect("fa.txt id");
    let fb_id = committed_file_id_in_commit(&before, "B", 0, "fb.txt").expect("fb.txt id");

    // Uncommit one file from each branch in a single batched operation.
    env.but(format!("uncommit {fa_id},{fb_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Uncommitted changes

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄zz [uncommitted]
┊   sky A fa.txt
┊   qqz A fb.txt
┊
┊╭┄g0 [A]
┊●   1#0 add fa (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   1#1 add fb (no changes)
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit <branch> -m "message" --changes <id>` to commit them

"#]]);

    let after = status_json(&env)?;
    assert!(uncommitted_contains_file(&after, "fa.txt"));
    assert!(uncommitted_contains_file(&after, "fb.txt"));

    // Both files were removed from their respective branch commits but remain in
    // the worktree.
    assert_eq!(commit_file_content(&env, "A:fa.txt"), None);
    assert_eq!(commit_file_content(&env, "B:fb.txt"), None);
    assert_eq!(worktree_file_content(&env, "fa.txt"), "a content\n");
    assert_eq!(worktree_file_content(&env, "fb.txt"), "b content\n");

    Ok(())
}

#[test]
fn uncommit_command_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env.but("--format json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_cli_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test uncommit command
    env.but(format!("uncommit {commit_cli_id}"))
        .assert()
        .success();

    // Verify the files are now uncommitted
    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "nkn",
      "filePath": "a.txt",
      "changeType": "added"
    },
    {
      "cliId": "pnl",
      "filePath": "b.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "k0",
      "assignedChanges": [],
      "branches": [
...

"#]]);

    Ok(())
}

#[test]
fn uncommit_diff_json_keeps_mutation_result_and_diff() -> anyhow::Result<()> {
    fn run(agent: bool) -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

        env.setup_metadata(&["A", "B"]);
        commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

        let before = status_json(&env)?;
        let commit_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();
        let command = format!("--format json uncommit {commit_cli_id} --diff");
        let output = if agent {
            env.but(command)
                .env("AI_AGENT", "codex")
                .allow_json()
                .output()?
        } else {
            env.but(command).allow_json().output()?
        };
        assert!(
            output.status.success(),
            "uncommit --diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let result = if agent { &json["result"] } else { &json };
        assert_eq!(result["ok"], true);

        let changes = result["diff"]["changes"]
            .as_array()
            .expect("diff changes should be an array");
        assert!(
            changes
                .iter()
                .any(|change| change["path"].as_str() == Some("a.txt")),
            "diff output should include the uncommitted files"
        );

        if agent {
            assert!(
                json.get("status").is_some(),
                "agent JSON output should still include status"
            );
        }

        Ok(())
    }

    run(false)?;
    run(true)?;
    Ok(())
}

#[test]
fn uncommit_command_validation() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Test that uncommit rejects uncommitted files
    env.but("uncommit a.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit a.txt - it is an uncommitted file or hunk. Only commits and files-in-commits can be uncommitted.

"#]]);

    // Test that uncommit rejects branches
    env.but("uncommit A").assert().failure().stderr_eq(str![[r#"
Failed to uncommit. Cannot uncommit A - it is a branch. Only commits and files-in-commits can be uncommitted.

"#]]);
}

#[test]
fn uncommit_command_with_discard_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let commit_cli_ids_before = branch_commit_cli_ids(&before, "A");
    let source_cli_id = commit_cli_ids_before[0].clone();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1 create a.txt and b.txt
┊│     1:n A a.txt
┊│     1:p A b.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("uncommit {source_cli_id} --discard"))
        .assert()
        .success();

    let after = status_json(&env)?;
    let commit_cli_ids_after = branch_commit_cli_ids(&after, "A");

    assert_eq!(
        commit_cli_ids_after.len() + 1,
        commit_cli_ids_before.len(),
        "discarding a commit via uncommit should remove that commit from branch history"
    );
    assert!(
        !commit_cli_ids_after.contains(&source_cli_id),
        "source commit should no longer be present after discard"
    );
    assert!(
        !uncommitted_contains_file(&after, "a.txt") && !uncommitted_contains_file(&after, "b.txt"),
        "discarding a commit should not move its changes into uncommitted"
    );

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn uncommit_command_with_discard_on_committed_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env)?;
    let committed_file_cli_id = committed_file_cli_id_for_file(&before, "A", "b.txt")
        .expect("b.txt committed-file id should exist");

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1 create a.txt and b.txt
┊│     1:n A a.txt
┊│     1:p A b.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("uncommit {committed_file_cli_id} -d"))
        .assert()
        .success();

    let after = status_json(&env)?;
    assert!(
        !uncommitted_contains_file(&after, "b.txt"),
        "discarded committed file changes should not end up uncommitted"
    );
    assert!(
        !branch_commits_contain_file(&after, "A", "b.txt"),
        "discarded committed file changes should no longer be in commit history"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "a.txt"),
        "other committed file changes should remain in history"
    );

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   1 create a.txt and b.txt
┊│     1:n A a.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn uncommit_help_mentions_discard_flag() -> anyhow::Result<()> {
    let env = Sandbox::empty();

    let output = env.but("uncommit --help").output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains("-d, --discard"),
        "expected uncommit help to list the discard flag"
    );
    assert!(
        stdout.contains("Discard the selected committed changes"),
        "expected uncommit help to describe discard behavior"
    );

    Ok(())
}

#[test]
fn agent_uncommit_discard_multiple_sources_writes_single_json_with_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    let before = status_json(&env)?;
    let commit_cli_ids_before = branch_commit_cli_ids(&before, "A");
    let source_cli_ids = format!("{},{}", commit_cli_ids_before[0], commit_cli_ids_before[1]);

    let output = env
        .but(format!("--format json uncommit {source_cli_ids} --discard"))
        .env("AI_AGENT", "codex")
        .allow_json()
        .output()?;
    assert!(output.status.success());

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed["result"]["ok"], serde_json::json!(true));
    assert!(
        parsed.get("status").is_some(),
        "agent JSON wrapper should include status"
    );

    let after = status_json(&env)?;
    let commit_cli_ids_after = branch_commit_cli_ids(&after, "A");

    assert_eq!(
        commit_cli_ids_after.len() + 2,
        commit_cli_ids_before.len(),
        "discarding two commit sources should remove both from branch history"
    );
    assert!(
        !uncommitted_contains_file(&after, "a.txt") && !uncommitted_contains_file(&after, "b.txt"),
        "discarded commits should not move their changes into uncommitted"
    );

    Ok(())
}
