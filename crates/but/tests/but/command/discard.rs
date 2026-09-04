use bstr::ByteSlice;

use crate::{
    command::util::{self, commit_file_with_worktree_changes_as_two_hunks},
    utils::{CommandExt as _, Sandbox},
};

#[test]
fn rejects_unnamed_segment_as_source() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-anonymous-segment");
    env.setup_metadata(&["A"]);

    env.but("discard g0")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot operate on anonymous branch 'g0'

Hint: Name it with `but reword g0` first! Note that the short ID is likely to change when the branch is named.

"#]]);
}

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
╭┄ @ [uncommitted] (no changes)
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
╭┄ @ [uncommitted]
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
╭┄ @ [uncommitted] (no changes)
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
╭┄ @ [uncommitted]
┊   tz A src/keep-me.ts
┊
┊╭┄ g0 [A]
┊●   uwm seed rename source only
┊│     uwm:l A src/rename-source-only.ts
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

    env.but("discard @").assert().success();

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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   uwm seed rename source only
┊│     uwm:l A src/rename-source-only.ts
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
    env.but("discard @").assert().success();

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
╭┄ @ [uncommitted]
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
╭┄ @ [uncommitted] (no changes)
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
Discarded commit tvn

⚠ A conflict occurred during checkout. Run `but status` for more information.

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊    commit.txt {conflicted}
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M
⚠ Uncommitted file conflicts: edit each file to the wanted contents (or delete it), then run `but resolve <path>...` to mark it resolved.

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
Discarded commit syk

⚠ A conflict occurred during checkout. Run `but status` for more information.

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊    commit.txt {conflicted}
┊
┊╭┄ g0 [A]
┊●   pmw commit
┊│     pmw:t A commit.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M
⚠ Uncommitted file conflicts: edit each file to the wanted contents (or delete it), then run `but resolve <path>...` to mark it resolved.

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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   yqn second discardable commit
┊│     yqn:r A second-commit.txt
┊●   lmp first discardable commit
┊│     lmp:m A first-commit.txt
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
Discarded commits lmp, yqn

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   toz files to selectively discard
┊│     toz:n A discarded-from-commit.txt
┊│     toz:x A retained-in-commit.txt
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
  "type": "committedChanges",
  "sourceCommitId": "7df37764a21d5510d4108ad82bfaf98bc926a1a8",
  "sourceChangeId": "tozmluwlnkpxmqykupputuolovmvyprt",
  "paths": [
    "discarded-from-commit.txt"
  ],
  "newCommitId": "ab57cd43e38112a1b44246daf6eb509f6097f5a4",
  "newChangeId": "tozmluwlnkpxmqykupputuolovmvyprt"
}

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   toz files to selectively discard
┊│     toz:x A retained-in-commit.txt
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
    env.but("discard @ uncommitted.txt")
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
╭┄ @ [uncommitted]
┊   ln A uncommitted.txt
┊
┊╭┄ g0 [A]
┊●   opq committed source
┊│     opq:z A committed.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn discard_rejects_committed_changes_from_multiple_commits() {
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
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   lqq second committed source
┊│     lqq:q A second-committed.txt
┊●   ssw first committed source
┊│     ssw:t A first-committed.txt
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_committed_hunk_in_modified_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let original_content = "one
two
three
four
five
six
seven
";
    env.file("file.txt", original_content);
    env.but("commit -m 'Add file'").assert().success();

    env.file("file.txt", format!("first\n{original_content}last\n"));
    env.but("commit -m 'Modify file'").assert().success();
    let modified_commit = env.invoke_git("rev-parse refs/heads/a-branch-1");

    env.but(format!("diff {modified_commit}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 x:u:2 file.txt │
────────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +first
1 ┊ 2 │  one
2 ┊ 3 │  two
3 ┊ 4 │  three

────────────────╮
 x:u:e file.txt │
────────────────╯

@@ -5,3 +6,4 @@
───────────────
5 ┊  6 │  five
6 ┊  7 │  six
7 ┊  8 │  seven
  ┊  9 │ +last

"#]]);

    env.but(format!("discard {modified_commit}:u:2"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded changes from file.txt from xsw to create xsw

"#]]);

    let rewritten_commit = env.invoke_git("rev-parse refs/heads/a-branch-1");
    env.but(format!("diff {rewritten_commit}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 x:u:e file.txt │
────────────────╯

@@ -5,3 +5,4 @@
───────────────
5 ┊ 5 │  five
6 ┊ 6 │  six
7 ┊ 7 │  seven
  ┊ 8 │ +last

"#]]);
}

#[test]
fn discard_single_committed_hunk_in_deleted_file_discards_deletion() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.remove_file("A");
    env.but("commit -m 'Delete file'").assert().success();
    let delete_commit = env.invoke_git("rev-parse refs/heads/A");

    env.but(format!("diff {delete_commit}"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
 s:t:a A │
─────────╯

@@ -1,1 +1,0 @@
───────────────
1 ┊   │ -A

"#]]);

    env.but(format!("discard {delete_commit}:t:a"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded changes from A from sum to create sum

"#]]);

    // commit now has no changes
    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   sum Delete file (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_single_committed_hunk_in_added_file_discards_addition() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("diff tpm")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
 t:t:6 A │
─────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +A

"#]]);

    env.but("discard tpm:t:6")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded changes from A from tpm to create tpm

"#]]);

    // commit now has no changes
    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn discard_final_content_hunk_in_renamed_file_does_not_discard_rename_itself() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);

    let original_content = "one\ntwo\nthree\n";
    env.file("file.txt", original_content);
    env.but("commit -m 'Add file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit xvz on new branch 'a-branch-1'

"#]]);

    env.remove_file("file.txt");
    env.file("renamed_file.txt", format!("{original_content}\nnew"));
    env.but("commit -m 'Rename and edit file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit xlx on branch 'a-branch-1'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   xlx Rename and edit file
┊│     xlx:q R renamed_file.txt
┊●   xvz Add file
┊│     xvz:u A file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("diff xlx")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────────────╮
 xl:q:7 renamed_file.txt │
─────────────────────────╯

@@ -1,3 +1,5 @@
───────────────
1 ┊ 1 │  one
2 ┊ 2 │  two
3 ┊ 3 │  three
  ┊ 4 │ +
  ┊ 5 │ +new

"#]]);

    env.but("discard xlx:q:7")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded changes from renamed_file.txt from xlx to create xlx

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   xlx Rename and edit file
┊│     xlx:q R renamed_file.txt
┊●   xvz Add file
┊│     xvz:u A file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("diff xlx")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────────────╮
 xl:q:e renamed_file.txt │
─────────────────────────╯

No diff available - file is either empty, binary, or too large

"#]]);
}

/// This is here to document this strange corner case that we probably don't want to have. Pending a
/// decision on what to do with "unihunks", for renamed files especially.
#[test]
fn discard_unihunk_in_renamed_file_without_content_discards_rename() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.rename_file("A", "B");
    env.but("commit -m 'Rename file A -> B'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit ylp on branch 'A'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ylp Rename file A -> B
┊│     ylp:p R B
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("diff ylp")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
 y:p:e B │
─────────╯

No diff available - file is either empty, binary, or too large

"#]]);

    env.but("discard ylp:p:e")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Discarded changes from B from ylp to create ylp

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   ylp Rename file A -> B (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
    env.but("diff ylp")
        .assert()
        .success()
        .stdout_eq(snapbox::str![""]);
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
────────────────╮
 lw:2 hunks.txt │
────────────────╯

@@ -1,4 +1,4 @@
───────────────
1 ┊   │ -first
  ┊ 1 │ +firsta
2 ┊ 2 │  line
3 ┊ 3 │  line
4 ┊ 4 │  line

────────────────╮
 lw:e hunks.txt │
────────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -last
   ┊  9 │ +lasta

"#]]);

    env.but("discard lw:2")
        .assert()
        .success()
        .stdout_eq("Discarded uncommitted changes from hunks.txt\n");

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 lw:e hunks.txt │
────────────────╯

@@ -6,4 +6,4 @@
───────────────
 6 ┊  6 │  line
 7 ┊  7 │  line
 8 ┊  8 │  line
 9 ┊    │ -last
   ┊  9 │ +lasta

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
fn discard_defaults_to_uncommitted_area() {
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
╭┄ @ [uncommitted] (no changes)
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
