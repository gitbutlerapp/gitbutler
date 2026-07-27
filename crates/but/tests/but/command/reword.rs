use gitbutler_commit::commit_ext::CommitMessageBstr as _;
use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn reword_commit_with_message_flag() {
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

    // Use reword with -m flag to change commit message.
    env.but("reword tpm -m 'Updated commit message'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..]

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 95614cf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* dfe058b (A) Updated commit message
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

/// Commits with a change ID are identified by it, matching how `but status`
/// displays them.
#[test]
fn reword_commit_with_change_id_shows_change_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("new.txt", "content\n");
    env.but("commit -b A -m 'add new.txt'").assert().success();

    env.but("reword 1 -m 'reworded'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for 1

"#]]);
}

#[test]
fn reword_commit_with_multiline_message() -> anyhow::Result<()> {
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

    // Use reword with multiline message
    env.but("reword tpm -m 'First line\n\n\tSecond paragraph with details'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..]

"#]]);

    // Verify the commit message was updated with multiline content
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 71a85cb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 3ffa6ce (A) First line
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );

    let repo = env.open_repo();
    assert_eq!(
        repo.rev_parse_single(":/First line")?
            .object()?
            .into_commit()
            .message_bstr(),
        "First line\n\n\tSecond paragraph with details"
    );

    Ok(())
}

// Note: Branch rename test is omitted because the test scenario uses single-character
// branch names ("A") which don't meet the 2-character minimum requirement for CLI IDs.
// The branch rename functionality with -m flag is tested manually and works correctly.
#[test]
fn reword_branch_from_editor_trims_trailing_newlines_in_confirmation_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");

    env.setup_metadata(&["A"]);
    env.but("branch new branch-to-rename-123")
        .assert()
        .success();

    env.file(
        "editor.sh",
        "#!/usr/bin/env bash\nprintf 'renamed-branch\\n\\n' > \"$1\"\n",
    );
    env.but("reword branch-to-rename-123")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Renamed branch 'branch-to-rename-123' to 'renamed-branch'

"#]]);
}

#[test]
fn reword_branch_rejects_head() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new branch-to-rename").assert().success();

    env.but("reword branch-to-rename -m HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);
}

#[test]
fn reword_branch_rejects_non_normalized_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("reword A -m /B")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Bad input '/B'

Invalid branch name

Hint: Try 'B' instead

"#]]);
}

#[test]
fn reword_branch_rejects_branch_name_that_already_exists() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch new existing").assert().success();

    env.but("reword A -m existing")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: A branch named 'existing' is already applied

"#]]);
}

#[test]
fn reword_branch_allows_rewording_to_same_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("reword A -m A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Branch already named 'A' - nothing to do

"#]])
        .stderr_eq(str![[]]);
}

#[test]
fn reword_commit_with_same_message_succeeds_as_noop() {
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

    // Try to reword with the same message
    env.but("reword tpm -m 'add A'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
No changes to commit message - nothing to be done

"#]]);
}

#[test]
fn reword_commit_with_json_flag() {
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

    // Use reword with -m flag to change commit message (using commit ID)
    env.but("reword tpm -m 'Updated commit message' --format json")
        .assert()
        .success()
        .stdout_eq(str![[r#"{
  "new_commit_id": [..]
}

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 95614cf (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* dfe058b (A) Updated commit message
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
}

#[test]
fn reword_commit_json_can_request_status_after() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let output = env
        .but("reword tpm -m 'Updated commit message' --format json --status-after")
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;

    assert!(
        json["result"]["new_commit_id"].is_string(),
        "status output must retain the reword result"
    );
    assert!(
        json["status"]["stacks"].is_array(),
        "status-after must include the workspace with fresh commit IDs"
    );

    Ok(())
}
