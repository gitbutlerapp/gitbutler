use super::util::{enter_edit_mode_with_conflicted_commit, status_json};
use crate::utils::{CommandExt as _, Sandbox};
use snapbox::IntoData;

#[test]
fn worktrees() {
    let env = Sandbox::init_scenario_with_target_and_default_settings_slow("two-worktrees");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

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
fn anonymous_segment() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0
┊●   sxu anonymous (no changes)
┊│
┊├┄h0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn unborn() {
    let env = Sandbox::open_scenario_with_target_and_default_settings("unborn");
    snapbox::assert_data_eq!(env.git_log(), snapbox::str![""]);

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
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* 85efbe4 (HEAD -> main) M

"#]]
    );

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
      "cliId": "q",
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
              "cliId": "tpm",
              "changeId": "tpmktkqkknswxzyszlkxlrzoqorvpmur",
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
              "cliId": "lrm",
              "changeId": "lrmqkrvsuswuvvsnqpzqsoyswomkqvpw",
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
      "cliId": "n",
      "filePath": "a.txt",
      "changeType": "modified"
    },
    {
      "cliId": "p",
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
                  "cliId": "1#0:n",
                  "filePath": "a.txt",
                  "changeType": "modified"
                },
                {
                  "cliId": "1#0:p",
                  "filePath": "b.txt",
                  "changeType": "modified"
                }
              ]
...
              "message": "create-a-and-b",
...
              "changes": [
                {
                  "cliId": "1#1:n",
                  "filePath": "a.txt",
                  "changeType": "added"
                },
                {
                  "cliId": "1#1:p",
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
fn json_commit_cli_ids_use_change_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("commits-with-same-prefix");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A"]);

    // Assert that JSON exposes each full change ID and uses its display-padded prefix as CLI ID,
    // while retaining the underlying commit ID.
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
              "cliId": "usn",
              "changeId": "usnytowxypnotllltxmxrklxpksltzzr",
              "commitId": "5c88a8ec10067ef547f14b467776d3584cd683ea",
              "createdAt": "[RFC_TIMESTAMP]",
              "message": "add A13/n",
...
            {
              "cliId": "opy",
              "changeId": "opypvmowxsmlvxvktmlrnqwkywlwlrno",
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
              "cliId": "tvm",
              "changeId": "tvmyxqqsmtxrysurmzrxmylqrtmmxpyn",
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
┊●   lvx add A
├╯
┊
┊╭┄(upstream: origin/main) 1 new commit
┊● 67247ca add upstream-commit-message-that-is-intentionally-very-very-long-to-exc…
┊┊
├╯ 9fd740d (common base) 2000-01-02 add merge-base-message-that-is-intentio…

Hint: origin/main moved ahead; run `but pull` to update the workspace

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
┊●   nyq A-change
├╯
┊
┊╭┄h0 [B]
┊●   kyl B-change
├╯
┊
┊● 9354ac4 (upstream: origin/main) 2 new commits
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
┊● 55165db (upstream: origin/main) 1 new commit
├╯ 55165db (common base) 2000-01-02 merge document-but-pr-skill

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
┊╭┄to [top] (no commits)
┊│
┊├┄bo [bottom] (merged upstream) (no commits)
├╯
┊
┊● 334227d (upstream: origin/main) 1 new commit
├╯ 334227d (common base) 2000-01-02 merge bottom

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
┊●   nyq A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   kyl B-change
├╯
┊
┊╭┄(upstream: origin/main) 2 new commits
┊● 9354ac4 main-advance
┊● 756ee31 A-change
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
┊●   nyq A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   kyl B-change
├╯
┊
┊╭┄ex [extra-untracked] ○ empty (no commits)
├╯
┊
┊╭┄(upstream: origin/main) 2 new commits
┊● 9354ac4 main-advance
┊● 756ee31 A-change
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
┊●   nyq A-change
├╯
┊
┊╭┄h0 [B] [✓ upstream merges cleanly]
┊●   wxl B-change
┊│
┊├┄ma [main] (merged upstream)
┊●   upk M2
┊●   tpp M1
├╯
┊
┊╭┄(upstream: origin/main) 2 new commits
┊● ba5149e M2
┊● 6daac93 M1
┊┊
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
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
fn agent_status_explains_rewritten_commit_marker() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one.txt", "one\n");
    env.but("commit -m 'add one' -c A")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    env.invoke_git("update-ref refs/remotes/origin/A refs/heads/A");

    env.file("one.txt", "one amended\n");
    let target_commit = env.invoke_git("rev-parse --short refs/heads/A");
    env.but(format!("amend {target_commit} --changes one.txt"))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   [..] add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // The first agent-detected invocation also delivers the skill-install
    // notice ahead of the graph (the sandbox home has no skill installed).
    env.but("status")
        .env("AI_AGENT", "codex")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
⚠ AGENT ACTION REQUIRED: Install the GitButler skill before continuing.
Run now: but skill install
Then read the installed SKILL.md path printed by that command and continue.
Do not merely tell the user to run it.
This notice repeats until the skill is installed. If it still appears after installing, report it instead of retrying.

╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   [..] add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: ◐ means rewritten locally vs upstream.
Hint: commits are listed newest first. The first token on each line is the ID to use in commands.

"#]]);

    env.but("--format agent status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊◐   [..] add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: ◐ means rewritten locally vs upstream.
Hint: commits are listed newest first. The first token on each line is the ID to use in commands.

"#]]);
}

#[test]
fn conflicted_uncommitted_file_is_surfaced() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // Leave behind what a conflicting workspace update produces for an uncommitted
    // file: conflict markers in the worktree and unmerged entries in the index.
    env.file(
        "conflicted.txt",
        "<<<<<<< ours\nours\n=======\ntheirs\n>>>>>>> theirs\n",
    );
    env.invoke_bash(
        r#"base=$(echo base | git hash-object -w --stdin) &&
ours=$(echo ours | git hash-object -w --stdin) &&
theirs=$(echo theirs | git hash-object -w --stdin) &&
printf '100644 %s 1\tconflicted.txt\n100644 %s 2\tconflicted.txt\n100644 %s 3\tconflicted.txt\n' "$base" "$ours" "$theirs" | git update-index --index-info"#,
    );

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊    conflicted.txt {conflicted}
┊
┊╭┄g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M
⚠ Uncommitted file conflicts: choose the desired file state, then run `git add -- <path>`.

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        status_json(&env)?["conflictedFiles"],
        serde_json::json!(["conflicted.txt"]),
        "JSON status should list uncommitted files with unresolved index conflicts"
    );

    // Committing composes the oplog snapshot and the pre-commit hook index swap;
    // both must tolerate the unmerged index, and the conflict must survive.
    env.file("other.txt", "unrelated\n");
    env.but("commit A -m unrelated").assert().success();
    assert_eq!(
        env.invoke_git("ls-files --unmerged").lines().count(),
        3,
        "the index conflict survives the commit, including the hook index swap"
    );
    assert_eq!(
        env.invoke_git("show --name-only --format= A --"),
        "other.txt",
        "the commit contains only the unrelated file, not the conflicted one"
    );

    // Git already owns index conflict resolution; once marked resolved, the file
    // becomes an ordinary committable change and the warning disappears.
    env.file("conflicted.txt", "resolved\n");
    env.invoke_git("add -- conflicted.txt");
    assert_eq!(
        status_json(&env)?["conflictedFiles"],
        serde_json::Value::Null,
        "the conflict warning is gone after resolving"
    );
    assert!(
        status_json(&env)?["uncommittedChanges"]
            .as_array()
            .is_some_and(|changes| changes
                .iter()
                .any(|change| change["filePath"] == "conflicted.txt")),
        "the resolved file becomes an ordinary committable change"
    );

    Ok(())
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

#[test]
fn status_file_prefixed_with_persisted_or_synthetic_change_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("B", "Some content");
    env.invoke_git("config --local gitbutler.testing.changeId 1234");

    env.but("commit -m 'Commit with change ID'")
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   123 Commit with change ID
┊│     123:p A B
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn file_ids_are_nicely_aligned() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    for n in 0..10 {
        env.file(format!("file-{n}.txt"), "file #{n} content");
    }

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted]
┊   r  A file-0.txt
┊   k  A file-1.txt
┊   t  A file-2.txt
┊   v  A file-3.txt
┊   wx A file-4.txt
┊   wv A file-5.txt
┊   wk A file-6.txt
┊   x  A file-7.txt
┊   m  A file-8.txt
┊   zx A file-9.txt
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("commit -m 'add files'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   1 add files
┊│     1:r  A file-0.txt
┊│     1:k  A file-1.txt
┊│     1:t  A file-2.txt
┊│     1:v  A file-3.txt
┊│     1:wx A file-4.txt
┊│     1:wv A file-5.txt
┊│     1:wk A file-6.txt
┊│     1:x  A file-7.txt
┊│     1:m  A file-8.txt
┊│     1:z  A file-9.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // ensure verbose output is also nicely aligned
    env.but("status -vf")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 5877ef4)
┊│     add files
┊│     1:r  A file-0.txt
┊│     1:k  A file-1.txt
┊│     1:t  A file-2.txt
┊│     1:v  A file-3.txt
┊│     1:wx A file-4.txt
┊│     1:wv A file-5.txt
┊│     1:wk A file-6.txt
┊│     1:x  A file-7.txt
┊│     1:m  A file-8.txt
┊│     1:z  A file-9.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
