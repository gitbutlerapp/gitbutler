use super::util::{enter_edit_mode_with_conflicted_commit, status_json};
use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn worktrees() {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    insta::assert_snapshot!(env.git_log(), @r"
    *   063d8c1 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 3e01e28 (B) B
    * | 4c4624e (A) A
    |/  
    | * 8dc508f (origin/main, origin/HEAD, main) M-advanced
    |/  
    | * 197ddce (origin/A) A-remote
    |/  
    * 081bae9 M-base
    * 3183e43 M1
    ");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/two-worktrees/status-with-worktrees.stdout.term.svg"
        ]);

    env.but("status --verbose")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/two-worktrees/status-with-worktrees-verbose.stdout.term.svg"
        ]);
}

#[test]
fn unborn() {
    let env = Sandbox::open_scenario_with_target_and_default_settings("unborn");
    insta::assert_snapshot!(env.git_log(), @"");

    // TODO: make this work
    env.but("status --verbose")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Setup required: No GitButler project found at . - run `but setup` to configure the project

"#]]);
}

#[test]
fn first_commit_no_workspace() {
    let env = Sandbox::open_scenario_with_target_and_default_settings("first-commit");
    insta::assert_snapshot!(env.git_log(), @"* 85efbe4 (HEAD -> main) M");

    // TODO: make this work
    env.but("status --verbose")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Setup required: No GitButler project found at . - run `but setup` to configure the project

"#]]);
}

#[test]
fn remote_and_local_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("remote-local-divergence");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["main", "A"]);

    // Under branch A, remote-only and local-only commits and files are shown.
    // CLI IDs are shown only for local-only files.
    env.but("status --files")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/remote-and-local-files.stdout.term.svg"
        ]);
}

#[test]
fn json_shows_paths_as_strings() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"]);

    // Create a new file to ensure we have file assignments
    env.file("test-file.txt", "test content");

    env.but("--format json status")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "qu",
      "filePath": "test-file.txt",
      "changeType": "added"
    }
  ],
  "stacks": [
    {
      "cliId": "j0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
              "cliId": "94",
              "commitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
              "createdAt": "2000-01-01T00:00:00+00:00",
              "message": "add A/n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            }
          ],
          "upstreamCommits": [],
          "branchStatus": "completelyUnpushed",
          "reviewId": null,
          "ci": null
        }
      ]
    },
    {
      "cliId": "k0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "h0",
          "name": "B",
          "commits": [
            {
              "cliId": "d3",
              "commitId": "d3e2ba36c529fbdce8de90593e22aceae21f9b17",
              "createdAt": "2000-01-01T00:00:00+00:00",
              "message": "add B/n",
              "authorName": "author",
              "authorEmail": "author@example.com",
              "conflicted": false,
              "reviewId": null,
              "changes": null
            }
          ],
          "upstreamCommits": [],
          "branchStatus": "completelyUnpushed",
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
    "message": "add M/n",
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
      "message": "add M/n",
      "authorName": "author",
      "authorEmail": "author@example.com",
      "conflicted": null,
      "reviewId": null,
      "changes": null
    },
    "lastFetched": null
  }
}

"#]]);
}

// TODO This test demonstrates how IDs are assigned to uncommitted and committed
// files that have multiple hunks. This test can be removed when we have CLI
// IDs for hunks, a command (e.g. `rub`) is taught to use them, and that command
// is tested.
#[test]
fn uncommitted_and_committed_file_cli_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"]);

    env.file("a.txt", format!("first\n{}last\n", "line\n".repeat(100)));
    env.file("b.txt", "only\n");
    env.but("commit A -m create-a-and-b").assert().success();
    env.file("a.txt", format!("firsta\n{}lasta\n", "line\n".repeat(100)));
    env.file("b.txt", "onlya\n");
    env.but("commit A -m edit-a-and-b").assert().success();
    env.file("a.txt", format!("firstb\n{}lastb\n", "line\n".repeat(100)));
    env.file("b.txt", "onlyb\n");

    env.but("--format json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "nk",
      "filePath": "a.txt",
      "changeType": "modified"
    },
    {
      "cliId": "pn",
      "filePath": "b.txt",
      "changeType": "modified"
    }
  ],
  "stacks": [
...
              "message": "edit-a-and-b",
...
              "changes": [
                {
                  "cliId": "44:nk",
                  "filePath": "a.txt",
                  "changeType": "modified"
                },
                {
                  "cliId": "44:pn",
                  "filePath": "b.txt",
                  "changeType": "modified"
                }
              ]
...
              "message": "create-a-and-b",
...
              "changes": [
                {
                  "cliId": "49:nk",
                  "filePath": "a.txt",
                  "changeType": "added"
                },
                {
                  "cliId": "49:pn",
                  "filePath": "b.txt",
                  "changeType": "added"
                }
              ]
...

"#]]);

    Ok(())
}

#[test]
fn long_file_cli_ids_are_aligned() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"]);

    // foo1 has a CLI ID of length 2; the others have length 3
    env.file("foo1", "contents");
    env.file("foo23", "contents");
    env.file("foo242", "contents");

    // Even with differing lengths, the IDs are aligned
    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/long-file-cli-ids-are-aligned.stdout.term.svg"
        ]);
}

#[test]
fn long_cli_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"]);

    // For "add A13" and "add A3", the IDs have 3 characters. The others have 2.
    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/long-cli-ids.stdout.term.svg"
        ]);
}

#[test]
fn long_cli_ids_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"]);

    // Assert a handful of commits to show that the commit CLI IDs become longer
    // if a short ID would be ambiguous, but remain at 2 characters otherwise.
    env.but("--format json status -f")
        .allow_json()
        .with_assert(env.assert_with_uuid_and_timestamp_redactions())
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
...
          "commits": [
            {
              "cliId": "5c8",
              "commitId": "5c88a8ec10067ef547f14b467776d3584cd683ea",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A13/n",
...
            {
              "cliId": "a1",
              "commitId": "a18ea48cd317c7c8fc9317b6f2427be4cdb2585d",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A12/n",
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
...
            {
              "cliId": "5c7",
              "commitId": "5c7c6d7f3854bb61978b410b1ae8146be9948b26",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A3/n",
...

"#]]);

    Ok(())
}

#[test]
fn status_hint_with_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("new-file.txt", "content");

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/hints/status-hint-with-uncommitted-changes.stdout.term.svg"
        ]);
}

#[test]
fn status_hint_clean_workspace() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/hints/status-hint-clean-workspace.stdout.term.svg"
        ]);
}

#[test]
fn status_hint_when_no_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("unapply A").assert().success();

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/hints/status-hint-no-branches.stdout.term.svg"
        ]);
}

#[test]
fn status_no_hint_flag_suppresses_hint() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status --no-hint")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/hints/status-no-hint.stdout.term.svg"
        ]);
}

#[test]
fn status_shows_no_commits_label_for_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/classification/status-shows-no-commits-label.stdout.term.svg"
        ]);
}

#[test]
fn status_upstream_merge_status_empty() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty");
    env.setup_metadata(&["A", "B"]);

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-merge-status-empty.stdout.term.svg"
        ]);
}

#[test]
fn status_upstream_summary_without_flag() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-many-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-summary.stdout.term.svg"
        ]);
}

#[test]
fn status_upstream_detailed_with_flag() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-many-commits");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-detailed.stdout.term.svg"
        ]);
}

#[test]
fn status_upstream_detailed_truncates_after_8() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-many-commits");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-truncates-after-8.stdout.term.svg"
        ]);
}

#[test]
fn status_upstream_and_merge_base_messages_truncate_when_unpaged() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-long-messages");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] [✓ upstream merges cleanly]
┊●   601614c add A
├╯
┊
┊╭┄(upstream) ⏫ 1 commit
┊● 67247ca add upstream-commit-message-that-is-intentionally-very-very-long-to-exc…
┊┊
├╯ 9fd740d (common base) 2000-01-02 add merge-base-message-that-is-intentio…

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn status_upstream_merge_status_integrated() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-merge-status-integrated.stdout.term.svg"
        ]);
}

#[test]
fn status_marks_merged_upstream_without_upstream_flag() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (merged upstream)
┊●   756ee31 A-change
├╯
┊
┊╭┄h0 [B]
┊●   536958e B-change
├╯
┊
┊● 9354ac4 (upstream) ⏫ 2 commits
├╯ efc9211 (common base) 2000-01-02 base

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

#[test]
fn status_marks_empty_remote_branch_merged_upstream() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-merged-empty-branch");

    env.but("apply origin/document-but-pr-skill")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Applied remote branch 'origin/document-but-pr-skill' to workspace

"#]]);

    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄do [document-but-pr-skill] (merged upstream) (no commits)
├╯
┊
┊● 55165db (upstream) ⏫ 1 commit
├╯ 55165db (common base) 2000-01-02 merge document-but-pr-skill

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    assert_pull_removes_merged_upstream_branch(&env);
}

#[test]
fn status_marks_empty_remote_branch_merged_upstream_when_tip_matches_target() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-merged-empty-branch-ff");
    env.set_target_sha("refs/heads/base");

    env.but("apply origin/document-but-pr-skill")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Applied remote branch 'origin/document-but-pr-skill' to workspace

"#]]);

    let output = env
        .but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8_lossy(&output);
    assert!(
        output.contains("[document-but-pr-skill] (merged upstream) (no commits)"),
        "the fast-forward merged branch should be labelled as merged upstream:\n{output}"
    );

    assert_pull_removes_merged_upstream_branch(&env);
}

/// An empty branch stacked on top of a branch that merged upstream must not be treated
/// as merged itself: it contributed no commits of its own. Regression test for `but status`
/// labelling it `(merged upstream)` and `but pull` deleting the whole stack (including the
/// unmerged top branch) because every branch was wrongly classified as integrated.
#[test]
fn unmerged_empty_branch_above_merged_one_is_not_treated_as_merged() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "upstream-merged-branch-below-empty-branch",
    );
    env.setup_metadata(&["bottom"]);
    // Stack `top` directly above `bottom` so they form a single two-branch stack.
    {
        use but_core::RefMetadata as _;
        use std::ops::DerefMut as _;
        let mut meta = env.meta();
        let ws_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
        let mut ws = meta.workspace(ws_ref).unwrap();
        ws.deref_mut()
            .insert_new_segment_above_anchor_if_not_present(
                "refs/heads/top".try_into().unwrap(),
                "refs/heads/bottom".try_into().unwrap(),
            );
        meta.set_workspace(&ws).unwrap();
    }

    // `bottom` merged upstream; `top` rests on it and must not be labelled merged.
    env.but("status")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄op [top] (no commits)
┊│
┊├┄bo [bottom] (merged upstream) (no commits)
├╯
┊
┊● 334227d (upstream) ⏫ 1 commit
├╯ 334227d (common base) 2000-01-02 merge bottom

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);

    env.invoke_git("remote set-url origin .");
    env.but("pull").env("NO_BG_TASKS", "1").assert().success();

    let branches: Vec<String> = status_json(&env).unwrap()["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .map(|b| b["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        branches.iter().any(|b| b == "top"),
        "`but pull` must keep the unmerged `top` branch, got: {branches:?}"
    );
}

/// A branch whose only commit introduces no changes of its own, stacked on top of a
/// branch that was *squash-merged* upstream, must not be treated as merged itself: it
/// contributed nothing that was merged. Regression test for the data-loss bug where the
/// squash-merge trial let the no-change top commit "borrow" the cumulative content of the
/// squash-merged `bottom` below it, so `but status` labelled `top` `(merged upstream)` and
/// `but pull` deleted the whole stack — losing the unmerged `top` branch. The genuinely
/// squash-merged `bottom` must still be detected and removed.
#[test]
fn no_change_commit_above_squash_merged_branch_is_not_treated_as_merged() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "upstream-squash-merged-below-no-change-branch",
    );
    env.setup_metadata(&["bottom"]);
    // Stack `top` directly above `bottom` so they form a single two-branch stack.
    {
        use but_core::RefMetadata as _;
        use std::ops::DerefMut as _;
        let mut meta = env.meta();
        let ws_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into().unwrap();
        let mut ws = meta.workspace(ws_ref).unwrap();
        ws.deref_mut()
            .insert_new_segment_above_anchor_if_not_present(
                "refs/heads/top".try_into().unwrap(),
                "refs/heads/bottom".try_into().unwrap(),
            );
        meta.set_workspace(&ws).unwrap();
    }

    // `bottom` was squash-merged upstream and must be labelled `(merged upstream)`.
    // `top`'s sole commit introduces no changes, so it must NOT be labelled merged.
    let status = env
        .but("status --format json")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: serde_json::Value = serde_json::from_slice(&status).unwrap();
    let branch_status_of = |name: &str| -> String {
        status["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
            .find(|b| b["name"].as_str() == Some(name))
            .and_then(|b| b["branchStatus"].as_str())
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        branch_status_of("bottom"),
        "integrated",
        "`bottom` was squash-merged upstream and must be detected as integrated"
    );
    assert_ne!(
        branch_status_of("top"),
        "integrated",
        "`top`'s no-change commit must NOT be treated as integrated"
    );

    env.invoke_git("remote set-url origin .");
    env.but("pull").env("NO_BG_TASKS", "1").assert().success();

    let branches: Vec<String> = status_json(&env).unwrap()["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().into_iter().flatten())
        .map(|b| b["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        branches.iter().any(|b| b == "top"),
        "`but pull` must keep the unmerged `top` branch, got: {branches:?}"
    );
    assert!(
        !branches.iter().any(|b| b == "bottom"),
        "`but pull` must remove the genuinely squash-merged `bottom` branch, got: {branches:?}"
    );
}

fn assert_pull_removes_merged_upstream_branch(env: &Sandbox) {
    env.invoke_git("remote set-url origin .");
    env.but("pull").env("NO_BG_TASKS", "1").assert().success();

    let status_after = env
        .but("status --format json")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after: serde_json::Value = serde_json::from_slice(&status_after).unwrap();
    assert_eq!(
        status_after["stacks"].as_array().unwrap().len(),
        0,
        "the merged upstream branch should be removed by `but pull`"
    );
}

/// Like `status_upstream_merge_status_integrated`, but the fixture adds two
/// extra branches (`extra-untracked`, `extra-untracked-2`) that point at `base`
/// and are NOT registered in workspace metadata.
///
/// Setup (fixture `upstream-integrated-with-extra-branch`):
/// - Branches `A` and `B` each have one commit on top of `base`.
/// - `origin/main` has advanced past `base` with a cherry-pick of A plus
///   a `main-advance` commit.
/// - `extra-untracked` and `extra-untracked-2` point at `base` with no
///   commits of their own.
/// - Only `A` and `B` are registered in `setup_metadata`.
///
/// Expected: both extra branches are pruned entirely.
#[test]
fn status_upstream_prunes_untracked_integrated_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow(
        "upstream-integrated-with-extra-branch",
    );
    // Only register A and B — `extra-untracked` is deliberately omitted.
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (merged upstream)
┊●   756ee31 A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   536958e B-change
├╯
┊
┊╭┄(upstream) ⏫ 2 commits
┊● 9354ac4 main-advance
┊● 756ee31 A-change
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

/// Same fixture as `status_upstream_prunes_untracked_integrated_branch`, but
/// `extra-untracked` is now registered in `setup_metadata` (simulating
/// auto-discovery), while `extra-untracked-2` remains unregistered.
///
/// Expected: `extra-untracked` is kept (metadata-tracked), `extra-untracked-2`
/// is pruned (not tracked).
#[test]
fn status_upstream_prunes_metadata_tracked_integrated_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow(
        "upstream-integrated-with-extra-branch",
    );
    // Register A, B, and extra-untracked (simulating auto-discovery).
    // extra-untracked-2 remains unregistered.
    env.setup_metadata_at_target(&["A", "B", "extra-untracked"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] (merged upstream)
┊●   756ee31 A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   536958e B-change
├╯
┊
┊╭┄ex [extra-untracked] ○ empty (no commits)
├╯
┊
┊╭┄(upstream) ⏫ 2 commits
┊● 9354ac4 main-advance
┊● 756ee31 A-change
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

/// Two branches with different merge bases against the target.
///
/// Setup (fixture `upstream-different-bases`):
/// - `A` forks from `base` with one commit.
/// - `origin/main` has two commits on top of `base`: `M1` and `M2`.
/// - `B` forks from `M2` (the current `origin/main` tip) with one commit.
///
/// The graph walk starts from the lowest common base (`base`), so B's stack
/// includes `M1` and `M2`. Since both stacks are metadata-tracked they are
/// not pruned — `M1` and `M2` appear in B's stack as integrated commits,
/// to be cleaned up by `integrate_upstream`.
#[test]
fn status_upstream_prunes_with_different_bases() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings_slow("upstream-different-bases");
    env.setup_metadata(&["A", "B"]);
    // This test wants the target sha to be the common ancestor ancestor of the
    // workspace.
    env.set_target_sha("refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A] [✓ upstream merges cleanly]
┊●   756ee31 A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   594a02c B-change
┊│
┊├┄ma [main] (merged upstream)
┊●   ba5149e M2
┊●   6daac93 M1
├╯
┊
┊╭┄(upstream) ⏫ 2 commits
┊● ba5149e M2
┊● 6daac93 M1
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

/// Simulate a `git fetch` that advances `origin/main` after the workspace
/// commit was created.
///
/// Setup (fixture `upstream-advanced-after-workspace`):
/// - `A` and `B` each have one commit on top of `base`.
/// - The workspace commit was created when `origin/main` pointed at `base`.
/// - A fetch then advances `origin/main` by two commits (`first-advance`,
///   `second-advance`) that are *not* ancestors of the workspace commit.
/// - `old-integrated` points at `first-advance` and is added to A's stack
///   metadata (simulating auto-discovery).
///
/// Expected: `old-integrated` must NOT appear in any workspace stack, because
/// its tip is only reachable from the new target (post-fetch), not from the
/// workspace commit.
#[test]
fn status_upstream_advanced_target_does_not_leak_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow(
        "upstream-advanced-after-workspace",
    );
    env.setup_metadata(&["A", "B"]);

    // Add old-integrated to A's stack in metadata, simulating auto-discovery
    // before the branch was integrated upstream.
    {
        use but_core::RefMetadata;
        use std::ops::DerefMut;
        let mut meta = env.meta();
        let ws_ref: &gix::refs::FullNameRef = but_core::WORKSPACE_REF_NAME.try_into()?;
        let mut ws = meta.workspace(ws_ref)?;
        ws.deref_mut()
            .insert_new_segment_above_anchor_if_not_present(
                "refs/heads/old-integrated".try_into()?,
                "refs/heads/A".try_into()?,
            );
        meta.set_workspace(&ws)?;
    }

    let output = env
        .but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    // old-integrated must NOT appear in any workspace stack
    assert!(
        !stdout.contains("old-integrated"),
        "old-integrated should not appear in workspace stacks, but got:\n{stdout}"
    );

    Ok(())
}

#[test]
fn status_upstream_merge_status_conflicted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("upstream-conflicted");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");

    env.but("status --upstream")
        .env("NO_BG_TASKS", "1")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/upstream/status-upstream-merge-status-conflicted.stdout.term.svg"
        ]);
}

#[test]
fn status_shows_pushed_commit_marker() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("status-pushed");
    env.setup_metadata(&["A"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/classification/status-shows-pushed-commit-marker.stdout.term.svg"
        ]);
}

#[test]
fn status_shows_rewritten_branch_with_remote_and_local_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("status-modified");
    env.setup_metadata(&["A"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/classification/status-shows-rewritten-branch-with-remote-and-local-commits.stdout.term.svg"
        ]);
}

#[test]
fn status_in_edit_mode_delegates_to_resolve_status() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::file![
            "snapshots/status/edit-mode/status-delegates-to-resolve-status.stdout.term.svg"
        ]);

    Ok(())
}
