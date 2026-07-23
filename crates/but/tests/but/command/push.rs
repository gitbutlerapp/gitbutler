use snapbox::str;

use super::util::find_branch;
use crate::utils::{CommandExt, Sandbox};

fn repo_with_unpushed_branch() -> anyhow::Result<Sandbox> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

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

    Ok(env)
}

fn configure_other_tracking_remote(env: &Sandbox) -> std::path::PathBuf {
    let remote_base = env.invoke_git("rev-parse refs/heads/branchB^");
    let other = env.app_data_dir().join("other.git");
    env.invoke_bash(format!(
        "rm -rf {other} && git clone -q --bare . {other} && \
         git remote add other {other} && \
         git config branch.branchB.remote other && \
         git config branch.branchB.merge refs/heads/branchB && \
         git --git-dir={other} update-ref refs/heads/branchB {remote_base} && \
         git update-ref refs/remotes/other/branchB {remote_base}",
        other = other.display(),
    ));
    other
}

#[test]
fn push_dry_run_json_reports_remote_and_remote_ref() -> anyhow::Result<()> {
    let env = repo_with_unpushed_branch()?;
    configure_other_tracking_remote(&env);

    let output = env
        .but("push --dry-run --format json branchB")
        .allow_json()
        .output()?;
    assert!(
        output.status.success(),
        "push --dry-run --format json branchB failed: {}",
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
    assert_eq!(branch["remote"], "other");
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
    assert_eq!(remote_ref, "refs/remotes/other/branchB");

    Ok(())
}

#[test]
fn push_dry_run_agent_reports_human_summary() -> anyhow::Result<()> {
    let env = repo_with_unpushed_branch()?;

    let output = env.but("push --dry-run --format agent branchB").output()?;
    assert!(
        output.status.success(),
        "push --dry-run --format agent branchB failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Dry run:") && stdout.contains("Run without --dry-run"),
        "agent dry-run push should print the human summary, got: {stdout}"
    );
    assert!(
        output.stderr.is_empty(),
        "agent dry-run push should not print progress, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[test]
fn push_uses_tracking_remote_when_branch_tracks_another_remote() -> anyhow::Result<()> {
    let env = repo_with_unpushed_branch()?;
    let local_tip = env.invoke_git("rev-parse refs/heads/branchB");
    let other = configure_other_tracking_remote(&env);

    env.but("push branchB").assert().success();

    assert_eq!(
        env.invoke_git(&format!(
            "--git-dir={} rev-parse refs/heads/branchB",
            other.display()
        )),
        local_tip,
        "push should update the branch's tracking remote"
    );

    Ok(())
}

#[test]
fn push_prints_associated_review_link() -> anyhow::Result<()> {
    let env = repo_with_unpushed_branch()?;
    let ctx = env.context();
    let mut cache = ctx.db.get_cache_mut()?;
    but_forge::cache_review(
        &mut cache,
        &but_forge::ForgeReview {
            html_url: "https://example.com/pull/42".to_string(),
            number: 42,
            title: String::new(),
            body: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "branchB".to_string(),
            target_branch: "main".to_string(),
            sha: String::new(),
            integration_commit_shas: vec![],
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: Default::default(),
        },
    )?;
    drop(cache);

    env.but("push branchB")
        .assert()
        .success()
        .stdout_eq(str![[r#"

✓ Push completed successfully

  branchB -> origin/branchB ((new branch) -> [..])
  Updated review #42 at https://example.com/pull/42

"#]]);

    Ok(())
}

#[test]
fn push_refuses_conflicted_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata_at_target(&["A"], "origin/main");

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
    let status_output = env.but("--format json status").allow_json().output()?;
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

    // Rub the first commit to uncommitted (zz) - this should create a conflict
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
