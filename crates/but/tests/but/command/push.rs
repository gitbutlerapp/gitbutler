use snapbox::str;

use super::util::sandbox_with_conflicted_commit;
use crate::utils::{CommandExt, Sandbox};

fn repo_with_unpushed_branch() -> Sandbox {
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

    env
}

fn shell_quote_path(path: &std::path::Path) -> String {
    shell_words::quote(&path.display().to_string()).into_owned()
}

fn repo_with_unpushed_single_branch() -> (Sandbox, std::path::PathBuf) {
    let env = Sandbox::open_with_default_settings("one-fork");
    env.but("config feature single-branch enable")
        .assert()
        .success();

    let remote = env.app_data_dir().join("origin with spaces.git");
    let remote_arg = shell_quote_path(&remote);
    env.invoke_bash(format!(
        "git clone -q --bare . {remote_arg} && git remote set-url origin {remote_arg}",
    ));

    env.file("unpushed.txt", "content\n");
    env.but("commit -m 'unpushed work'").assert().success();

    (env, remote)
}

fn assert_single_branch_status_before_push(env: &Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ma [main]
┊●   1 unpushed work
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but help` for all commands

"#]]);
}

fn assert_single_branch_status_after_push(env: &Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ma [main] (merged upstream)
┊●   1 unpushed work
┊●   nmy M (no changes)
├╯
┊
┊● d50ec84 (upstream: origin/main) 2 new commits
├╯ e31e6ca (common base) 2000-01-02 add init

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
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
fn pushes_an_explicit_checked_out_branch_in_single_branch_mode() {
    let (env, remote) = repo_with_unpushed_single_branch();
    let local_tip = env.invoke_git("rev-parse main");
    assert_single_branch_status_before_push(&env);

    env.but("push main").assert().success();
    assert_single_branch_status_after_push(&env);

    assert_eq!(
        env.invoke_git(&format!(
            "--git-dir={} rev-parse refs/heads/main",
            shell_quote_path(&remote)
        )),
        local_tip,
        "explicit push should update the checked-out branch on the remote"
    );
    assert!(
        env.open_repo()
            .try_find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .is_none(),
        "pushing must not create a managed workspace"
    );
}

#[test]
fn bare_push_pushes_the_checked_out_branch_in_single_branch_mode() {
    let (env, remote) = repo_with_unpushed_single_branch();
    let local_tip = env.invoke_git("rev-parse main");
    assert_single_branch_status_before_push(&env);

    env.but("push").assert().success();
    assert_single_branch_status_after_push(&env);

    assert_eq!(
        env.invoke_git(&format!(
            "--git-dir={} rev-parse refs/heads/main",
            shell_quote_path(&remote)
        )),
        local_tip,
        "bare push should select and update the checked-out branch"
    );
    assert!(
        env.open_repo()
            .try_find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .is_none(),
        "pushing must not create a managed workspace"
    );
}

/// An unreachable remote that is not the target's must not block a dry-run push:
/// `fetch_from_remotes` only fails when the target's own fetch remote failed.
#[test]
fn push_dry_run_ignores_unreachable_unrelated_remote() {
    let env = repo_with_unpushed_branch();
    env.invoke_git("remote add broken /nonexistent/path/broken.git");

    env.but("push --dry-run branchB").assert().success();
}

#[test]
fn push_dry_run_json_reports_remote_and_remote_ref() {
    let env = repo_with_unpushed_branch();
    configure_other_tracking_remote(&env);

    let output = env
        .but("push --dry-run --json branchB")
        .allow_json()
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "push --dry-run --json branchB failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
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
        String::from_utf8(bytes).unwrap()
    };
    assert_eq!(remote_ref, "refs/remotes/other/branchB");
}

#[test]
fn push_dry_run_agent_reports_human_summary() {
    let env = repo_with_unpushed_branch();

    let output = env
        .but("push --dry-run branchB")
        .env("PI_CODING_AGENT", "true")
        .output()
        .unwrap();
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
}

#[test]
fn push_uses_tracking_remote_when_branch_tracks_another_remote() {
    let env = repo_with_unpushed_branch();
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
}

#[test]
fn push_refuses_conflicted_commits() {
    let env = sandbox_with_conflicted_commit();

    // Try to push the branch - should fail with an error about conflicted commits.
    env.but("push A").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'A': the push would include 1 conflicted commit.
Conflicted commits: [..]
Please resolve conflicts before pushing using 'but resolve <commit>'.

"#]]);
}

#[test]
fn push_all_exits_nonzero_when_push_fails() {
    let env = repo_with_unpushed_branch();

    // Reject every push via a pre-push hook, then push without a branch
    // argument: the non-interactive push-all path must surface the failure
    // in the exit code, not just in the printed summary.
    env.invoke_bash(
        "mkdir -p .githooks \
         && printf '#!/bin/sh\\necho PRE-PUSH DENY; exit 1\\n' > .githooks/pre-push \
         && chmod +x .githooks/pre-push \
         && git config core.hooksPath .githooks",
    );

    // Counts are wildcarded: the claim is the non-zero exit on failure, not
    // how many candidates this fixture happens to produce.
    env.but("push").assert().failure().stderr_eq(str![[r#"
Error: failed to push [..] of [..] branches

"#]]);
}

#[test]
fn push_refuses_conflicted_commits_on_ancestors() {
    let env = sandbox_with_conflicted_commit();

    // Stack a clean branch on top of the conflicted one.
    env.but("branch new B --anchor A").assert().success();
    env.file("on-top.txt", "content\n");
    env.but("commit -b B -m 'work on top'").assert().success();

    // Pushing B also pushes its ancestor A, so A's conflicted commit must
    // refuse the push even though B itself is clean.
    env.but("push B").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'B': the push would include 1 conflicted commit.
Conflicted commits: [..]
Please resolve conflicts before pushing using 'but resolve <commit>'.

"#]]);
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
