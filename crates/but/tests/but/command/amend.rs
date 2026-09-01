use snapbox::str;

use crate::{
    command::util::{
        add_dirty_worktree, branch_commit_cli_ids, enable_worktree_manipulation,
        status_json_with_files as status_json,
    },
    utils::Sandbox,
};

#[test]
fn rejects_unnamed_segment_as_target() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);
    env.file("new.txt", "content\n");

    env.but("amend -t g0")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Cannot operate on anonymous branch 'g0'

Hint: Name it with `but reword g0` first! Note that the short ID is likely to change when the branch is named.

"#]]);
}

fn uncommitted_contains_file(status: &serde_json::Value, file_path: &str) -> bool {
    status["uncommittedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

fn branch_commits_contain_file(
    status: &serde_json::Value,
    branch_name: &str,
    file_path: &str,
) -> bool {
    status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .filter(|branch| branch["name"].as_str().unwrap() == branch_name)
        .flat_map(|branch| branch["commits"].as_array().unwrap().iter())
        .flat_map(|commit| commit["changes"].as_array().unwrap().iter())
        .any(|change| change["filePath"].as_str().unwrap() == file_path)
}

#[test]
fn amend_rejects_dependency_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    // Commit `first` to branch foo and an unrelated file to branch bar.
    env.file("first", "Some text");
    env.but("commit -m 'add first' -b foo").assert().success();
    env.file("second", "Other text");
    env.but("commit -m 'add second' -b bar").assert().success();

    // Change `first` (which depends on foo) and try to amend it into bar's
    // commit. The squash internals reject the operation atomically.
    env.file("first", "changes");
    let status = status_json(&env);
    let bar_commit_cli_id = branch_commit_cli_ids(&status, "bar")[0].clone();
    env.but(format!("amend first --target {bar_commit_cli_id}"))
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(str![[r#"
Error: Cannot amend: 1 change could not be applied:
  first
    line 1 depends on foo (1)

Hint: to apply these changes, stack bar on top of foo and try again — commits already on the branch move with it:
  but move bar --above foo

"#]]);

    let after = status_json(&env);
    assert!(
        uncommitted_contains_file(&after, "first"),
        "a rejected amend must leave its source uncommitted"
    );
    assert!(
        !branch_commits_contain_file(&after, "bar", "first"),
        "a rejected amend must not modify the target branch"
    );
}

#[test]
fn amend_accepts_multiple_uncommitted_changes() {
    assert_multiple_amend(|target_cli_id| {
        format!("amend one.txt two.txt --target {target_cli_id}")
    });
}

#[test]
fn amend_accepts_branch_target() {
    assert_multiple_amend(|_target_cli_id| "amend one.txt two.txt --target A".to_string());
}

#[test]
fn amend_without_source_implies_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "content");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted]
┊   qs A file
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("amend -t tpm")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended tpm

"#]])
        .stderr_eq(str![""]);

    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

fn assert_multiple_amend(args: impl FnOnce(&str) -> String) {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("one.txt", "one\n");
    env.file("two.txt", "two\n");
    env.file("three.txt", "three\n");

    let before = status_json(&env);
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(args(&target_cli_id))
        .assert()
        .success()
        .stdout_eq(str![[r#"
Amended tpm

"#]])
        .stderr_eq(str![""]);

    let after = status_json(&env);
    assert!(
        !uncommitted_contains_file(&after, "one.txt"),
        "first amended file should no longer be uncommitted"
    );
    assert!(
        !uncommitted_contains_file(&after, "two.txt"),
        "second amended file should no longer be uncommitted"
    );
    assert!(
        uncommitted_contains_file(&after, "three.txt"),
        "unmentioned file should remain uncommitted"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "one.txt"),
        "first file should be amended into a commit on branch A"
    );
    assert!(
        branch_commits_contain_file(&after, "A", "two.txt"),
        "second file should be amended into a commit on branch A"
    );
}

#[test]
fn amend_rejects_merged_upstream_commit() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");
    env.file("file.txt", "Some text");

    let status = status_json(&env);
    let source = status["uncommittedChanges"][0]["cliId"]
        .as_str()
        .unwrap()
        .to_string();

    // `nyq` is branch A's commit, whose content already landed on origin/main.
    env.but(format!("amend -t nyq {source}"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    // The escape hatch permits amending landed commits regardless.
    env.but(format!("amend -t nyq {source} --allow-merged"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended nyq

"#]]);
}

#[test]
fn amend_rejects_landed_commit_in_partially_integrated_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "pull-partially-integrated-multi-branch-stack",
    );
    env.setup_single_stack_metadata_at_target(&["A", "C"], "refs/heads/base");
    env.file("file.txt", "Some text");

    let status = status_json(&env);
    let source = status["uncommittedChanges"][0]["cliId"]
        .as_str()
        .unwrap()
        .to_string();
    let landed_bottom = branch_commit_cli_ids(&status, "C")
        .pop()
        .expect("branch C has a commit");

    // Branch C at the bottom of the stack has landed while A on top is live;
    // the landed commit alone must be refused.
    env.but(format!("amend -t {landed_bottom} {source}"))
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit e5378e0 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn retired_syntax_gets_a_teaching_hint() {
    let env = Sandbox::empty();

    // The pre-revamp `but amend <commit> --changes <id>,<id>` form: the hint
    // suggests the concrete modern equivalent before the parse error.
    env.but("amend j4 --changes ab,cd")
        .assert()
        .failure()
        .stderr_eq(str![[r#"

note: this invocation used retired `but amend` syntax. The modern equivalent is:

    but amend -t j4 ab cd

See `but amend --help` for details.
error: unexpected argument '--changes' found

  tip: to pass '--changes' as a value, use '-- --changes'

Usage: but amend --target <COMMIT_OR_BRANCH> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn parse_error_appends_examples() {
    let env = Sandbox::empty();

    // A rejected amend command line must end with the registered example
    // invocations, after clap's own error and usage output.
    env.but("amend").assert().failure().stderr_eq(str![[r#"
error: the following required arguments were not provided:
  --target <COMMIT_OR_BRANCH>

Usage: but amend --target <COMMIT_OR_BRANCH> [SOURCES]...

For more information, try '--help'.

Examples:
  but amend -t <commit> <file-or-hunk>...   # amend selected uncommitted changes
  but amend -t <commit>                     # amend all uncommitted changes
  but amend -t <branch>                     # amend into the tip of a branch

"#]]);
}

#[test]
fn root_level_parse_error_gets_no_examples() {
    let env = Sandbox::empty();

    // The bad flag belongs to the root grammar; appending `amend` examples
    // would misattribute the failure to the amend grammar.
    env.but("--nope amend")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: unexpected argument '--nope' found

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);
}

#[test]
fn retired_flag_with_help_passes_through_without_hint() {
    let env = Sandbox::empty();

    // Help requests arrive as clap parse errors too; when clap decides to
    // show help despite the retired `--changes` marker being present, no
    // retired-syntax note may precede it.
    env.but("amend --help --changes ab")
        .assert()
        .success()
        .stderr_eq(str![""]);
}

/// A worktree file's ID amends that change into a commit: it lands there and
/// leaves the worktree's uncommitted area, whose checkout is updated so the
/// change does not reappear.
#[test]
fn amend_a_file_from_a_linked_worktree() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");

    env.but("amend nl --target tpm")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended tpm

"#]]);

    // The change moved: amended into A's commit, gone from the worktree's area.
    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt {wt-feature} (no changes)
┊├╯
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:u A note.txt
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
}

/// The worktree's own ID names its whole uncommitted area, and the amend target
/// does not have to be on the worktree's branch.
#[test]
fn amend_a_worktrees_whole_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");

    env.but("amend wt --target lrm")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended lrm

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt {wt-feature} (no changes)
┊├╯
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
┊│     lrm:u A note.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

/// Changes dirty in different checkouts cannot go into one amend: they are
/// read from and cancelled out of different repositories.
#[test]
fn amend_refuses_changes_from_several_checkouts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");
    env.file("main.txt", "dirty in the main worktree\n");

    env.but("amend main.txt wt-feature:note.txt --target tpm")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Cannot use changes from the uncommitted area and worktree wt-feature together

Hint: An operation can only take changes from one checkout at a time

"#]]);
}

/// A clean worktree's ID expands to no changes, which is refused before the
/// squash classification requires a source.
#[test]
fn amend_a_clean_worktree_has_nothing_to_amend() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
    env.but("status").assert().success();
    let wt = env.app_data_dir().join("worktrees");
    but_testsupport::invoke_bash_at_dir(
        &format!(
            r#"git worktree add -q -b wt-clean "{wt}/wt-clean" A"#,
            wt = wt.display()
        ),
        env.projects_root(),
    );

    env.but("status")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt {wt-clean} (no changes)
┊├╯
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

    env.but("amend wt --target tpm")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: No changes to amend

Hint: Run `but status` to show applicable targets

"#]]);
}

/// `@` expands to the main checkout's whole uncommitted area for amending, the way a
/// worktree's ID expands to its area.
#[test]
fn amend_the_uncommitted_area_by_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.file("one.txt", "first\n");
    env.file("two.txt", "second\n");

    env.but("amend @ --target lrm")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Amended lrm

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:pl A B
┊│     lrm:z  A one.txt
┊│     lrm:pp A two.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
