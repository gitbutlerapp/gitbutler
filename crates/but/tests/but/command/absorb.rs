use bstr::ByteSlice;
use snapbox::str;

use crate::{
    command::util::{self, commit_file_with_worktree_changes_as_two_hunks},
    utils::{CommandExt, Sandbox},
};

fn find_uncommitted_cli_id(status: &serde_json::Value, path: &str) -> Option<String> {
    status["uncommittedChanges"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|change| change["filePath"].as_str() == Some(path))
        .and_then(|change| change["cliId"].as_str().map(ToOwned::to_owned))
}

#[test]
fn unresolvable_source_errors_instead_of_absorbing_everything() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // A source that resolves to nothing must fail loudly. It used to be
    // swallowed, silently degrading `absorb <id>` to absorb-everything.
    env.but("absorb zq").assert().failure();

    // The worktree change is untouched.
    let status = util::status_json(&env);
    assert!(
        find_uncommitted_cli_id(&status, "a.txt").is_some(),
        "nothing was absorbed by the failed command"
    );
}

#[test]
fn ambiguous_source_errors_instead_of_absorbing_an_arbitrary_match() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");
    // "foo23" and "foo242" share the reverse-hex ID prefix "kp".
    env.file("foo23", "data\n");
    env.file("foo242", "data\n");

    env.but("absorb kp").assert().failure();

    // Nothing was absorbed by the ambiguous selector.
    let status = util::status_json(&env);
    assert!(
        find_uncommitted_cli_id(&status, "a.txt").is_some(),
        "the ambiguous command must not touch any commit"
    );
}

#[test]
fn uncommitted_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("--json status -f")
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
    }
  ],
...
"#]]);

    env.but("absorb")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: 1 a.txt
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4
    a.txt @6,4 +6,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    // Change was absorbed
    let repo = env.open_repo();
    let blob = repo.rev_parse_single(b"A:a.txt").unwrap().object().unwrap();
    snapbox::assert_data_eq!(
        blob.data.as_bstr().to_string(),
        snapbox::str![[r#"
firsta
line
line
line
line
line
line
line
lasta

"#]]
    );

    // Status is clean
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [],
...

"#]]);
}

#[test]
fn uncommitted_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Verify that the first hunk is nk:2, and absorb it.
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────────╮
 nk:2 a.txt │
────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +firsta
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

────────────╮
 nk:e a.txt │
────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -last
   ┊  9 │ +lasta

"#]]);
    env.but("absorb nk:2")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: 1 a.txt
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    // Change was partially absorbed
    let repo = env.open_repo();
    let blob = repo.rev_parse_single(b"A:a.txt").unwrap().object().unwrap();
    snapbox::assert_data_eq!(
        blob.data.as_bstr().to_string(),
        snapbox::str![[r#"
firsta
line
line
line
line
line
line
line
last

"#]]
    );

    // Status is not clean
    env.but("--json status -f")
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
    }
  ],
...

"#]]);
}

#[test]
fn committed_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────────╮
 nk:2 a.txt │
────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +firsta
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

────────────╮
 nk:e a.txt │
────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -last
   ┊  9 │ +lasta

"#]]);

    env.but("commit -b A -m 'partial change to a.txt 1'")
        .assert()
        .success();

    let context_distance = (env.app_settings().context_lines * 2 + 1) as usize;

    // Change the file at the top & commit
    env.file(
        "a.txt",
        format!("first\n{}lasta\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────────╮
 nk:f a.txt │
────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -firsta
  ┊ 1 │ +first
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

"#]]);

    env.but("commit -b A -m 'partial change to a.txt 2'")
        .assert()
        .success();

    // Change the file at the bottom & commit
    env.file(
        "a.txt",
        format!("first\n{}last\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────────╮
 nk:1 a.txt │
────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -lasta
   ┊  9 │ +last

"#]]);

    env.but("commit -b A -m 'partial change to a.txt 3'")
        .assert()
        .success();

    // Change the file at the top & bottom & absorb
    env.file(
        "a.txt",
        format!("first new\n{}last new\n", "line\n".repeat(context_distance)),
    );

    // Verify the hunks
    env.but("diff a.txt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
────────────╮
 nk:b a.txt │
────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +first new
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

────────────╮
 nk:5 a.txt │
────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -last
   ┊  9 │ +last new

"#]]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   nk M a.txt
┊
┊╭┄ g0 [A]
┊●   1#0 partial change to a.txt 3
┊│     1#0:n M a.txt
┊●   1#1 partial change to a.txt 2
┊│     1#1:n M a.txt
┊●   1#2 partial change to a.txt 1
┊│     1#2:n M a.txt
┊●   1#3 a.txt
┊│     1#3:n A a.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("absorb")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: 1 partial change to a.txt 2
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4

Absorbed to commit: 1 partial change to a.txt 3
  (files locked to commit due to hunk range overlap)
    a.txt @6,4 +6,4


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    env.but("stf")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 partial change to a.txt 3
┊│     1#0:n M a.txt
┊●   1#1 partial change to a.txt 2
┊│     1#1:n M a.txt
┊●   1#2 partial change to a.txt 1
┊│     1#2:n M a.txt
┊●   1#3 a.txt
┊│     1#3:n A a.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Change was full absorbed
    let repo = env.open_repo();
    let blob = repo.rev_parse_single(b"A:a.txt").unwrap().object().unwrap();
    snapbox::assert_data_eq!(
        blob.data.as_bstr().to_string(),
        snapbox::str![[r#"
first new
line
line
line
line
line
line
line
last new

"#]]
    );
}

#[test]
fn concurrent_absorb_of_independent_files_succeeds() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");
    commit_file_with_worktree_changes_as_two_hunks(&env, "B", "b.txt");

    let status = util::status_json(&env);
    let id_a = find_uncommitted_cli_id(&status, "a.txt").expect("should find a.txt CLI ID");
    let id_b = find_uncommitted_cli_id(&status, "b.txt").expect("should find b.txt CLI ID");

    let child_a = util::but_std_cmd(&env, &format!("absorb {id_a}"))
        .spawn()
        .unwrap();
    let child_b = util::but_std_cmd(&env, &format!("absorb {id_b}"))
        .spawn()
        .unwrap();

    let out_a = child_a.wait_with_output().unwrap();
    let out_b = child_b.wait_with_output().unwrap();

    assert!(
        out_a.status.success(),
        "absorb a.txt failed: {}",
        out_a.stderr.as_bstr()
    );
    assert!(
        out_b.status.success(),
        "absorb b.txt failed: {}",
        out_b.stderr.as_bstr()
    );

    let status = util::status_json(&env);
    assert_eq!(
        status["uncommittedChanges"]
            .as_array()
            .map(|changes| changes.len())
            .unwrap_or(0),
        0,
        "both files should be absorbed from the worktree"
    );
}

#[test]
fn dry_run_shows_plan_without_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Get initial status
    let initial_status = env
        .but("--json status -f")
        .allow_json()
        .output()
        .unwrap()
        .stdout;

    // Run absorb with dry-run flag
    env.but("absorb --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: 1 a.txt
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4
    a.txt @6,4 +6,4

Dry run complete. No changes were made.

"#]])
        .stderr_eq(str![""]);

    // Verify that no changes were actually made - status should be unchanged
    let post_dry_run_status = env
        .but("--json status -f")
        .allow_json()
        .output()
        .unwrap()
        .stdout;
    assert_eq!(
        initial_status, post_dry_run_status,
        "Status should be unchanged after dry-run"
    );

    // Also verify the workspace commit did NOT change during dry-run
    let repo = env.open_repo();
    let ws_id = repo
        .rev_parse_single(b"gitbutler/workspace")
        .unwrap()
        .detach();
    // Re-run dry-run and confirm workspace is still the same
    env.but("absorb --dry-run").assert().success();
    let ws_id_after = repo
        .rev_parse_single(b"gitbutler/workspace")
        .unwrap()
        .detach();
    assert_eq!(ws_id, ws_id_after, "dry-run must not touch workspace HEAD");

    // Verify the file content wasn't actually changed
    let repo = env.open_repo();
    let blob = repo.rev_parse_single(b"A:a.txt").unwrap().object().unwrap();
    snapbox::assert_data_eq!(
        blob.data.as_bstr().to_string(),
        snapbox::str![[r#"
first
line
line
line
line
line
line
line
last

"#]]
    );

    // Verify there are still uncommitted changes
    env.but("--json status -f")
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
    }
  ],
...

"#]]);
}

/// Regression test for https://github.com/gitbutlerapp/gitbutler/issues/12750
/// After absorb, the `gitbutler/workspace` HEAD must be refreshed so that
/// tools inspecting HEAD (e.g. pre-push hooks that stash against it) see
/// an up-to-date synthetic commit rather than a stale one.
#[test]
fn workspace_head_is_refreshed_after_absorb() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    // Record the workspace commit *before* absorb.
    let repo = env.open_repo();
    let ws_before = repo
        .rev_parse_single(b"gitbutler/workspace")
        .unwrap()
        .detach();

    env.but("absorb").assert().success().stderr_eq(str![""]);

    // After absorb the workspace commit must have changed.
    let ws_after = repo
        .rev_parse_single(b"gitbutler/workspace")
        .unwrap()
        .detach();

    assert_ne!(
        ws_before, ws_after,
        "gitbutler/workspace HEAD should be refreshed after absorb"
    );
}

#[test]
fn absorb_skips_merged_upstream_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    // This change depends on branch A's commit, whose content already landed
    // on origin/main; absorb must not amend it.
    env.file("file-a.txt", "change-A-modified\n");

    env.but("absorb")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Skipped: not absorbing into 756ee31 A-change: commit is merged upstream
Hint: most likely you want `but pull`, which removes landed work; in rare cases pass --allow-merged to absorb anyway
Nothing left to absorb

"#]]);
}

#[test]
fn absorb_json_reports_skipped_merged_upstream_commits_on_stderr() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file-a.txt", "change-A-modified\n");

    env.but("--json absorb")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "ok": false,
  "skippedMergedUpstream": [
    "756ee31783c2adf1542abe10ea254866d1464983"
  ]
}

"#]])
        .stderr_eq(str![[r#"
warning: skipped absorbing into 1 merged-upstream commit(s): 756ee31. Run `but pull` to update the workspace, or pass --allow-merged to absorb anyway.

"#]]);
}

#[test]
fn absorb_json_reports_partially_skipped_merged_upstream_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    // file-a.txt depends on branch A's landed commit (skipped); file-b.txt on
    // branch B's live commit (absorbed).
    env.file("file-a.txt", "change-A-modified\n");
    env.file("file-b.txt", "change-B-modified\n");

    env.but("--json absorb")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "ok": true,
  "rejected": 0,
  "plan": {
    "total_files": 1,
    "commits": [
      {
        "commit_id": "536958e9343fce0fa27fd4d51f88317cca5ff78f",
        "commit_summary": "B-change",
        "reason": "hunk_dependency",
        "reason_description": "files locked to commit due to hunk range overlap",
        "files": [
          {
            "path": "file-b.txt",
            "hunks": [
              "@1,1 +1,1"
            ]
          }
        ]
      }
    ]
  },
  "skippedMergedUpstream": [
    "756ee31783c2adf1542abe10ea254866d1464983"
  ]
}

"#]])
        .stderr_eq(str![[r#"
warning: skipped absorbing into 1 merged-upstream commit(s): 756ee31. Run `but pull` to update the workspace, or pass --allow-merged to absorb anyway.

"#]]);
}
