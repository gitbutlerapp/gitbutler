use snapbox::IntoData;
use snapbox::str;

use crate::command::util;
use crate::utils::{CommandExt, Sandbox};

fn pretty_status(env: &Sandbox) -> String {
    serde_json::to_string_pretty(&util::status_json(env)).unwrap()
}

fn raw_json_status(env: &Sandbox) -> String {
    let output = env.but("--json status").allow_json().output().unwrap();
    format!(
        "status={}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn install_editor_script(env: &Sandbox, script: &str) {
    env.file("editor.sh", script);
}

#[test]
fn integrate_pull_rebase_applies_and_snapshots_before_and_after() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   a952a0b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 643ade3 (A) add only-on-local
|/  
| * 28baf9a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        pretty_status(&env),
        snapbox::str![[r#"
{
  "uncommittedChanges": [],
  "stacks": [],
  "mergeBase": {
    "cliId": "",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "2000-01-01T00:00:00+00:00",
    "message": "add M\n",
    "authorName": "author",
    "authorEmail": "author@example.com",
    "conflicted": null,
    "reviewId": null,
    "changes": null
  },
  "upstreamState": {
    "behind": 0,
    "latestCommit": {
      "cliId": "",
      "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
      "createdAt": "2000-01-01T00:00:00+00:00",
      "message": "add M\n",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}
"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   6a3496e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 74faa12 (A) add only-on-local
| * 28baf9a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        pretty_status(&env),
        snapbox::str![[r#"
{
  "uncommittedChanges": [],
  "stacks": [],
  "mergeBase": {
    "cliId": "",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "2000-01-01T00:00:00+00:00",
    "message": "add M\n",
    "authorName": "author",
    "authorEmail": "author@example.com",
    "conflicted": null,
    "reviewId": null,
    "changes": null
  },
  "upstreamState": {
    "behind": 0,
    "latestCommit": {
      "cliId": "",
      "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
      "createdAt": "2000-01-01T00:00:00+00:00",
      "message": "add M\n",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}
"#]]
        .raw()
    );
}

/// A local commit whose changes the remote already carries (rewritten there, so a
/// different id) is not replayed on top of its remote twin: that would leave an
/// empty duplicate the user then has to notice and remove.
#[test]
fn integrate_pull_rebase_drops_local_commits_already_upstream() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-shared-commit");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   491ff23 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f3874b9 (A) add shared
|/  
| * 67ba860 (origin/A) add only-on-remote
| * 432e10e add shared
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   4540f60 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 67ba860 (origin/A, A) add only-on-remote
| * 432e10e add shared
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn integrate_pull_rebase_drops_local_commit_upstream_rebased_over_other_files() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-rebased-commit");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   787c31d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f3874b9 (A) add shared
|/  
| * fcc8313 (origin/A) add shared
| * e60aacd add other
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   49be156 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * fcc8313 (origin/A, A) add shared
| * e60aacd add other
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn integrate_pull_rebase_drops_local_commit_upstream_rebased_over_same_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "branch-integrate-rebased-same-file",
    );

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   75177f1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 4012e3a (A) change top
|/  
| * 865e8df (origin/A) change top
| * 63760d2 change bottom
|/  
* 0f34722 (origin/main, origin/HEAD, main) add f

"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   8f9b9d5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 865e8df (origin/A, A) change top
| * 63760d2 change bottom
|/  
* 0f34722 (origin/main, origin/HEAD, main, gitbutler/target) add f

"#]]
        .raw()
    );
}

#[test]
fn integrate_pull_rebase_keeps_a_change_reintroduced_after_its_upstream_twin() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-repeated-change");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   1abcf29 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * a8c56fd (A) add x
| * 9bcd67a remove x
| * d6cad7e add x
|/  
| * d3bd334 (origin/A) add only-on-remote
| * c4b69b2 add x
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    // The first `add x` is already upstream and drops out; `remove x` and the
    // second `add x` replay, so the tip still has `x`.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   dd445fe (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 93e8df2 (A) add x
| * 30055c3 remove x
| * d3bd334 (origin/A) add only-on-remote
| * c4b69b2 add x
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    assert_eq!(env.read_file("x").unwrap(), "x\n");
}

/// The integration replays a local merge as a plain commit on its first parent.
/// Even with nothing left once upstream has what it brought in, a merge stays.
#[test]
fn integrate_pull_rebase_keeps_local_merge_left_with_nothing() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-shared-merge");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   2810235 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * db3cbe0 (A) merge side
|/| 
| * f3874b9 (side) add shared
|/  
| * 67ba860 (origin/A) add only-on-remote
| * 432e10e add shared
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   fba0efe (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 82d7561 (A) merge side
| * 67ba860 (origin/A) add only-on-remote
| * 432e10e add shared
|/  
| * f3874b9 (side) add shared
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn integrate_smart_squash_applies_matching_change_ids() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-smart-squash");

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 2662ee8 (HEAD -> gitbutler/workspace, A) add only-on-local
| * c42227a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        raw_json_status(&env),
        snapbox::str![[r#"
status=exit status: 1
stdout:

stderr:
Error: GitButler mode exit required: please run `but teardown` to preserve your work.

"#]]
    );

    env.but("branch update A --strategy smart-squash")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* bf02b24 (HEAD -> gitbutler/workspace, A) add only-on-remote
| * c42227a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
    );
    snapbox::assert_data_eq!(
        raw_json_status(&env),
        snapbox::str![[r#"
status=exit status: 1
stdout:

stderr:
Error: GitButler mode exit required: please run `but teardown` to preserve your work.

"#]]
    );
}

#[test]
fn integrate_dry_run_shows_preview_without_changing_repo() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    let before_log = env.git_log();
    let before_status = pretty_status(&env);

    env.but("branch update A --dry-run")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Preview

* A
● sm 74faa12 add only-on-local
● __ 28baf9a add only-on-remote
o 0dc3733

"#]]);

    snapbox::assert_data_eq!(
        &before_log,
        snapbox::str![[r#"
*   a952a0b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 643ade3 (A) add only-on-local
|/  
| * 28baf9a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        &before_status,
        snapbox::str![[r#"
{
  "uncommittedChanges": [],
  "stacks": [],
  "mergeBase": {
    "cliId": "",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "2000-01-01T00:00:00+00:00",
    "message": "add M\n",
    "authorName": "author",
    "authorEmail": "author@example.com",
    "conflicted": null,
    "reviewId": null,
    "changes": null
  },
  "upstreamState": {
    "behind": 0,
    "latestCommit": {
      "cliId": "",
      "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
      "createdAt": "2000-01-01T00:00:00+00:00",
      "message": "add M\n",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}
"#]]
        .raw()
    );
    assert_eq!(env.git_log(), before_log, "dry-run must not rewrite refs");
    assert_eq!(
        pretty_status(&env),
        before_status,
        "dry-run must not change workspace status"
    );
}

#[test]
fn integrate_dry_run_verbose_shows_divergence_before_preview() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    let before_log = env.git_log();
    let before_status = pretty_status(&env);

    env.but("branch update A --dry-run --verbose")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Current state: A <- origin/A

● __ 643ade3 (A) add only-on-local
┊● __ 28baf9a (origin/A) add only-on-remote
├╯
o 0dc3733 add M

----------------------------

Preview

* A
● sm 74faa12 add only-on-local
● __ 28baf9a add only-on-remote
o 0dc3733

"#]]);

    assert_eq!(
        env.git_log(),
        before_log,
        "verbose dry-run must not rewrite refs"
    );
    assert_eq!(
        pretty_status(&env),
        before_status,
        "verbose dry-run must not change workspace status"
    );
}

#[test]
fn integrate_merge_dry_run_marks_conflicted_preview_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow(
        "branch-integrate-conflicting",
    );

    env.but("branch update A -s merge --dry-run")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Preview

* A
● uk d1f9d65 Merge dbf2a866824eab2a4c485b30bcfba70af8502900 into previous commit {conflicted}
● __ 57ca948 local change in A
o 6a997fd

"#]]);
}

#[test]
fn integrate_interactive_unchanged_script_applies_generated_plan() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    install_editor_script(&env, "#!/usr/bin/env bash\n: \"$1\"\n");

    env.but("branch update A --interactive")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   6a3496e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 74faa12 (A) add only-on-local
| * 28baf9a (origin/A) add only-on-remote
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn integrate_interactive_dry_run_keeps_repo_unchanged() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    install_editor_script(&env, "#!/usr/bin/env bash\n: \"$1\"\n");
    let before_log = env.git_log();
    let before_status = pretty_status(&env);

    env.but("branch update A --interactive --dry-run")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Preview

* A
● sm 74faa12 add only-on-local
● __ 28baf9a add only-on-remote
o 0dc3733

"#]]);

    assert_eq!(
        env.git_log(),
        before_log,
        "interactive dry-run must not rewrite refs"
    );
    assert_eq!(
        pretty_status(&env),
        before_status,
        "interactive dry-run must not change workspace status"
    );
}

#[test]
fn integrate_interactive_applies_edited_merge_plan() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    install_editor_script(
        &env,
        r#"#!/usr/bin/env bash
cat > "$1" <<'EOF'
pick 643ade3
merge 28baf9a
EOF
"#,
    );

    env.but("branch update A --interactive")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .success()
        .stderr_eq(str![]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   646aabb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| *   d1f1cff (A) Merge 28baf9a2794d7722ceff84f2967b5186545b8a48 into previous commit
| |\  
| | * 28baf9a (origin/A) add only-on-remote
| |/  
|/|   
| * 643ade3 add only-on-local
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
}

#[test]
fn integrate_interactive_fails_on_parse_error() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    install_editor_script(
        &env,
        r#"#!/usr/bin/env bash
printf 'drop 643ade3\n' > "$1"
"#,
    );
    let before_log = env.git_log();

    env.but("branch update A --interactive")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: line 1: unknown command 'drop'

"#]]);

    assert_eq!(
        env.git_log(),
        before_log,
        "parse failures must not rewrite refs"
    );
}

#[test]
fn integrate_interactive_fails_on_out_of_scope_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-diverged");
    install_editor_script(
        &env,
        r#"#!/usr/bin/env bash
printf 'pick 0dc3733\n' > "$1"
"#,
    );
    let before_log = env.git_log();

    env.but("branch update A --interactive")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: line 1: invalid pick commit: commit '0dc3733' is not part of the editable divergence

"#]]);

    assert_eq!(
        env.git_log(),
        before_log,
        "validation failures must not rewrite refs"
    );
}

#[test]
fn integrate_errors_cleanly_without_tracking_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-no-tracking");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 9477ae7 (A) add A
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
    );
    snapbox::assert_data_eq!(
        pretty_status(&env),
        snapbox::str![[r#"
{
  "uncommittedChanges": [],
  "stacks": [],
  "mergeBase": {
    "cliId": "",
    "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
    "createdAt": "2000-01-01T00:00:00+00:00",
    "message": "add M\n",
    "authorName": "author",
    "authorEmail": "author@example.com",
    "conflicted": null,
    "reviewId": null,
    "changes": null
  },
  "upstreamState": {
    "behind": 0,
    "latestCommit": {
      "cliId": "",
      "commitId": "0dc37334a458df421bf67ea806103bf5004845dd",
      "createdAt": "2000-01-01T00:00:00+00:00",
      "message": "add M\n",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}
"#]]
        .raw()
    );

    env.but("branch update A")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Branch 'refs/heads/A' has no tracking branch

"#]]);
}
