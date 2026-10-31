use crate::utils::{CommandExt, Sandbox};

/// Show branch details for an applied branch using JSON output.
#[test]
fn show_lists_commits_ahead_for_applied_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied");
    env.setup_metadata(&["applied-branch"]);

    let result = env
        .but("--json branch show applied-branch")
        .allow_json()
        .output()
        .unwrap();

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    assert_eq!(json["branch"], "applied-branch");
    assert_eq!(json["commitsAhead"], 1);
    assert_eq!(json["commits"].as_array().unwrap().len(), 1);
    assert_eq!(json["commits"][0]["message"], "add applied.txt");
}

/// Report merge-check information for an applied branch using JSON output.
#[test]
fn show_check_reports_clean_merge_for_applied_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("pick-from-unapplied");
    env.setup_metadata(&["applied-branch"]);

    let result = env
        .but("--json branch show applied-branch --check")
        .allow_json()
        .output()
        .unwrap();

    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    assert_eq!(json["branch"], "applied-branch");
    assert_eq!(json["mergeCheck"]["mergesCleanly"], true);
    assert_eq!(
        json["mergeCheck"]["conflictingFiles"],
        serde_json::json!([])
    );
}

#[test]
fn show_works_for_unapplied_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch show A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

tpm add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("unapply A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Unapplied stack with 'A' from workspace

"#]]);

    env.but("branch show A")
        .assert()
        .stderr_eq(snapbox::str![[""]])
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

tpm add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn supports_short_codes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("branch show g0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

tpm add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn show_works_for_remote_only_branches() {
    let env = Sandbox::open_or_init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.invoke_bash(
        r#"
git checkout origin/main
git commit -m 'Add remote feature' --allow-empty
git update-ref refs/remotes/origin/remote-feature HEAD
git checkout gitbutler/workspace
"#,
    );

    env.but("branch show origin/remote-feature")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: origin/remote-feature (1 commits ahead)

uwy Add remote feature
    2000-01-02 00:00:00 by author
    0 files changed, 0 insertions, 0 deletions

"#]]);

    env.but("branch show remote-feature")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: remote-feature (1 commits ahead)

uwy Add remote feature
    2000-01-02 00:00:00 by author
    0 files changed, 0 insertions, 0 deletions

"#]]);
}

#[test]
fn showing_branch_that_isnt_top_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch show C")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: C (1 commits ahead)

wlx add C
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("branch show B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: B (1 commits ahead)

wwm add B
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);

    env.but("branch show A")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: A (1 commits ahead)

tpm add A
    2000-01-02 00:00:00 by author
    1 file changed, 1 insertion, 0 deletions

"#]]);
}

#[test]
fn show_only_lists_selected_branch_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("show B")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: B

Commits:
  wwm add B
    2000-01-02 00:00:00 by author

Stacked on:
  └─ A (1 commit)

"#]]);
}

#[test]
fn show_branch_bucket_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("show C --verbose --json")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "branch": "C",
  "commits": [
    {
      "sha": "aebb0903d514a4bda51b19a2c547b4333bf7847c",
      "short_sha": "aebb090",
      "cli_id": "wlx",
      "message": "add C",
      "full_message": "add C/n",
      "author_name": "author",
      "author_email": "author@example.com",
      "timestamp": 946771200,
      "files_changed": 1,
      "insertions": 1,
      "deletions": 0,
      "files": [
        {
          "path": "C",
          "status": "added",
          "insertions": 1,
          "deletions": 0
        }
      ]
    }
  ],
  "stackedOn": [
    {
      "name": "B",
      "commit_count": 1
    },
    {
      "name": "A",
      "commit_count": 1
    }
  ],
  "baseCommit": {
    "sha": "582f37b9cf8a4a20559396f5e73b3ace2e34bbcc",
    "short_sha": "582f37b",
    "cli_id": null,
    "message": "add B",
    "full_message": null,
    "author_name": "author",
    "author_email": "author@example.com",
    "timestamp": 946771200,
    "files_changed": null,
    "insertions": null,
    "deletions": null,
    "files": null
  }
}

"#]]);
    env.but("branch show B --files --json")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "branch": "B",
  "commitsAhead": 1,
  "commits": [
    {
      "sha": "582f37b9cf8a4a20559396f5e73b3ace2e34bbcc",
      "short_sha": "582f37b",
      "cli_id": "wwm",
      "message": "add B",
      "author_name": "author",
      "author_email": "author@example.com",
      "timestamp": 946771200,
      "files_changed": 1,
      "insertions": 1,
      "deletions": 0,
      "files": [
        {
          "path": "B",
          "status": "added",
          "insertions": 1,
          "deletions": 0
        }
      ]
    }
  ],
  "uncommittedFiles": [],
  "reviews": []
}

"#]]);
}

#[test]
fn show_empty_branch_bucket() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);
    env.but("branch new empty --above B").assert().success();

    env.but("show empty")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: empty

No commits on this branch.

Stacked on:
  ├─ B (1 commit)
  └─ A (1 commit)

"#]]);
    env.but("branch show empty")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Branch: empty (0 commits ahead)

No commits ahead of base branch.

"#]]);
}

#[test]
fn checking_merge_status_of_branch_that_isnt_top_of_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "one-stack-three-dependent-branches",
    );
    env.setup_metadata(&["A", "B", "C"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [C]
┊●   wlx add C
┊│
┊├┄ h0 [B]
┊●   wwm add B
┊│
┊├┄ i0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("branch show C --check").assert().success();

    env.but("branch show B --check").assert().success();

    env.but("branch show A --check").assert().success();
}
