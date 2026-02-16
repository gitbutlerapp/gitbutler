use snapbox::str;

use crate::{
    command::util::commit_two_files_as_two_hunks_each,
    utils::{CommandExt, Sandbox},
};

#[test]
fn move_commit_before_another_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    // Create three commits
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "c.txt", "d.txt", "second commit");
    commit_two_files_as_two_hunks_each(&env, "A", "e.txt", "f.txt", "third commit");

    // Get commit IDs from status
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits = &status_json["stacks"][0]["branches"][0]["commits"];

    // Commits are ordered newest first, so:
    // commits[0] = "third commit"
    // commits[1] = "second commit"
    // commits[2] = "first commit"
    // commits[3] = initial "add A" commit

    let third_commit_id = commits[0]["cliId"].as_str().unwrap();
    let first_commit_id = commits[2]["cliId"].as_str().unwrap();

    // Move "third commit" before "first commit" (making it the oldest)
    env.but(format!("move {} {}", third_commit_id, first_commit_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved [..] before [..]

"#]]);

    // Verify the move was successful by checking that we still have the right number of commits
    // (The actual reordering is tested in unit tests, here we just verify the command executed)
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits_after = &status_json["stacks"][0]["branches"][0]["commits"];

    // Should still have 4 commits (3 we created + initial "add A")
    assert_eq!(commits_after.as_array().unwrap().len(), 4);

    Ok(())
}

#[test]
fn move_commit_after_another_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    // Create three commits
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "c.txt", "d.txt", "second commit");
    commit_two_files_as_two_hunks_each(&env, "A", "e.txt", "f.txt", "third commit");

    // Get commit IDs
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits = &status_json["stacks"][0]["branches"][0]["commits"];

    let first_commit_id = commits[2]["cliId"].as_str().unwrap();
    let third_commit_id = commits[0]["cliId"].as_str().unwrap();

    // Move "first commit" after "third commit" (making it the newest)
    env.but(format!("move {} {} --after", first_commit_id, third_commit_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved [..] after [..]

"#]]);

    // Verify the move was successful by checking that we still have the right number of commits
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits_after = &status_json["stacks"][0]["branches"][0]["commits"];

    // Should still have 4 commits (3 we created + initial "add A")
    assert_eq!(commits_after.as_array().unwrap().len(), 4);

    Ok(())
}

#[test]
fn move_commit_to_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;

    // Create a commit on branch A
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "commit on A");

    // Get commit ID
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Move commit to branch B
    env.but(format!("move {} B", commit_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved [..] → [B]

"#]]);

    // Verify commit is now on branch B
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;

    // Branch A should have no commits (except the initial one)
    let branch_a_commits = &status_json["stacks"][0]["branches"][0]["commits"];
    assert_eq!(branch_a_commits.as_array().unwrap().len(), 1); // Only initial commit

    // Branch B should now have 2 commits (the moved one + initial "add B")
    let branch_b_commits = &status_json["stacks"][1]["branches"][0]["commits"];
    assert_eq!(branch_b_commits.as_array().unwrap().len(), 2);

    Ok(())
}

#[test]
fn move_commit_with_invalid_source() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    env.but("move nonexistent A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to move commit. Source 'nonexistent' not found. If you just performed a Git operation, try running 'but status' to refresh.

"#]]);

    Ok(())
}

#[test]
fn move_commit_with_after_flag_and_branch_target() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "commit on A");

    // Get commit ID
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Try to move commit to branch with --after flag (should error)
    env.but(format!("move {} B --after", commit_id))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to move commit. The --after flag only makes sense when moving a commit to another commit.
When moving to a branch, the commit is placed at the top of the stack by default.

"#]]);

    Ok(())
}

#[test]
fn move_same_commit_to_itself() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get commit ID
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Move commit to itself (should be no-op)
    env.but(format!("move {} {}", commit_id, commit_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Source and target are the same commit. Nothing to do.

"#]]);

    Ok(())
}

#[test]
fn move_commit_with_invalid_target() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    // Get commit ID
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commit_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    env.but(format!("move {} nonexistent", commit_id))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to move commit. Target 'nonexistent' not found. If you just performed a Git operation, try running 'but status' to refresh.

"#]]);

    Ok(())
}

#[test]
fn move_cross_stack_shows_helpful_error() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;

    env.setup_metadata(&["A", "B"])?;

    // Create commits on both branches
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "commit on A");
    commit_two_files_as_two_hunks_each(&env, "B", "c.txt", "d.txt", "commit on B");

    // Get commit IDs
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;

    let commit_a_id = status_json["stacks"][0]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();
    let commit_b_id = status_json["stacks"][1]["branches"][0]["commits"][0]["cliId"]
        .as_str()
        .unwrap();

    // Try to move commit from A to specific position in B (should show helpful error)
    env.but(format!("move {} {}", commit_a_id, commit_b_id))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to move commit. Cannot move commit to specific position in another stack

"#]]);

    Ok(())
}

#[test]
fn move_committed_file_to_another_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    // Create two commits with different files
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "c.txt", "d.txt", "second commit");

    // Get commit IDs and file IDs from status with files (-f flag)
    let status_output = env.but("--json status -f").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits = &status_json["stacks"][0]["branches"][0]["commits"];

    // commits[0] = "second commit" with c.txt and d.txt
    // commits[1] = "first commit" with a.txt and b.txt
    let first_commit_id = commits[1]["cliId"].as_str().unwrap();

    // Get the file ID for c.txt in the second commit
    // Files are under the "changes" array in the commit
    let c_txt_file_id = commits[0]["changes"]
        .as_array()
        .and_then(|changes| {
            changes
                .iter()
                .find(|change| change["filePath"].as_str() == Some("c.txt"))
        })
        .and_then(|change| change["cliId"].as_str())
        .expect("Could not find c.txt file ID in status output");

    // Move c.txt from second commit to first commit
    env.but(format!("move {} {}", c_txt_file_id, first_commit_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Moved files between commits!

"#]]);

    // Verify the file was moved by checking status again
    let status_output = env.but("--json status -f").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let commits = &status_json["stacks"][0]["branches"][0]["commits"];

    // After the move:
    // - The second commit should now only have d.txt
    // - The first commit should now have a.txt, b.txt, and c.txt
    let second_commit_changes = commits[0]["changes"].as_array().unwrap();
    let first_commit_changes = commits[1]["changes"].as_array().unwrap();

    assert_eq!(
        second_commit_changes.len(),
        1,
        "Second commit should have 1 file change after moving c.txt"
    );
    assert_eq!(
        first_commit_changes.len(),
        3,
        "First commit should have 3 file changes after receiving c.txt"
    );

    Ok(())
}
