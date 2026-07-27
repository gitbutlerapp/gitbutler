use snapbox::str;

use super::util::sandbox_with_conflicted_commit;
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
    env.but("commit -m 'first commit' -b branchB")
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

    let output = env
        .but("push --dry-run branchB")
        .env("PI_CODING_AGENT", "true")
        .output()?;
    assert!(
        output.status.success(),
        "push --dry-run branchB failed: {}",
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
fn push_refuses_conflicted_commits() -> anyhow::Result<()> {
    let env = sandbox_with_conflicted_commit();

    // Try to push the branch - should fail with an error about conflicted commits.
    env.but("push A").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'A': the branch contains 1 conflicted commit.
Conflicted commits: [..]
Please resolve conflicts before pushing using 'but resolve <commit>'.

"#]]);

    Ok(())
}

#[test]
fn push_rejects_merged_upstream_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    // Branch A's content already landed on origin/main; pushing it would
    // publish finished work to a branch nobody needs anymore.
    env.but("push A")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check


"#]]);
}
