use bstr::ByteSlice;

use crate::{
    command::util::{self, commit_file_with_worktree_changes_as_two_hunks},
    utils::{CommandExt as _, Sandbox},
};

#[test]
fn discard_removes_selected_change() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/discard-me.ts", "export const value = true;\n");

    env.but("discard src/discard-me.ts").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    assert!(
        !env.projects_root().join("src/discard-me.ts").exists(),
        "discarding a new file should remove it from the worktree"
    );
}

#[test]
fn discard_removes_path_prefix_mixed_with_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("path/to/first.txt", "first\n");
    env.file("path/to/second.txt", "second\n");
    env.file("path/to-other.txt", "outside the prefix\n");
    env.file("also-discard.txt", "selected separately\n");

    env.but("discard path/to/ also-discard.txt")
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   nl A path/to-other.txt
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn concurrent_discard_to_independent_files_succeeds() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/a/discard.ts", "export const a = true;\n");
    env.file("src/b/discard.ts", "export const b = true;\n");

    let child_a = util::but_std_cmd(&env, "discard src/a/discard.ts")
        .spawn()
        .unwrap();
    let child_b = util::but_std_cmd(&env, "discard src/b/discard.ts")
        .spawn()
        .unwrap();

    let out_a = child_a.wait_with_output().unwrap();
    let out_b = child_b.wait_with_output().unwrap();

    assert!(
        out_a.status.success(),
        "first discard failed: {}",
        out_a.stderr.as_bstr()
    );
    assert!(
        out_b.status.success(),
        "second discard failed: {}",
        out_b.stderr.as_bstr()
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_reverts_simple_rename() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/rename-source.ts", "export const source = true;\n");
    env.but("commit -b A -m 'seed rename source'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("src/rename-source.ts"),
        env.projects_root().join("src/rename-target.ts"),
    )
    .unwrap();

    env.but("discard src/rename-target.ts").assert().success();

    assert!(
        env.projects_root().join("src/rename-source.ts").exists(),
        "discarding a rename should restore the source path"
    );
    assert!(
        !env.projects_root().join("src/rename-target.ts").exists(),
        "discarding a rename should remove the target path"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "discarding a rename should leave a clean worktree"
    );
}

#[test]
fn discard_rename_does_not_discard_unrelated_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/rename-source-only.ts", "export const source = 1;\n");
    env.but("commit -b A -m 'seed rename source only'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("src/rename-source-only.ts"),
        env.projects_root().join("src/rename-target-only.ts"),
    )
    .unwrap();
    env.file("src/keep-me.ts", "export const keep = true;\n");

    env.but("discard src/rename-target-only.ts")
        .assert()
        .success();

    assert!(
        env.projects_root()
            .join("src/rename-source-only.ts")
            .exists(),
        "discarding rename should restore source path"
    );
    assert!(
        !env.projects_root()
            .join("src/rename-target-only.ts")
            .exists(),
        "discard should remove renamed target path"
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   tz A src/keep-me.ts
┊
┊╭┄ g0 [A]
┊●   1 seed rename source only
┊│     1:l A src/rename-source-only.ts
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let git_status = env.invoke_git("status --porcelain");
    assert!(
        git_status.contains("src/keep-me.ts"),
        "expected unrelated uncommitted file to remain, got:\n{git_status}"
    );
    assert!(
        !git_status.contains("rename-target-only") && !git_status.contains("rename-source-only"),
        "rename paths should no longer be dirty, got:\n{git_status}"
    );
}

#[test]
fn discard_the_whole_uncommitted_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/rename-source-only.ts", "export const source = 1;\n");
    env.but("commit -b A -m 'seed rename source only'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("src/rename-source-only.ts"),
        env.projects_root().join("src/rename-target-only.ts"),
    )
    .unwrap();
    env.file("src/keep-me.ts", "export const keep = true;\n");

    env.but("discard zz").assert().success();

    assert!(
        env.projects_root()
            .join("src/rename-source-only.ts")
            .exists(),
        "discarding rename should restore source path"
    );
    assert!(
        !env.projects_root()
            .join("src/rename-target-only.ts")
            .exists(),
        "discard should remove renamed target path"
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 seed rename source only
┊│     1:l A src/rename-source-only.ts
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "discarding a rename should leave a clean worktree"
    );
}

#[test]
fn discarding_multiple_hunks_in_a_file_works() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let content = "1\n2\n3\n4\n5\n6\n7";
    let file_path = "src/some_file.txt";

    env.file(file_path, content);
    env.but("commit -b A -m 'seed rename source only'")
        .assert()
        .success();

    env.file(file_path, "a\nb\nc\n1\n2\n3\n4\n5\n6\n7\nd\ne\nf");
    env.but("discard zz").assert().success();

    assert!(
        env.projects_root().join("src/some_file.txt").exists(),
        "discarding multiple hunks should keep the tracked file present"
    );

    let content_after_discard = env.read_file(file_path).unwrap();
    assert_eq!(
        content_after_discard, content,
        "discarding all hunks should restore the committed contents"
    );
}

#[test]
fn discard_multiple_uncommitted_files_outputs_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("first-uncommitted.txt", "first\n");
    env.file("second-uncommitted.txt", "second\n");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   rv A first-uncommitted.txt
┊   xs A second-uncommitted.txt
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("--json discard first-uncommitted.txt second-uncommitted.txt")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "uncommittedChanges",
  "paths": [
    "first-uncommitted.txt",
    "second-uncommitted.txt"
  ]
}

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_resulting_in_workdir_ud_conflict() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("commit.txt", "text\n");
    env.but("commit -b A -m 'discardable commit'")
        .assert()
        .success();
    let commit_id = env.invoke_git("rev-parse refs/heads/A");

    env.file("commit.txt", "would conflict if commit above was deleted\n");

    env.but(format!("discard {commit_id}"))
        .assert()
        .success()
        .stderr_eq("")
        .stdout_eq(snapbox::str![[r#"
Discarded commit 1

⚠ A conflict occurred during checkout. Run `but status` for more information.

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊    commit.txt {conflicted}
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M
⚠ Uncommitted file conflicts: choose the desired file state, then run `git add -- <path>`.

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_status(),
        snapbox::str![[r#"
UD commit.txt

"#]]
    );

    snapbox::assert_data_eq!(
        std::fs::read_to_string(env.projects_root().join("commit.txt")).unwrap(),
        snapbox::str![[r#"
would conflict if commit above was deleted

"#]]
    );
}

#[test]
fn discard_resulting_in_workdir_uu_conflict() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("commit.txt", "first\n");
    env.but("commit -b A -m 'commit'").assert().success();

    env.file("commit.txt", "second\n");
    env.but("commit -b A -m 'discardable commit'")
        .assert()
        .success();
    let commit_id = env.invoke_git("rev-parse refs/heads/A");

    env.file("commit.txt", "would conflict if commit above was deleted\n");

    env.but(format!("discard {commit_id}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded commit 1

⚠ A conflict occurred during checkout. Run `but status` for more information.

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊    commit.txt {conflicted}
┊
┊╭┄ g0 [A]
┊●   1 commit
┊│     1:t A commit.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M
⚠ Uncommitted file conflicts: choose the desired file state, then run `git add -- <path>`.

Hint: run `but help` for all commands

"#]]);

    snapbox::assert_data_eq!(
        env.git_status(),
        snapbox::str![[r#"
UU commit.txt

"#]]
    );

    // The output depends on whether merge.conflictstyle=diff3 is configured in
    // gitconfig, so add a wildcard to support both types of output.
    snapbox::assert_data_eq!(
        std::fs::read_to_string(env.projects_root().join("commit.txt")).unwrap(),
        snapbox::str![[r#"
<<<<<<< ours
would conflict if commit above was deleted
...
=======
first
>>>>>>> theirs

"#]]
    );
}

#[test]
fn discard_multiple_commits_outputs_human() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("first-commit.txt", "first\n");
    env.but("commit -b A -m 'first discardable commit'")
        .assert()
        .success();
    let first = env.invoke_git("rev-parse refs/heads/A");

    env.file("second-commit.txt", "second\n");
    env.but("commit -b A -m 'second discardable commit'")
        .assert()
        .success();
    let second = env.invoke_git("rev-parse refs/heads/A");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 second discardable commit
┊│     1#0:r A second-commit.txt
┊●   1#1 first discardable commit
┊│     1#1:m A first-commit.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("discard {first} {second}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded commits 1, 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert!(
        !env.projects_root().join("first-commit.txt").exists(),
        "discarding the first commit should remove its changes"
    );
    assert!(
        !env.projects_root().join("second-commit.txt").exists(),
        "discarding the second commit should remove its changes"
    );
}

#[test]
fn discard_committed_files_outputs_new_commit_in_json() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("discarded-from-commit.txt", "discard me\n");
    env.file("retained-in-commit.txt", "retain me\n");
    env.but("commit -b A -m 'files to selectively discard'")
        .assert()
        .success();
    let source = env.invoke_git("rev-parse refs/heads/A");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 files to selectively discard
┊│     1:n A discarded-from-commit.txt
┊│     1:x A retained-in-commit.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but(format!("--json discard {source}:discarded-from-commit.txt"))
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "committedFiles",
  "sourceCommitId": "c61e0f8eb6e54760c5a265d93044bf29b7a5716a",
  "sourceChangeId": "1",
  "paths": [
    "discarded-from-commit.txt"
  ],
  "newCommitId": "372ab397ba61d3368a1a9e769f39af3997c4e1ad",
  "newChangeId": "1"
}

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 files to selectively discard
┊│     1:x A retained-in-commit.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    assert!(
        !env.projects_root()
            .join("discarded-from-commit.txt")
            .exists(),
        "discarding a committed file should remove its changes"
    );
    assert!(
        env.projects_root().join("retained-in-commit.txt").exists(),
        "discarding one committed file should retain other committed files"
    );
}

#[test]
fn discard_rejects_mixed_sources() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("committed.txt", "committed\n");
    env.but("commit -b A -m 'committed source'")
        .assert()
        .success();
    let commit = env.invoke_git("rev-parse refs/heads/A");
    env.file("uncommitted.txt", "uncommitted\n");

    env.but(format!("discard A {commit}"))
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<CHANGES>'

Cannot mix different types of sources

Hint: Discard branches, commits, committed files, or uncommitted changes separately

"#]]);
    env.but(format!("discard {commit} {commit}:committed.txt"))
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<CHANGES>'

Cannot mix different types of sources

Hint: Discard branches, commits, committed files, or uncommitted changes separately

"#]]);
    env.but("discard zz uncommitted.txt")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<CHANGES>'

Cannot mix different types of sources

Hint: Discard branches, commits, committed files, or uncommitted changes separately

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   ln A uncommitted.txt
┊
┊╭┄ g0 [A]
┊●   1 committed source
┊│     1:z A committed.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn discard_rejects_committed_files_from_multiple_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("first-committed.txt", "first\n");
    env.but("commit -b A -m 'first committed source'")
        .assert()
        .success();
    let first = env.invoke_git("rev-parse refs/heads/A");

    env.file("second-committed.txt", "second\n");
    env.but("commit -b A -m 'second committed source'")
        .assert()
        .success();
    let second = env.invoke_git("rev-parse refs/heads/A");

    env.but(format!(
        "discard {first}:first-committed.txt {second}:second-committed.txt"
    ))
    .assert()
    .failure()
    .stderr_eq(snapbox::str![[r#"
Error: Bad input for '<CHANGES>'

All committed files must come from the same commit

Hint: Discard committed files from each commit separately

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 second committed source
┊│     1#0:q A second-committed.txt
┊●   1#1 first committed source
┊│     1#1:t A first-committed.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_an_uncommitted_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "hunks.txt");

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
lw:2 hunks.txt│
──────────────╯
   1  │-first
     1│+firsta
   2 2│ line
   3 3│ line
   4 4│ line
──────────────╮
lw:e hunks.txt│
──────────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);

    env.but("discard lw:2")
        .assert()
        .success()
        .stdout_eq("Discarded uncommitted changes from hunks.txt\n");

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
──────────────╮
lw:e hunks.txt│
──────────────╯
    6  6│ line
    7  7│ line
    8  8│ line
    9   │-last
       9│+lasta

"#]]);

    let content = env.read_file("hunks.txt").expect("hunks.txt should exist");
    assert!(
        content.starts_with("first\n"),
        "the discarded first hunk should be restored"
    );
    assert!(
        content.ends_with("lasta\n"),
        "the undiscarded last hunk should remain"
    );
}

#[test]
fn discard_that_conflicts_warns_on_stderr_in_json_mode() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-dependent-commits");
    env.setup_metadata(&["A"]);

    let status = util::status_json(&env);
    let bottom = util::branch_commit_cli_ids(&status, "A")
        .pop()
        .expect("branch A has commits");

    // Discarding the bottom commit rebases its dependent on top of the base,
    // which conflicts. JSON output stays parseable, so the warning goes to
    // stderr.
    env.but(format!("--json discard {bottom}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "type": "commits",
  "commits": [
    {
      "commitId": "5f55e3a5b4a9441ce84d6a0858f7c0a970576d50",
      "changeId": "zpotlpzlquzwlkypyzutoxyswrxxquxm"
    }
  ]
}

"#]])
        .stderr_eq(snapbox::str![[r#"
warning: this operation left 1 commit(s) conflicted: [..]. Resolve with `but resolve`, or back out with `but undo`.

"#]]);
}

#[test]
fn discard_defaults_to_zz() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("src/discard-me.ts", "export const value = true;\n");

    env.but("discard")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded uncommitted changes from src/discard-me.ts

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
