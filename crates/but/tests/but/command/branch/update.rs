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
  "stacks": [
    {
      "cliId": "h0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
              "cliId": "smn",
              "changeId": "smnwyxurtyqkwkkqslklkvwzxxksotoy",
              "commitId": "74faa12600297e7b9e8363028de23c69f1db1c14",
              "createdAt": "2000-01-01T00:00:00+00:00",
              "message": "add only-on-local\n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            },
            {
              "cliId": "yxu",
              "changeId": "yxuympxptyrpmltqvkxykkswvvlloxql",
              "commitId": "28baf9a2794d7722ceff84f2967b5186545b8a48",
              "createdAt": "2000-01-01T00:00:00+00:00",
              "message": "add only-on-remote\n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            }
          ],
          "upstreamCommits": [],
          "branchStatus": "unpushedCommits",
          "reviewId": null,
          "ci": null
        }
      ]
    }
  ],
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

#[test]
fn integrate_remote_merge_brings_its_merged_files_into_the_worktree() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("branch-integrate-remote-merge");

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    // `A` was only behind `origin/A`, so it now points at the remote merge itself.
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   f444733 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| *   15e2a6d (origin/A, A) Merge feat into A
| |\  
| | * 20c2b59 (origin/feat) add guard
| |/  
| * 4f93c21 add lane
|/  
* 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M

"#]]
        .raw()
    );
    assert!(
        env.projects_root().join("guard").is_file(),
        "the file brought in by the remote merge is checked out"
    );
}

#[test]
fn integrate_remote_branch_that_merged_a_newer_target() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "branch-integrate-remote-merged-newer-target",
    );
    env.setup_metadata_at_target(&["A", "B"], "main");

    // The merge of a newer `main` and the commit on top exist only on `origin/A`.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊╭┄┄ (upstream: on origin/A)
┊●   200e51e add A2-by-collaborator
┊●   db3fa13 Merge newer main into A
┊●   c39f3a2 add main3
┊●   9f62045 add main2
┊-
┊●   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    // `A` is level with `origin/A`, while the workspace base stays at the old `main`.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   mtx add A2-by-collaborator
┊●   mun Merge newer main into A
┊●   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    let a_after_update = env.invoke_git("rev-parse A");
    let origin_a = env.invoke_git("rev-parse origin/A");
    let newer_main_files_after_update =
        ["main2", "main3"].map(|file| env.projects_root().join(file).is_file());

    env.but("pull").assert().success();

    // The cutoff moves to the newer `main` and `A` is rebased onto it, keeping its merge.
    // The workspace's old base parent keeps its displayed common base at `add M`.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊◐   mtx add A2-by-collaborator
┊◐   mun Merge newer main into A (no changes)
┊◐   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        a_after_update, origin_a,
        "a branch that is only behind takes the remote history as is"
    );
    assert_eq!(
        newer_main_files_after_update,
        [true, true],
        "main2 and main3 from the newer main arrive through the merge"
    );
    assert_eq!(
        env.invoke_git("rev-parse A~1^2"),
        env.invoke_git("rev-parse origin/main"),
        "after the pull, the merge still merges the newer main, now the base"
    );
}

#[test]
fn integrate_remote_branch_that_merged_a_newer_target_below_a_local_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "branch-integrate-remote-merged-newer-target-with-local-commit",
    );
    env.setup_metadata_at_target(&["A", "B"], "main");

    // `A` has a commit the remote lacks, and `origin/A` has the merge of a newer `main`.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊╭┄┄ (upstream: on origin/A)
┊●   200e51e add A2-by-collaborator
┊●   db3fa13 Merge newer main into A
┊●   c39f3a2 add main3
┊●   9f62045 add main2
┊-
┊●   qul add A-local
┊●   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch update A")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Updated branch A.

"#]]);

    // The local commit is rebased on top of the remote history, which arrives as is.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   qul add A-local
┊●   mtx add A2-by-collaborator
┊●   mun Merge newer main into A
┊●   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    let below_local_after_update = env.invoke_git("rev-parse A~1");
    let origin_a = env.invoke_git("rev-parse origin/A");
    let local_message = env.invoke_git("log -1 --format=%s A");
    let files_after_update =
        ["main2", "main3", "A-local"].map(|file| env.projects_root().join(file).is_file());

    env.but("pull").assert().success();

    // The cutoff moves to the newer `main`; the merge and the local commit stay in `A`.
    // The workspace's old base parent keeps its displayed common base at `add M`.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   qul add A-local
┊◐   mtx add A2-by-collaborator
┊◐   mun Merge newer main into A (no changes)
┊◐   zpl add A1
├╯
┊
┊╭┄ h0 [B]
┊●   kvk add B1
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        below_local_after_update, origin_a,
        "the local commit sits directly on the unchanged remote history"
    );
    assert_eq!(
        local_message, "add A-local",
        "the local commit stays at the tip of `A`"
    );
    assert_eq!(
        files_after_update,
        [true, true, true],
        "the newer main's files arrive and the local change is kept"
    );
    assert_eq!(
        env.invoke_git("rev-parse A~2^2"),
        env.invoke_git("rev-parse origin/main"),
        "after the pull, the merge still merges the newer main, now the base"
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
