use snapbox::str;

use super::util::find_branch;
use crate::utils::{CommandExt, Sandbox};

#[test]
fn push_dry_run_json_reports_remote_and_remote_ref() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let remote_git = env.app_data_dir().join("origin.git");
    let remote_git = remote_git.display();
    env.invoke_bash(format!(
        "rm -rf {remote_git} && git clone --bare . {remote_git} && (git remote get-url origin >/dev/null 2>&1 && git remote set-url origin {remote_git} || git remote add origin {remote_git})",
    ));

    env.but("branch new branchB").assert().success();
    env.but("apply branchB").assert().success();

    env.file("test-file.txt", "line 1\nline 2\nline 3\n");
    env.but("commit -m 'first commit' branchB")
        .assert()
        .success();

    let output = env
        .but("push --dry-run --json branchB")
        .allow_json()
        .output()?;
    assert!(
        output.status.success(),
        "push --dry-run --json branchB failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let branches = json["branches"]
        .as_array()
        .unwrap_or_else(|| panic!("expected branches array in JSON output: {json:#}"));
    assert!(
        !branches.is_empty(),
        "expected at least one branch in dry-run JSON output: {json:#}"
    );
    let branch = &branches[0];

    assert_eq!(branch["branchName"], "branchB");
    assert_eq!(branch["remote"], "origin");
    let remote_ref = if let Some(remote_ref) = branch["remoteRef"].as_str() {
        remote_ref.to_owned()
    } else {
        let bytes = branch["remoteRef"]
            .as_array()
            .expect("expected remoteRef to serialize as a string or byte array")
            .iter()
            .map(|byte| {
                byte.as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .expect("remoteRef bytes should be valid u8 values")
            })
            .collect();
        String::from_utf8(bytes)?
    };
    assert_eq!(remote_ref, "refs/remotes/origin/branchB");

    Ok(())
}

#[test]
fn push_refuses_conflicted_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;

    let remote_git = env.app_data_dir().join("origin.git");
    let remote_git = remote_git.display();
    env.invoke_bash(format!(
        "rm -rf {remote_git} && git clone --bare . {remote_git} && (git remote get-url origin >/dev/null 2>&1 && git remote set-url origin {remote_git} || git remote add origin {remote_git})",
    ));

    // Create a new branch for our test
    env.but("branch new branchB").assert().success();

    // Create a file with initial content and commit it
    env.file("test-file.txt", "line 1\nline 2\nline 3\n");
    env.but("commit -m 'first commit' branchB")
        .assert()
        .success();

    // Add more content that depends on the first commit and commit again
    env.file("test-file.txt", "line 1\nline 2\nline 3\nline 4\n");
    env.but("commit -m 'second commit' branchB")
        .assert()
        .success();

    // Make origin a writable local repository for the push attempt.
    // Get the first commit's CLI ID from status
    let status_output = env.but("--json status").allow_json().output()?;
    let status_json: serde_json::Value = serde_json::from_slice(&status_output.stdout)?;
    let branch = find_branch(&status_json, "branchB")?;
    let first_commit_id = branch["commits"]
        .as_array()
        .and_then(|commits| {
            commits
                .iter()
                .find(|commit| commit["message"].as_str() == Some("first commit"))
        })
        .and_then(|commit| commit["cliId"].as_str())
        .expect("should have first commit cliId");

    // Rub the first commit to unassigned (zz) - this should create a conflict
    // in the second commit since it depends on the first
    env.but(format!("rub {first_commit_id} zz"))
        .assert()
        .success();

    // Try to push the branch - should fail with an error about conflicted commits
    env.but("push branchB")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot push branch 'branchB': the branch contains 1 conflicted commit.
Conflicted commits: [..]
Please resolve conflicts before pushing using 'but resolve <commit>'.

"#]]);

    Ok(())
}
