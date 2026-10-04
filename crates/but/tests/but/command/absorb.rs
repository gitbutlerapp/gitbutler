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
    env.but("absorb zq")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Source 'zq' not found. If you just performed a Git operation (squash, rebase, etc.), try running 'but status' to refresh the current state.

"#]]);

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

    env.but("absorb kp")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: 'kp' is ambiguous - it matches more than one uncommitted change. Use more characters to disambiguate.

"#]]);

    // Nothing was absorbed by the ambiguous selector.
    let status = util::status_json(&env);
    assert!(
        find_uncommitted_cli_id(&status, "a.txt").is_some(),
        "the ambiguous command must not touch any commit"
    );
}

#[test]
fn absorbs_ancestor_changes_before_descendant_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("absorb-parent-before-child");
    env.setup_metadata_at_target(&["B", "A"], "origin/main");
    let padding = "/* padding */\n".repeat(12);
    let source = format!(
        "static unsigned long domain_reduce_node_claims(void)\n\
         {{\n\
         \x20   unsigned long released = 0;\n\
         \x20   unsigned int node;\n\
         \n\
         \x20   for ( node = 0; node < 8; ++node )\n\
         \x20       released += node;\n\
         \n\
         \x20   return released;\n\
         }}\n\
         \n\
         {padding}\
         void release_claims(void)\n\
         {{\n\
         \x20   domain_reduce_node_claims();\n\
         }}\n\
         \n\
         {padding}\
         void unset_claims(void)\n\
         {{\n\
         \x20   domain_reduce_node_claims();\n\
         }}\n\
         \n\
         {padding}\
         void redeem_claims(void)\n\
         {{\n\
         \x20   domain_reduce_node_claims();\n\
         }}\n"
    );
    env.file("claims.c", source);

    env.but("absorb")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: ryy add claim release call sites
  (files locked to commit due to hunk range overlap)
    claims.c @32,7 +23,7
    claims.c @49,7 +40,7

Absorbed to commit: qkt redeem claims during allocation
  (files locked to commit due to hunk range overlap)
    claims.c @1,21 +1,12
    claims.c @66,5 +57,5


Hint: you can run `but undo` to undo these changes

"#]])
        .stderr_eq(str![""]);

    let status = util::status_json(&env);
    assert_eq!(
        status["uncommittedChanges"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
        0,
        "the changes to both commits should be absorbed"
    );

    let parent_diff = env.invoke_git("show A --format= -- claims.c");
    assert!(
        parent_diff.contains("+    domain_reduce_node_claims();"),
        "the ancestor's call sites should be absorbed into A; commit diff:\n{parent_diff}"
    );
    let child_diff = env.invoke_git("show B --format= -- claims.c");
    assert!(
        child_diff.contains("+static unsigned long domain_reduce_node_claims(void)"),
        "the descendant's helper rename should be absorbed into B:\n{child_diff}"
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

Absorbed to commit: pxx a.txt
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
──────────────╮
 nk:2 M a.txt │
──────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +firsta
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

──────────────╮
 nk:e M a.txt │
──────────────╯

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

Absorbed to commit: pxx a.txt
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
──────────────╮
 nk:2 M a.txt │
──────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +firsta
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

──────────────╮
 nk:e M a.txt │
──────────────╯

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
──────────────╮
 nk:f M a.txt │
──────────────╯

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
──────────────╮
 nk:1 M a.txt │
──────────────╯

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
──────────────╮
 nk:b M a.txt │
──────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +first new
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

──────────────╮
 nk:5 M a.txt │
──────────────╯

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
╭┄ @ [uncommitted]
┊   nk M a.txt
┊
┊╭┄ g0 [A]
┊●   sll partial change to a.txt 3
┊│     sll:n M a.txt
┊●   sol partial change to a.txt 2
┊│     sol:n M a.txt
┊●   rzm partial change to a.txt 1
┊│     rzm:n M a.txt
┊●   pxx a.txt
┊│     pxx:n A a.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("absorb")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Found 1 changed file to absorb:

Absorbed to commit: sol partial change to a.txt 2
  (files locked to commit due to hunk range overlap)
    a.txt @1,4 +1,4

Absorbed to commit: sll partial change to a.txt 3
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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   sll partial change to a.txt 3
┊│     sll:n M a.txt
┊●   sol partial change to a.txt 2
┊│     sol:n M a.txt
┊●   rzm partial change to a.txt 1
┊│     rzm:n M a.txt
┊●   pxx a.txt
┊│     pxx:n A a.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

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

Absorbed to commit: pxx a.txt
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
    // The refreshed workspace commit merges the rewritten stack tips.
    let mut parents: Vec<_> = repo
        .find_commit(ws_after)
        .unwrap()
        .parent_ids()
        .map(|id| id.detach())
        .collect();
    let mut tips =
        ["A", "B"].map(|branch| repo.rev_parse_single(branch.as_bytes()).unwrap().detach());
    parents.sort();
    tips.sort();
    assert_eq!(
        parents, tips,
        "workspace commit parents must be the current stack tips"
    );
    // The workspace tree carries the absorbed content, so tools inspecting
    // HEAD see the amended state rather than a stale one.
    let blob = repo
        .rev_parse_single(b"gitbutler/workspace:a.txt")
        .unwrap()
        .object()
        .unwrap();
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
        .failure()
        .stdout_eq(str![[r#"
Skipped: not absorbing into 756ee31 A-change: commit is merged upstream
Hint: most likely you want `but pull`, which removes landed work; in rare cases pass --allow-merged to absorb anyway
Nothing left to absorb

"#]])
    .stderr_eq(str![[r#"
Error: Cannot absorb selected changes because every target commit is merged upstream

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
        .failure()
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
Error: Cannot absorb selected changes because every target commit is merged upstream

"#]]);
}

#[test]
fn absorb_json_reports_blocked_mixed_merged_upstream_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    // file-a.txt depends on branch A's landed commit; file-b.txt depends on
    // branch B's live commit. Selecting both blocks the whole invocation.
    env.file("file-a.txt", "change-A-modified\n");
    env.file("file-b.txt", "change-B-modified\n");

    env.but("--json absorb")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![[r#"
{
  "ok": false,
  "skippedMergedUpstream": [
    "756ee31783c2adf1542abe10ea254866d1464983"
  ],
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
  }
}

"#]])
        .stderr_eq(str![[r#"
warning: skipped absorbing into 1 merged-upstream commit(s): 756ee31. Run `but pull` to update the workspace, or pass --allow-merged to absorb anyway.
Error: Cannot absorb selected changes because at least one target commit is merged upstream

"#]]);
}

#[test]
fn absorb_selected_landed_target_blocks_other_selected_targets() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file-a.txt", "change-A-modified\n");
    env.file("file-b.txt", "change-B-modified\n");

    let repo = env.open_repo();
    let a_before = repo.rev_parse_single(b"refs/heads/A").unwrap().detach();
    let b_before = repo.rev_parse_single(b"refs/heads/B").unwrap().detach();
    let a_worktree_before = env.read_file("file-a.txt").unwrap();
    let b_worktree_before = env.read_file("file-b.txt").unwrap();
    let output = env
        .but("--json absorb")
        .allow_json()
        .env("NO_BG_TASKS", "1")
        .output()
        .unwrap();
    let stdout = output.stdout.as_bstr();
    assert!(
        stdout.contains_str(b"file-b.txt") && stdout.contains_str(b"skippedMergedUpstream"),
        "the eligible plan and selected landed target must be observable at the filter boundary: {stdout}"
    );
    let a_after = repo.rev_parse_single(b"refs/heads/A").unwrap().detach();
    let b_after = repo.rev_parse_single(b"refs/heads/B").unwrap().detach();
    let a_worktree_after = env.read_file("file-a.txt").unwrap();
    let b_worktree_after = env.read_file("file-b.txt").unwrap();
    assert!(
        !output.status.success()
            && a_after == a_before
            && b_after == b_before
            && a_worktree_after == a_worktree_before
            && b_worktree_after == b_worktree_before,
        "a selected landed target must block the full invocation; status={:?}, \
             A: {a_before:?} -> {a_after:?}, B: {b_before:?} -> {b_after:?}, \
             worktree A: {a_worktree_before:?} -> {a_worktree_after:?}, \
             worktree B: {b_worktree_before:?} -> {b_worktree_after:?}, stdout={stdout}",
        output.status,
    );
}

#[test]
fn dry_run_with_selected_landed_target_keeps_state_unchanged() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file-a.txt", "change-A-modified\n");
    env.file("file-b.txt", "change-B-modified\n");

    let repo = env.open_repo();
    let refs_before = ["refs/heads/A", "refs/heads/B"].map(|reference| {
        repo.rev_parse_single(reference.as_bytes())
            .unwrap()
            .detach()
    });
    let status_before = util::status_json(&env);
    env.but("absorb --dry-run")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Cannot absorb selected changes because at least one target commit is merged upstream

"#]]);
    let refs_after = ["refs/heads/A", "refs/heads/B"].map(|reference| {
        repo.rev_parse_single(reference.as_bytes())
            .unwrap()
            .detach()
    });
    assert_eq!(
        refs_after, refs_before,
        "blocked dry-run does not move refs"
    );
    assert_eq!(
        util::status_json(&env),
        status_before,
        "blocked dry-run does not change worktree status"
    );
}

#[test]
fn dry_run_with_planner_created_blank_target_keeps_state_unchanged() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata_at_target(&["A", "B"], "origin/main");
    env.but("branch new empty --above A").assert().success();
    env.file("new.txt", "new empty-target content\n");

    let repo = env.open_repo();
    let refs_before = ["refs/heads/empty", "refs/heads/A", "refs/heads/B"].map(|reference| {
        repo.rev_parse_single(reference.as_bytes())
            .unwrap()
            .detach()
    });
    let status_before = util::status_json(&env);
    env.but("absorb --dry-run")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success();
    let refs_after = ["refs/heads/empty", "refs/heads/A", "refs/heads/B"].map(|reference| {
        repo.rev_parse_single(reference.as_bytes())
            .unwrap()
            .detach()
    });
    assert_eq!(
        refs_after, refs_before,
        "blank-target dry-run does not move refs"
    );
    assert_eq!(
        util::status_json(&env),
        status_before,
        "blank-target dry-run does not change worktree status"
    );
}

/// Regression test for GB-1534: in single-branch mode absorb must amend the
/// checked-out branch without recreating `gitbutler/workspace` and moving
/// `HEAD` onto it.
#[test]
fn single_branch_absorb_keeps_head_on_branch() {
    let env = Sandbox::open_with_default_settings("single-branch-mode");
    env.but("branch new feature").assert().success();
    env.file("file.txt", "a\nb\nc\n");
    env.but("commit -m 'change b'").assert().success();
    env.file("file.txt", "a\nB\nc\n");

    env.but("absorb").assert().success().stderr_eq(str![""]);

    let repo = env.open_repo();
    let blob = repo
        .rev_parse_single(b"feature:file.txt")
        .unwrap()
        .object()
        .unwrap();
    // The change was absorbed into the branch commit.
    snapbox::assert_data_eq!(
        blob.data.as_bstr().to_string(),
        snapbox::str![[r#"
a
B
c

"#]]
    );
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "feature",
        "absorb must leave HEAD on the checked-out branch"
    );
    assert!(
        repo.try_find_reference("refs/heads/gitbutler/workspace")
            .unwrap()
            .is_none(),
        "absorb must not create a workspace ref in single-branch mode"
    );
}

#[test]
fn absorbing_a_linked_worktrees_changes_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    util::enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    let wt_dir = util::add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");

    env.but("absorb")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Cannot absorb uncommitted changes in worktree wt-feature yet

"#]]);

    env.but("absorb nl")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Cannot absorb uncommitted changes in worktree wt-feature yet

"#]]);

    // Neither checkout's changes were absorbed.
    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   qy A main.txt
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt:@ [uncommitted] {wt-feature}
┊┊┊   nl A note.txt
┊┊├┄ wt [wt-feature] (no commits)
┊├╯
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn bare_absorb_in_an_unmanaged_worktree_is_refused() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let wt_dir = util::add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in main\n");

    env.but("absorb")
        .current_dir(&wt_dir)
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Worktree wt-feature is not managed by GitButler

Hint: Run `but worktree list` to see the worktrees GitButler manages

"#]]);
}
