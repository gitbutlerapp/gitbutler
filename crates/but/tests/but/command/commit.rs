use super::util::{find_branch, status_json_with_files};
use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn commits_a_dirty_file_on_a_new_branch_in_single_branch_mode() {
    let env = Sandbox::open_with_default_settings("one-fork");
    env.but("config feature single-branch enable")
        .assert()
        .success();
    env.file("ad-hoc.txt", "content\n");

    env.but("status --files")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   xt A ad-hoc.txt
┊
┊╭┄ ma [main]
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let diff = env
        .but("--json diff")
        .allow_json()
        .output()
        .expect("diff should succeed");
    let diff: serde_json::Value =
        serde_json::from_slice(&diff.stdout).expect("diff output should be JSON");
    let changes = diff["changes"]
        .as_array()
        .expect("diff.changes should be an array");
    assert_eq!(changes.len(), 1, "the dirty file should be the only change");
    assert_eq!(changes[0]["path"], "ad-hoc.txt");
    let change_id = changes[0]["id"]
        .as_str()
        .expect("the dirty file should have a CLI ID");

    env.but(format!(
        "commit -b feature -m 'add ad-hoc file' {change_id}"
    ))
    .assert()
    .success()
    .stderr_eq(snapbox::str![])
    .stdout_eq(snapbox::str![[r#"
Created commit 1 on new branch 'feature'

"#]]);

    env.but("status --files")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ at [feature]
┊●   1 add ad-hoc file
┊│     1:x A ad-hoc.txt
┊│
┊├┄ ma [main]
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but help` for all commands

"#]]);

    let status = status_json_with_files(&env);
    assert_eq!(
        status["uncommittedChanges"].as_array().map(Vec::len),
        Some(0),
        "the committed file should no longer be dirty"
    );
    let branch = find_branch(&status, "feature");
    assert_eq!(branch["commits"].as_array().map(Vec::len), Some(1));
    assert_eq!(branch["commits"][0]["changes"][0]["filePath"], "ad-hoc.txt");
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "feature",
        "creating the branch should check it out in single-branch mode"
    );
    assert_eq!(env.invoke_git("show feature:ad-hoc.txt"), "content");
    assert!(
        env.open_repo()
            .try_find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .is_none(),
        "the commit journey must remain outside managed workspace mode"
    );
}

#[test]
fn commits_on_the_checked_out_branch_in_single_branch_mode() {
    let env = Sandbox::open_with_default_settings("one-fork");
    env.but("config feature single-branch enable")
        .assert()
        .success();
    let old_head = env.invoke_git("rev-parse HEAD");
    env.file("existing.txt", "content\n");

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   pr A existing.txt
┊
┊╭┄ ma [main]
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'commit on main'")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'main'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ma [main]
┊●   1 commit on main
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "main",
        "committing to the checked-out branch should keep it checked out"
    );
    assert_eq!(
        env.invoke_git("rev-parse HEAD^"),
        old_head,
        "the commit should advance the checked-out branch"
    );
    assert_eq!(env.invoke_git("show HEAD:existing.txt"), "content");
    assert!(
        env.open_repo()
            .try_find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .is_none(),
        "committing to an existing branch must not create a managed workspace"
    );
}

#[test]
fn commits_at_each_branch_in_an_existing_single_branch_stack() {
    let env = Sandbox::open_with_default_settings("one-fork");
    env.but("config feature single-branch enable")
        .assert()
        .success();

    env.but("branch new middle").assert().success();
    env.but("commit --empty -b middle -m 'middle base'")
        .assert()
        .success();
    env.but("branch new top").assert().success();
    env.but("commit --empty -b top -m 'top base'")
        .assert()
        .success();

    env.file("bottom.txt", "bottom\n");
    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   tk A bottom.txt
┊
┊╭┄ to [top]
┊●   1#0 top base (no changes)
┊│
┊├┄ mi [middle]
┊●   1#1 middle base (no changes)
┊│
┊├┄ ma [main]
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -b main -m 'bottom position'")
        .assert()
        .success();
    env.file("middle.txt", "middle\n");
    env.but("commit -b middle -m 'middle position'")
        .assert()
        .success();
    env.file("top.txt", "top\n");
    env.but("commit -b top -m 'top position'")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top]
┊●   1#0 top position
┊●   1#1 top base (no changes)
┊│
┊├┄ mi [middle]
┊●   1#2 middle position
┊●   1#3 middle base (no changes)
┊│
┊├┄ ma [main]
┊●   1#4 bottom position
┊●   nmy M (no changes)
├╯
┊
┴ e31e6ca (common base) 2000-01-02 add init

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        env.invoke_git("log --format=%s --reverse origin/main..top"),
        "M\nbottom position\nmiddle base\nmiddle position\ntop base\ntop position",
        "commits should remain ordered at the bottom, middle, and top branch positions"
    );
    assert_eq!(
        env.invoke_git("show -s --format=%s main"),
        "bottom position"
    );
    assert_eq!(
        env.invoke_git("show -s --format=%s middle"),
        "middle position"
    );
    assert_eq!(env.invoke_git("show -s --format=%s top"), "top position");
    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "top",
        "committing lower in the stack should preserve the checked-out top branch"
    );
    assert!(
        env.open_repo()
            .try_find_reference(but_core::WORKSPACE_REF_NAME)
            .unwrap()
            .is_none(),
        "stacked commits must remain outside managed workspace mode"
    );
}

#[test]
fn commits_on_top_of_a_checked_out_managed_workspace_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("config feature single-branch enable")
        .assert()
        .success();
    env.invoke_git("checkout A");
    env.file("outside-workspace.txt", "content\n");

    env.but("commit -b feature -m 'commit outside workspace'")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on new branch 'feature'

"#]]);

    assert_eq!(
        env.invoke_git("symbolic-ref --short HEAD"),
        "feature",
        "the new branch should be checked out"
    );
    assert_eq!(
        env.invoke_git("rev-parse feature^"),
        env.invoke_git("rev-parse A"),
        "the new branch should be based on the previously checked-out branch"
    );
    assert_eq!(
        env.invoke_git("show feature:outside-workspace.txt"),
        "content"
    );
}

#[test]
fn no_message_nothing_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("commit --no-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message) (no changes)
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn no_args_single_head_no_message_human_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit --no-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'A'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn agent_mutation_omits_status_unless_requested() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("file.txt", "Some text");

    let result = env
        .but("commit --no-message")
        .env("AI_AGENT", "codex")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout);

    assert!(stdout.contains("Created commit"));
    assert!(
        !stdout.contains("╭┄ zz"),
        "agent mutations must omit workspace status by default"
    );
    assert!(
        !stdout.contains("commits are listed newest first"),
        "agent mutations must omit status hints by default"
    );

    env.file("other.txt", "Other text");
    let result = env
        .but("commit --no-message --status-after")
        .env("AI_AGENT", "codex")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout);

    assert!(stdout.contains("Created commit"));
    assert!(
        stdout.contains("╭┄ zz"),
        "explicit status opt-in must append workspace status"
    );
}

#[test]
fn human_mutation_can_request_status_after() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("file.txt", "Some text");

    let result = env
        .but("commit --no-message --status-after")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout);

    assert!(stdout.contains("Created commit"));
    assert!(
        stdout.contains("╭┄ zz"),
        "human callers should be able to opt into workspace status"
    );
}

#[test]
fn agent_commit_json_uses_native_result_without_status_by_default() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit --no-message --json")
        .env("AI_AGENT", "codex")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commitId": "7bbfdca68284535242b93595db5f6a5bc885a124",
  "changeId": "1"
}

"#]]);
}

#[test]
fn non_agent_commit_json_uses_native_result() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit --no-message --json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commitId": "7bbfdca68284535242b93595db5f6a5bc885a124",
  "changeId": "1"
}

"#]]);
}

#[test]
fn no_args_single_head_message_from_editor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // TODO: move this into Sandbox
    env.file("editor.sh", "printf 'commit from editor\\n' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 commit from editor
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn single_head_with_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit -m 'add file.txt'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add file.txt
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_repeat_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit -m 'add file.txt' -m 'with more' -m 'text lines'")
        .assert()
        .success();

    env.but("status -v")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha b141567)
┊│     add file.txt  with more  text lines
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 9477ae7)
┊│     add A 
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("show 1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Commit:    b14156794f81a138bd06c2a5287fd5db15408b56
Change-ID: 1
Author:    author <author@example.com>
Date:      2000-01-02 00:00:00 +0000 (26y ago)
Committer: committer <committer@example.com>

add file.txt

with more

text lines

Files changed:
  A file.txt

"#]]);
}

#[test]
fn editor_user_writes_no_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("editor.sh", "printf '' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn editor_fails() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("editor.sh", "false");
    let editor_path = env.projects_root().join("editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.file("file.txt", "Some text");

    env.but("commit")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .failure()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::str![[r#"
Error: Editor exited with non-zero status

"#]]);
}

#[test]
fn create_commit_on_new_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file.txt", "Some text");

    env.but("commit --no-message").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_user_provided_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("commit -m 'add first' -b file").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ fi [file]
┊●   1 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("second", "change file");

    env.but("commit -m 'add second' -b file").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ fi [file]
┊●   1#0 add second
┊●   1#1 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("third", "change file");

    env.but("commit -m 'add third' -b other").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ot [other]
┊●   1#0 add third
├╯
┊
┊╭┄ fi [file]
┊●   1#1 add second
┊●   1#2 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("fourth", "change file");

    env.but("commit -m 'add fourth' -b other")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ot [other]
┊●   1#0 add fourth
┊●   1#1 add third
├╯
┊
┊╭┄ fi [file]
┊●   1#2 add second
┊●   1#3 add first
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_new_branch_with_canned_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some text");

    env.but("commit -m 'add file.txt' -b").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 add file.txt
├╯
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn create_commit_on_branch_that_is_not_applied_fails() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.invoke_git("branch existing");

    env.file("first", "Some text");

    env.but("commit -m 'add first' -b existing")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'existing' exists but is not applied

Hint: Run `but apply existing` to apply the branch first

"#]]);
}

#[test]
fn bails_on_rejected_specs() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("commit -m 'add first' -b foo").assert().success();

    env.file("first", "changes");

    env.but("commit -m 'add first' -b bar")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot commit: 1 change could not be applied:
  first
    line 1 depends on foo (1)

Hint: to apply these changes, create bar stacked on top of foo and try again:
  but branch new bar --anchor foo

"#]]);
}

#[test]
fn newly_created_branches_are_included_in_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("first", "Some text");

    env.but("commit -m 'add first' -b foo --json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "commitId": "5a6fc56305c69edc974a5ed2d100c525db8fd288",
  "changeId": "1",
  "branch": "foo"
}

"#]]);
}

#[test]
fn empty_flag_to_force_empty_commit_when_changes_exist() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(
        "changes",
        "Some changes that will not be included in commit",
    );

    env.but("commit -m 'empty commit despite changes in worktree' --empty")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   vq A changes
┊
┊╭┄ br [a-branch-1]
┊●   1 empty commit despite changes in worktree (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn empty_commit_ignores_metadata_for_missing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("commit -m 'empty commit' --empty")
        .assert()
        .success();

    assert!(
        env.open_repo()
            .try_find_reference("refs/heads/A")
            .expect("reference lookup succeeds")
            .is_none(),
        "oplog preparation must not recreate a branch from stale metadata"
    );
}

#[test]
fn commit_empty_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("commit --no-message --above zll")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   1 (no commit message) (no changes)
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_empty_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.but("commit --no-message --below zll")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
┊●   1 (no commit message) (no changes)
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'add file.txt' --above zll")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   1 add file.txt
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'add file.txt' --above g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 add file.txt
┊│
┊├┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'add file.txt' --below zll")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
┊●   1 add file.txt
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'add file.txt' --below g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│
┊├┄ br [a-branch-1]
┊●   1 add file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_below_branch_with_multiple_commits_treats_branch_as_bucket() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("file.txt", "Some changes");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   uv A file.txt
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'add file.txt' --below g0")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
┊│
┊├┄ br [a-branch-1]
┊●   1 add file.txt
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_above_refuses_on_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.file("second", "Conflicting with commit 9ac4652");

    env.but("commit -m 'add second' --above zll")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot commit: 1 change could not be applied:
  second
    line 1 depends on A (ywx)

"#]]);
}

#[test]
fn commit_below_refuses_on_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ywx add second
┊●   zll add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    env.file("second", "Conflicting with commit 9ac4652");

    env.but("commit -m 'add second' --below ywx")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot commit: 1 change could not be applied:
  second
    line 1 depends on A (ywx)

"#]]);
}

#[test]
fn refuses_above_and_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("commit --above dontcare --below dontcare")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--above <BRANCH_OR_COMMIT>' cannot be used with '--below <BRANCH_OR_COMMIT>'

Usage: but commit --above <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

Examples:
  but commit -b <branch> -m "message"                    # commit onto a branch (created if needed)
  but commit -b <branch> -m "message" <file-or-hunk>...  # commit only the given changes
  but commit -m "message"                                # commit when only one stack is applied

"#]]);
}

#[test]
fn refuses_above_and_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("commit --above dontcare -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--above <BRANCH_OR_COMMIT>' cannot be used with '--branch [<BRANCH>]'

Usage: but commit --above <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

Examples:
  but commit -b <branch> -m "message"                    # commit onto a branch (created if needed)
  but commit -b <branch> -m "message" <file-or-hunk>...  # commit only the given changes
  but commit -m "message"                                # commit when only one stack is applied

"#]]);
}

#[test]
fn refuses_below_and_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    env.but("commit --below dontcare -b")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the argument '--below <BRANCH_OR_COMMIT>' cannot be used with '--branch [<BRANCH>]'

Usage: but commit --below <BRANCH_OR_COMMIT> [CHANGES]...

For more information, try '--help'.

Examples:
  but commit -b <branch> -m "message"                    # commit onto a branch (created if needed)
  but commit -b <branch> -m "message" <file-or-hunk>...  # commit only the given changes
  but commit -m "message"                                # commit when only one stack is applied

"#]]);
}

#[test]
fn above_branch_not_in_workspace_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("unapply B").assert().success();

    env.but("commit --above B")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'B'

Hint: Target must be an applied branch or commit. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn above_commit_not_in_workspace_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply B").assert().success();

    // Unapplied commits have no change ID in the workspace map, so use the commit ID intentionally.
    env.but("commit --above d3")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'd3'

Hint: Target must be an applied branch or commit. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn above_non_branch_non_commit_target_returns_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit --above zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected a commit or a branch, got uncommitted changes

Hint: Run `but status` to show applicable targets

"#]]);
}

#[test]
fn committing_specific_cli_ids() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one", "content");
    env.file("two", "content");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   kl   A one
┊   twop A two
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit --no-message kl").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   twop A two
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊│     1:k A one
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn hunks_within_file_are_not_order_dependent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let original_data = "enough\nlines\nto\ncreate\nmultiple\nhunks\nwhen\nediting";

    env.file("file", original_data);

    env.but("commit --no-message").assert().success();

    env.file("file", format!("first hunk\n{original_data}\nlast hunk"));

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
qs:5 file│
─────────╯
     1│+first hunk
   1 2│ enough
   2 3│ lines
   3 4│ to
─────────╮
qs:2 file│
─────────╯
    6  7│ hunks
    7  8│ when
    8  9│ editing
      10│+last hunk

"#]]);

    env.but("commit --no-message qs:5 qs:2").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message)
┊│     1#0:q M file
┊●   1#1 (no commit message)
┊│     1#1:q A file
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("commit --no-message qs:2 qs:5").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message)
┊│     1#0:q M file
┊●   1#1 (no commit message)
┊│     1#1:q A file
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn overlapping_changes_to_modified_file_are_deduplicated() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let original_data = "enough\nlines\nto\ncreate\nmultiple\nhunks\nwhen\nediting";

    env.file("file", original_data);

    env.but("commit --no-message").assert().success();

    env.file("file", format!("first hunk\n{original_data}\nlast hunk"));

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
qs:5 file│
─────────╯
     1│+first hunk
   1 2│ enough
   2 3│ lines
   3 4│ to
─────────╮
qs:2 file│
─────────╯
    6  7│ hunks
    7  8│ when
    8  9│ editing
      10│+last hunk

"#]]);

    env.but("commit --no-message qs:5 qs:2 qs:5")
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message)
┊│     1#0:q M file
┊●   1#1 (no commit message)
┊│     1#1:q A file
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("commit --no-message file qs:5").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message)
┊│     1#0:q M file
┊●   1#1 (no commit message)
┊│     1#1:q A file
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_something_that_isnt_a_cli_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // The retired grammar's positional branch: teach `-b` targeting.
    env.but("commit --no-message A")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find uncommitted change: 'A'

Hint: 'A' is a branch. To commit onto it, run `but commit -b A -m "message" [<change>...]`

"#]]);

    // Anything that names no branch keeps the generic hint.
    env.but("commit --no-message notexist")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find uncommitted change: 'notexist'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn can_commit_with_path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("path/to/first.txt", "first");
    env.file("path/to/second.txt", "second");
    env.file("path/other/to/third.txt", "third");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   om A path/other/to/third.txt
┊   ms A path/to/first.txt
┊   rr A path/to/second.txt
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit path/to/ --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   om A path/other/to/third.txt
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊│     1:m A path/to/first.txt
┊│     1:r A path/to/second.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn path_prefix_with_mix_of_modifications() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("dir/to_modify.txt", "first");
    env.file("dir/to_delete.txt", "second");
    env.file("dir/to_empty.txt", "third");

    env.but("commit --no-message").assert().success();

    std::fs::remove_file(env.projects_root().join("dir/to_delete.txt")).unwrap();
    env.file("dir/to_empty.txt", "");
    env.file(
        env.projects_root().join("dir/to_modify.txt"),
        "first\nnew line",
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   lm D dir/to_delete.txt
┊   no M dir/to_empty.txt
┊   xv M dir/to_modify.txt
┊
┊╭┄ g0 [A]
┊●   1 (no commit message)
┊│     1:l A dir/to_delete.txt
┊│     1:n A dir/to_empty.txt
┊│     1:x A dir/to_modify.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit dir/ --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message)
┊│     1#0:l D dir/to_delete.txt
┊│     1#0:n M dir/to_empty.txt
┊│     1#0:x M dir/to_modify.txt
┊●   1#1 (no commit message)
┊│     1#1:l A dir/to_delete.txt
┊│     1#1:n A dir/to_empty.txt
┊│     1#1:x A dir/to_modify.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("diff 1#0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────────╮
D dir/to_delete.txt│
───────────────────╯
   1  │-second
──────────────────╮
M dir/to_empty.txt│
──────────────────╯
   1  │-third
───────────────────╮
M dir/to_modify.txt│
───────────────────╯
   1 1│ first
     2│+new line

"#]]);
}

#[test]
fn requires_specifying_stack_when_there_are_multiple() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.but("commit --empty --no-message")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Unclear where to commit. Found more than one stack

Hint: You can specify where to commit with `--branch [<BRANCH>]`

"#]]);
}

#[test]
fn committing_above_an_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    env.but("branch new top").assert().success();
    env.but("commit one -m 'add one' --above top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 add one
┊│
┊├┄ to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_empty_branch_with_empty_branch_below() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    env.but("branch new middle").assert().success();
    env.but("branch new --anchor middle top").assert().success();
    env.but("commit one -m 'add one' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   1 add one
┊│
┊├┄ mi [middle] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_non_top_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.file("two", "two content");

    env.but("commit one -m 'add one' -b bottom")
        .assert()
        .success();
    env.but("branch new --anchor bottom middle")
        .assert()
        .success();
    env.but("branch new --anchor middle top").assert().success();
    env.but("commit two -m 'add two' --below middle")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ mi [middle] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   1#0 add two
┊│
┊├┄ bo [bottom]
┊●   1#1 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_below_an_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.file("two", "two content");

    env.but("branch new top").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   kl   A one
┊   twop A two
┊
┊╭┄ to [top] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit one -m 'add one' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   twop A two
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   1 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("reword a-branch-1 -m bottom").assert().success();

    env.but("commit two -m 'add two' --below top")
        .assert()
        .success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ to [top] (no commits)
┊│
┊├┄ br [a-branch-1]
┊●   1#0 add two
┊│
┊├┄ bo [bottom]
┊●   1#1 add one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn gives_good_error_when_your_terminal_doesnt_support_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit --interactive")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Terminal doesn't support interactivity

"#]]);
}

#[test]
fn commit_to_existing_branch_via_short_code() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("commit -b g0 -m 'new commit'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 new commit (no changes)
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn commit_to_new_branch_with_same_name_as_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "");

    env.but("commit -b file -m 'add file'").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ fi [file]
┊●   1 add file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn can_overspecify_hunk_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "hello");

    env.but("diff")
        .assert()
        .success()
        // Full ID is qs:3c81ccd4449094b2becf2b846fc69cfdfcaa613c
        .stdout_eq(snapbox::str![[r#"
─────────╮
qs:3 file│
─────────╯
     1│+hello

"#]]);

    env.but("commit -m 'Add file' qs:3c81").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 Add file
┊│     1:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn error_on_ambiguous_hunk_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file(
        "file",
        "
1
2
3
4
5
6

1
2
3
4
5
6
",
    );

    env.but("commit -m 'Add file'").assert().success();

    env.file(
        "file",
        "
1
2
3
hellooo
4
5
6

1
2
3
hellooooo
4
5
6
",
    );

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────╮
qs:79 file│
──────────╯
   2 2│ 1
   3 3│ 2
   4 4│ 3
     5│+hellooo
   5 6│ 4
   6 7│ 5
   7 8│ 6
──────────╮
qs:78 file│
──────────╯
    9 10│ 1
   10 11│ 2
   11 12│ 3
      13│+hellooooo
   12 14│ 4
   13 15│ 5
   14 16│ 6

"#]]);

    env.but("commit --no-message qs:7")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Ambiguous uncommitted change 'qs:7', matches multiple items

Hint: Use a longer ID to disambiguate

"#]]);
}

#[test]
fn commit_does_not_needlessly_touch_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("A", "new content");

    let old_time = std::fs::metadata(env.projects_root().join("A"))
        .unwrap()
        .modified()
        .unwrap();
    env.but("commit -m test").assert().success();
    let new_time = std::fs::metadata(env.projects_root().join("A"))
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(
        new_time, old_time,
        "time should be the same, because file should not have been modified"
    );
}

#[test]
fn new_branches_are_created_on_top() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("commit --no-message -b").assert().success();

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message) (no changes)
├╯
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn committing_modified_and_renamed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "content");
    env.file("file-2", "content-2");

    env.but("commit -m 'add files'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 add files
┊│     1:q A file
┊│     1:k A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.file("file-2", "new content");
    env.rename_file("file-2", "file");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   qs M file
┊   kw D file-2
┊
┊╭┄ br [a-branch-1]
┊●   1 add files
┊│     1:q A file
┊│     1:k A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("commit -m 'change file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 change file
┊│     1#0:q M file
┊│     1#0:k D file-2
┊●   1#1 add files
┊│     1#1:q A file
┊│     1#1:k A file-2
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn branch_flag_rejects_merged_upstream_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    env.but("commit --no-message --branch A")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn default_target_skips_merged_upstream_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    // A has landed upstream, so the commit goes to B without prompting.
    env.but("commit --no-message")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'B'

"#]]);
}

#[test]
fn default_target_creates_new_branch_when_all_branches_merged_upstream() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-single-stack");
    env.setup_metadata_at_target(&["A"], "refs/heads/base");
    env.file("file.txt", "Some text");

    // The only branch has landed upstream, so the commit starts a new branch.
    env.but("commit --no-message")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on new branch 'a-branch-1'

"#]]);
}

#[test]
fn above_below_reject_merged_upstream_targets() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    // `nyq` is branch A's commit, whose content already landed on origin/main.
    env.but("commit --no-message --below nyq")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    env.but("commit --no-message --above A")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn branch_flag_with_allow_merged_permits_merged_upstream_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    env.but("commit --no-message --branch A --allow-merged")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'A'

"#]]);
}

#[test]
fn partially_integrated_stack_guards_only_the_landed_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "C"], "refs/heads/base");
    env.file("file.txt", "Some text");

    // Branch C at the bottom has landed; branch A on top is live. Committing
    // to the live tip must still work.
    env.but("commit --no-message --branch A")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on branch 'A'

"#]]);

    // The landed bottom branch stays refused.
    env.but("commit --no-message --branch C")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'C' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn branch_flag_rejects_empty_merged_upstream_branch() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-merged-empty-branch");

    env.but("apply origin/document-but-pr-skill")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success();
    env.file("file.txt", "Some text");

    // The applied branch has no local commits, but its remote tip has been
    // merged into the target; status marks it `(merged upstream)`.
    env.but("commit --no-message --branch document-but-pr-skill")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'document-but-pr-skill' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn retired_syntax_is_translated_and_hinted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");
    env.file("two", "two content");
    env.file("three", "three content");

    // The pre-revamp syntax: positional branch, `-c` to create it, and
    // `--changes` with a comma-separated list.
    env.but("commit my-branch -c -m 'add one and two' --changes one,two")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit 1 on new branch 'my-branch'

"#]])
        .stderr_eq(snapbox::str![[r#"

note: this invocation used retired `but commit` syntax. The modern form is:

    but commit -b <branch> -m "message" <change>...

See `but commit --help` for details.

"#]]);

    // Only the selected changes were committed; `three` stays uncommitted.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   or A three
┊
┊╭┄ my [my-branch]
┊●   1 add one and two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn retired_syntax_with_unsafe_changes_value_refuses_and_hints() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    // An empty `--changes` value must not silently become "commit
    // everything"; translation is refused, and the hint precedes the original
    // parse error.
    env.but("commit my-branch -c -m 'add nothing' --changes ''")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"

note: this invocation used retired `but commit` syntax. The modern form is:

    but commit -b <branch> -m "message" <change>...

See `but commit --help` for details.
error: unexpected argument '-c' found

  tip: to pass '-c' as a value, use '-- -c'

Usage: but commit [OPTIONS] [CHANGES]...

For more information, try '--help'.

"#]]);

    // A missing `--changes` value fails even the retired grammar, exactly as
    // the retired binary did; the original error surfaces with no hint.
    env.but("commit my-branch -c -m 'add nothing' --changes --json")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: unexpected argument '-c' found

  tip: to pass '-c' as a value, use '-- -c'

Usage: but commit [OPTIONS] [CHANGES]...

For more information, try '--help'.

Examples:
  but commit -b <branch> -m "message"                    # commit onto a branch (created if needed)
  but commit -b <branch> -m "message" <file-or-hunk>...  # commit only the given changes
  but commit -m "message"                                # commit when only one stack is applied

"#]]);

    // Nothing was committed by either refused invocation.
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   kl A one
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[test]
fn retired_syntax_without_modern_equivalent_hints_and_fails() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "one content");

    // `--no-hooks` has no modern equivalent, so the command still fails with
    // the original error — but the hint teaches the new syntax first.
    env.but("commit my-branch -c -m 'add one' --no-hooks --changes one")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"

note: this invocation used retired `but commit` syntax. The modern form is:

    but commit -b <branch> -m "message" <change>...

See `but commit --help` for details.
error: unexpected argument '-c' found

  tip: to pass '-c' as a value, use '-- -c'

Usage: but commit [OPTIONS] [CHANGES]...

For more information, try '--help'.

"#]]);
}
