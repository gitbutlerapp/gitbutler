use snapbox::{IntoData as _, str};

use super::util::{
    add_worktree_with_commit, find_branch, sandbox_with_conflicted_commit, status_json,
};
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

/// [`repo_with_unpushed_branch()`] with a linked worktree on the new branch `wt`, with one
/// commit of its own, resting on the unpushed `branchB`.
fn repo_with_worktree_on_unpushed_branch() -> Sandbox {
    let env = repo_with_unpushed_branch();
    // Worktrees that predate the first read are adopted as archived.
    env.but("worktree list").assert().success();
    add_worktree_with_commit(&env, "wt", "branchB");
    env
}

#[test]
fn pushing_a_worktree_branch_pushes_what_it_rests_on_first() {
    let env = repo_with_worktree_on_unpushed_branch();

    env.but("push wt").assert().success().stdout_eq(str![[r#"

✓ Push completed successfully

  branchB -> origin/branchB ((new branch) -> 7566fe0)
  wt -> origin/wt ((new branch) -> 9da8421)

"#]]);
}

#[test]
fn bare_push_includes_worktree_lanes() {
    let env = repo_with_worktree_on_unpushed_branch();

    // Stacks push before worktrees, so `wt` finds `branchB` already current.
    env.but("push").assert().success().stdout_eq(str![[r#"

✓ Successfully pushed 3 commits

  branchB -> origin/branchB ((new branch) -> 7566fe0)
  A -> origin/A ((new branch) -> 9477ae7)
  wt -> origin/wt ((new branch) -> 9da8421)

"#]]);
}

fn shell_quote_path(path: &std::path::Path) -> String {
    shell_words::quote(&path.display().to_string()).into_owned()
}

fn repo_with_unpushed_single_branch() -> (Sandbox, std::path::PathBuf) {
    let env = Sandbox::open_with_default_settings("one-fork");

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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] [HEAD]
┊●   pxt unpushed work
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base, origin/main) 2000-01-02 add init

Hint: run `but help` for all commands

"#]]);
}

fn assert_single_branch_status_after_push(env: &Sandbox) {
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ ma [main] [HEAD] (merged upstream)
┊●   pxt unpushed work
┊●   nmy M (no changes)
├╯
┊
┊● d781bd0 (main, upstream: origin/main) 2 new commits
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
fn push_shows_gits_reason_when_the_remote_is_unreachable() {
    let env = repo_with_unpushed_branch();
    env.invoke_git("remote set-url origin /nonexistent/path/origin.git");

    env.but("push branchB").assert().failure().stderr_eq(str![[r#"
Error: fatal: '/nonexistent/path/origin.git' does not appear to be a git repository
fatal: Could not read from remote repository.

Please make sure you have the correct access rights
and the repository exists.

Hint: Check the remote's URL with `git remote -v`, your network connection and your credentials, then retry

"#]]);

    // A bare push lists what failed per branch, so the reason and the hint have to be there.
    env.but("push").assert().failure().stdout_eq(str![[r#"


✗ Failed to push 2 branches:
    branchB - fatal: '/nonexistent/path/origin.git' does not appear to be a git repository
fatal: Could not read from remote repository.

Please make sure you have the correct access rights
and the repository exists.

Hint: Check the remote's URL with `git remote -v`, your network connection and your credentials, then retry
    A - fatal: '/nonexistent/path/origin.git' does not appear to be a git repository
fatal: Could not read from remote repository.

Please make sure you have the correct access rights
and the repository exists.

Hint: Check the remote's URL with `git remote -v`, your network connection and your credentials, then retry

"#]]).stderr_eq(str![[r#"
Error: failed to push 2 of 2 branches

"#]]);
}

/// Git reached the remote and can't write the refs it fetched, because `origin/blocked` is in the
/// way of `origin/blocked/ref`. The remote isn't at fault, so this stays an internal error.
#[test]
fn a_fetch_that_cannot_write_its_refs_stays_an_internal_error() {
    let env = repo_with_unpushed_branch();
    let remote_git = env.app_data_dir().join("origin.git");
    env.invoke_git(&format!(
        "-C {} branch blocked/ref main",
        remote_git.display()
    ));
    env.invoke_git("update-ref refs/remotes/origin/blocked HEAD");

    env.but("push --dry-run branchB")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: fetching remote `origin` failed

Caused by:
    0: backend error: git command exited with non-zero exit code 1:
...
"#]]);
}

#[test]
fn push_dry_run_of_a_stacked_branch_lists_the_branches_beneath_it() {
    let env = repo_with_unpushed_branch();
    env.but("branch new branchC --above branchB")
        .assert()
        .success();
    env.file("on-top.txt", "content\n");
    env.but("commit -m 'on top' -b branchC").assert().success();

    // Pushing branchC pushes the unpushed branchB beneath it too, so the preview lists both.
    env.but("push --dry-run branchC")
        .assert()
        .success()
        .stdout_eq(str![[r#"

Dry run: Showing what would be pushed

Stack: branchC (2 branches)

┌─ Branch: branchC ↑ (on top of branchB)
│   → Would push to: origin/branchC
│   Commits: 1 unpushed commit
│ 
│     16a3787 on top
│
└─ Branch: branchB
    → Would push to: origin/branchB
    Commits: 1 unpushed commit
  
      7566fe0 first commit


Summary: Would push 2 commits across 2 branches

Run without --dry-run to push these changes.

"#]]);
    env.but("push branchC")
        .assert()
        .success()
        .stdout_eq(str![[r#"

✓ Push completed successfully

  branchB -> origin/branchB ((new branch) -> 7566fe0)
  branchC -> origin/branchC ((new branch) -> 16a3787)

"#]]);
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
        .as_agent()
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
fn push_makes_the_pushed_branch_the_upstream() {
    let env = repo_with_unpushed_branch();

    env.but("push branchB").assert().success();

    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref branchB@{upstream}"),
        "origin/branchB",
        "a first push tracks the remote branch, as `git push -u` does"
    );
}

#[test]
fn push_keeps_an_upstream_already_configured() {
    let env = repo_with_unpushed_branch();
    configure_other_tracking_remote(&env);

    env.but("push branchB").assert().success();

    assert_eq!(
        env.invoke_git("rev-parse --abbrev-ref branchB@{upstream}"),
        "other/branchB",
        "the user's own upstream stays"
    );
}

#[test]
fn push_refuses_conflicted_commits() {
    let env = sandbox_with_conflicted_commit();

    // The refusal is bad input: only that prints a hint, and it names the commit to start with.
    // A dry run gives the same refusal. `origin` is this repository, so the dry run's fetch
    // makes `A` look pushed, and the refusal can't depend on `A` having something to preview.
    for command in ["push A", "push A --dry-run", "--json push A --dry-run"] {
        env.but(command)
            .assert()
            .failure()
            .stdout_eq(str![])
            .stderr_eq(str![[r#"
Error: Cannot push branch 'A': the push would include 1 conflicted commit.
Conflicted commits: nyo

Hint: Start resolving with `but resolve conflicts nyo`, then push again

"#]]);
    }
    env.but("resolve conflicts nyo").assert().success();
}

/// [`sandbox_with_conflicted_commit()`] with a remote that lacks `A`. The fixture's own `origin`
/// is this repository, so a fetch would make `A` look pushed.
fn conflicted_commit_with_real_remote() -> Sandbox {
    let env = sandbox_with_conflicted_commit();
    let remote = env.app_data_dir().join("origin.git");
    env.invoke_bash(format!(
        "git clone -q --bare . {remote} && \
         git --git-dir={remote} update-ref -d refs/heads/A && \
         git remote set-url origin {remote}",
        remote = remote.display(),
    ));
    env
}

/// Three branches a bare push treats differently: `B`, stacked on the conflicted `A`, and the
/// clean `clean` in its own lane.
fn repo_with_conflicted_stack_and_clean_lane() -> Sandbox {
    let env = conflicted_commit_with_real_remote();
    env.but("branch new B --above A").assert().success();
    env.file("on-top.txt", "content\n");
    env.but("commit -b B -m 'work on top'").assert().success();
    env.but("branch new clean").assert().success();
    env.file("clean.txt", "content\n");
    env.but("commit -b clean -m 'clean work'")
        .assert()
        .success();
    env
}

#[test]
fn bare_push_dry_run_reports_refused_lanes_beside_clean_ones() {
    let env = repo_with_conflicted_stack_and_clean_lane();

    // The real push refuses `B`, which takes `A` with it, and still pushes `clean`.
    env.but("push --dry-run")
        .assert()
        .failure()
        .stdout_eq(str![[r#"

Dry run: Showing what would be pushed

Stack: B (2 branches)

┌─ Branch: B ↑ (on top of A)
│   → Would not push to: origin/B
│   Commits: 1 unpushed commit
│ 
│     2b888dc work on top

│   ⚠ Cannot push branch 'B': the push would include 1 conflicted commit.
│     Conflicted commits: nyo
│     
│     Hint: Start resolving with `but resolve conflicts nyo`, then push again
│
└─ Branch: A
    → Would not push to: origin/A
    Commits: 1 unpushed commit

    ⚠ Would not be pushed, because the push of 'B' would be refused.

    ⚡ Force push required


   Branch: clean
    → Would push to: origin/clean
    Commits: 1 unpushed commit

      13cfef2 clean work


"#]])
        .stderr_eq(str![[r#"
Error: would refuse to push 1 of 2 branches

"#]]);
    // A row without a `warning` is a branch that would really be pushed.
    env.but("--json push --dry-run")
        .assert()
        .failure()
        .stdout_eq(str![[r#"
{
  "branches": [
    {
      "branchName": "A",
...
      "warning": "Would not be pushed, because the push of 'B' would be refused."
    },
    {
      "branchName": "B",
...
      "warning": "Cannot push branch 'B': the push would include 1 conflicted commit.[..]then push again",
      "stackedOn": "A"
    },
    {
      "branchName": "clean",
...
      "requiresForce": false
    }
  ]
}

"#]]);
    env.but("push").assert().failure().stdout_eq(str![[r#"

✓ Successfully pushed 1 commit

  clean -> origin/clean ((new branch) -> 13cfef2)

✗ Failed to push 1 branch:
    B - Cannot push branch 'B': the push would include 1 conflicted commit.
Conflicted commits: nyo

Hint: Start resolving with `but resolve conflicts nyo`, then push again

"#]]);
}

#[test]
fn bare_push_dry_run_lists_a_branch_that_goes_out_through_a_clean_worktree() {
    let env = conflicted_commit_with_real_remote();
    env.but("branch new lower --below A").assert().success();
    env.file("lower.txt", "content\n");
    env.but("commit -b lower -m 'lower work'")
        .assert()
        .success();
    env.but("worktree list").assert().success();
    add_worktree_with_commit(&env, "W", "lower");

    // The push of `A` is refused, but `W` rests on `lower` and takes it along.
    env.but("push --dry-run")
        .assert()
        .failure()
        .stdout_eq(str![[r#"

Dry run: Showing what would be pushed

Stack: A (2 branches)

┌─ Branch: A ↑ (on top of lower)
│   → Would not push to: origin/A
│   Commits: 1 unpushed commit

│   ⚠ Cannot push branch 'A': the push would include 1 conflicted commit.
│     Conflicted commits: nyo
│     
│     Hint: Start resolving with `but resolve conflicts nyo`, then push again

│   ⚡ Force push required
│
└─ Branch: lower
    → Would push to: origin/lower
    Commits: 1 unpushed commit
  
      9883b65 lower work


   Branch: W
    → Would push to: origin/W
    Commits: 1 unpushed commit

      fc68cee add W


"#]]);
    env.but("push").assert().failure().stdout_eq(str![[r#"

✓ Successfully pushed 1 commit

  lower -> origin/lower ((new branch) -> 9883b65)
  W -> origin/W ((new branch) -> fc68cee)

✗ Failed to push 1 branch:
    A - Cannot push branch 'A': the push would include 1 conflicted commit.
Conflicted commits: nyo

Hint: Start resolving with `but resolve conflicts nyo`, then push again

"#]]);
}

#[test]
fn push_of_several_conflicted_commits_starts_resolving_at_the_oldest() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-conflicts-in-both-branches-of-stack",
    );
    env.setup_metadata_at_target(&["A"], "main");
    env.but("pull").assert().success();

    // Commits list newest first, and resolving a commit rebases those above it.
    env.but("push A").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'A': the push would include 2 conflicted commits.
Conflicted commits: wmr, trk

Hint: Start resolving with `but resolve conflicts trk`, then push again

"#]]);
}

#[test]
fn push_hint_for_conflicted_commits_works_with_a_branch_checked_out() {
    let env = Sandbox::open_scenario_with_target_and_default_settings(
        "single-branch-conflict-in-bottom-commit",
    );
    env.but("pull").assert().success();

    // `but resolve <commit>` refuses while a branch is checked out, so the hint can't name it.
    env.but("push A").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'A': the push would include 1 conflicted commit.
Conflicted commits: rou

Hint: Start resolving with `but resolve conflicts rou`, then push again

"#]]);
    env.but("resolve conflicts rou").assert().success();
}

#[test]
fn bare_push_reports_a_conflict_refusal_with_its_hint() {
    let env = sandbox_with_conflicted_commit();

    // An internal error reaches `main`, which prints a backtrace with this set. Bad input exits first.
    env.but("push")
        .env("RUST_BACKTRACE", "1")
        .assert()
        .failure()
        .stdout_eq(str![[r#"


✗ Failed to push 1 branch:
    A - Cannot push branch 'A': the push would include 1 conflicted commit.
Conflicted commits: nyo

Hint: Start resolving with `but resolve conflicts nyo`, then push again

"#]])
        .stderr_eq(str![[r#"
Error: failed to push 1 of 1 branch

"#]]);
    env.but("--json push")
        .assert()
        .failure()
        .stdout_eq(str![[r#"
{
  "pushed": [],
  "failed": [
    {
      "branchName": "A",
      "error": "Cannot push branch 'A': the push would include 1 conflicted commit.\nConflicted commits: nyo\n\nHint: Start resolving with `but resolve conflicts nyo`, then push again"
    }
  ]
}

"#]].raw());
}

#[test]
fn push_of_something_that_is_not_an_applied_branch_is_bad_input() {
    let env = repo_with_unpushed_branch();

    // Only bad input prints a hint. A dry run resolves the name the same way.
    for command in ["push nosuch", "push nosuch --dry-run"] {
        env.but(command).assert().failure().stderr_eq(str![[r#"
Error: Branch 'nosuch' not found. Available branches:
  - A
  - branchB

Hint: If 'nosuch' exists but isn't applied, run `but apply nosuch` first

"#]]);
    }
    let status = status_json(&env);
    let commit = find_branch(&status, "branchB")["commits"][0]["cliId"]
        .as_str()
        .expect("branchB has a commit");
    env.but(format!("push {commit}"))
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Expected branch identifier, got a commit.

Hint: Use a branch name from `but status`

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
    env.but("branch new B --above A").assert().success();
    env.file("on-top.txt", "content\n");
    env.but("commit -b B -m 'work on top'").assert().success();

    // Pushing B also pushes its ancestor A, so A's conflicted commit must
    // refuse the push even though B itself is clean.
    env.but("push B").assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'B': the push would include 1 conflicted commit.
Conflicted commits: [..]

Hint: Start resolving with `but resolve conflicts [..]`, then push again

"#]]);
}

#[test]
fn push_refuses_conflicted_commits_in_anonymous_segments_beneath_a_worktree() {
    let env = sandbox_with_conflicted_commit();
    env.but("worktree list").assert().success();
    add_worktree_with_commit(&env, "W", "A");
    env.invoke_git("update-ref refs/heads/A A~1");

    // No branch names the conflicted commit, so a dry run needs the push's whole scope too.
    for command in ["push W", "push W --dry-run"] {
        env.but(command).assert().failure().stderr_eq(str![[r#"
Error: Cannot push branch 'W': the push would include 1 conflicted commit.
Conflicted commits: [..]

Hint: Start resolving with `but resolve conflicts [..]`, then push again

"#]]);
    }
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

    // A dry run fetches, and this fixture's own `origin` doesn't exist.
    env.invoke_git("remote set-url origin .");
    env.but("push A --dry-run")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
    env.but("push A --dry-run --allow-merged")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success();
}

#[test]
fn push_gives_decent_error_without_remote() {
    let env = Sandbox::empty();
    env.but("init --commit").assert().success();

    env.but("commit -m 'new commit'").assert().success();

    env.but("push")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No push remote configured

"#]]);

    env.but("switch --workspace").assert().success();

    env.but("push")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No push remote configured

"#]]);
}
