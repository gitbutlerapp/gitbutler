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
        commit_two_files_as_two_hunks_each, status_json_with_files as status_json,
        uncommitted_contains_file,
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

#[test]
fn uncommit_different_files_from_the_same_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    // A single commit on branch A introducing two files. The CLI groups both
    // committed-file ids into one source for that commit.
    env.file("c1.txt", "c1 content\n");
    env.file("c2.txt", "c2 content\n");
    env.but("commit -b A -m 'add c1 and c2'").assert().success();

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add c1 and c2
┊│     1:l A c1.txt
┊│     1:w A c2.txt
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
    env.but(format!("uncommit {c1_id} {c2_id}"))
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Uncommitted from 191c6ed

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊   ls A c1.txt
┊   wy A c2.txt
┊
┊╭┄ g0 [A]
┊●   1 add c1 and c2 (no changes)
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

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

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
fn uncommit_command_on_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get the commit ID from status
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_cli_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Test uncommit command
    env.but(format!("uncommit {commit_cli_id}"))
        .assert()
        .success();

    // Verify the files are now uncommitted
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "added"
    },
    {
      "cliId": "pn",
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
