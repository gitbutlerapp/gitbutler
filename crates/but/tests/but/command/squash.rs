use snapbox::str;

use crate::{
    command::util::{
        branch_commit_cli_id_for_file, branch_commit_cli_ids,
        commit_file_with_worktree_changes_as_two_hunks, commit_two_files_as_two_hunks_each,
        status_json_with_files as status_json,
    },
    utils::{CommandExt, Sandbox},
};

fn one_branch_three_commits() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    env.but("commit -m 'add one' one").assert().success();
    env.but("commit -m 'add two' two").assert().success();
    env.but("commit -m 'add three' three").assert().success();

    env
}

fn two_branches() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");
    env.file("four", "content of four");

    env.but("commit -b one -m 'add one' one").assert().success();
    env.but("commit -b one -m 'add two' two").assert().success();

    env.but("commit -b second -m 'add three' three")
        .assert()
        .success();
    env.but("commit -b second -m 'add four' four")
        .assert()
        .success();

    env
}

fn scenario_with_uncommitted_changes() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    env.but("commit --empty --no-message").assert().success();

    env
}

#[test]
fn squash_two_commits() {
    let env = one_branch_three_commits();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0 --target 1#1 --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed 1 into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 squashed
┊│     1#0:o A three
┊│     1#0:t A two
┊●   1#1 add one
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("squash 1#0 --target 1#1 --message 'squashed' --json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "newCommitId": "725130139e9f0178e29afbe9eff6a988afbca3fa",
  "newCommitChangeId": "1"
}

"#]]);
}

#[test]
fn squash_multiple_sources() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 1#1 --target 1#2 --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed 1, 1 into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 squashed
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_between_oldest_and_newest_commit() {
    let env = one_branch_three_commits();

    // Squashing the oldest commit into the newest one has to carry `one` into the target,
    // even though removing the source rewrites the commits in between.
    env.but("squash 1#2 --target 1#0 --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed 1 into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 squashed
┊│     1#0:k A one
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    // The same squash in the other direction ends up with the same files per commit.
    env.but("squash 1#0 --target 1#2 --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed 1 into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add two
┊│     1#0:t A two
┊●   1#1 squashed
┊│     1#1:k A one
┊│     1#1:o A three
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_target_message() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 --target 1#1 --use-target-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1#0 author 2000-01-01 00:00:00 +0000 (sha 5ab5165)
┊│     add two
┊│     1#0:o A three
┊│     1#0:t A two
┊● 1#1 author 2000-01-01 00:00:00 +0000 (sha ea345ba)
┊│     add one
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_source_message() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 --target 1#1 --use-source-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1#0 author 2000-01-01 00:00:00 +0000 (sha c441d34)
┊│     add three
┊│     1#0:o A three
┊│     1#0:t A two
┊● 1#1 author 2000-01-01 00:00:00 +0000 (sha ea345ba)
┊│     add one
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch() {
    let env = one_branch_three_commits();

    env.but("squash a-branch-1 -m 'squashed a branch'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha a694042)
┊│     squashed a branch
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch_into_commit_on_same_branch() {
    let env = one_branch_three_commits();

    env.but("squash a-branch-1 -t 1#1 --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 17b59a2)
┊│     add two
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch_into_commit_on_other_branch() {
    let env = one_branch_three_commits();

    env.but("commit -b target-branch -m 'new commit on new branch'")
        .assert()
        .success();

    env.file("file", "new file");
    env.but("commit file -b add-file-branch -m 'add file'")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ fi [add-file-branch]
┊● 1#0 author 2000-01-01 00:00:00 +0000 (sha e528488)
┊│     add file
┊│     1#0:q A file
├╯
┊
┊╭┄ ta [target-branch]
┊● 1#1 author 2000-01-01 00:00:00 +0000 (sha d1d6a19) (no changes)
┊│     new commit on new branch
├╯
┊
┊╭┄ br [a-branch-1]
┊● 1#2 author 2000-01-01 00:00:00 +0000 (sha f55169f)
┊│     add three
┊│     1#2:o A three
┊● 1#3 author 2000-01-01 00:00:00 +0000 (sha f63361f)
┊│     add two
┊│     1#3:t A two
┊● 1#4 author 2000-01-01 00:00:00 +0000 (sha ea345ba)
┊│     add one
┊│     1#4:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash a-branch-1 add-file-branch -t 1#1 --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branches 'a-branch-1', 'add-file-branch' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ta [target-branch]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 44aa30a)
┊│     new commit on new branch
┊│     1:q A file
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_multiple_branches_into_commit_on_one_of_the_branch_sources() {
    let env = one_branch_three_commits();

    env.but("commit -b target-branch -m 'target commit'")
        .assert()
        .success();
    env.but("commit -b target-branch -m 'random commit on target-branch'")
        .assert()
        .success();

    env.file("file", "new file");
    env.but("commit file -b add-file-branch -m 'add file'")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ fi [add-file-branch]
┊● 1#0 author 2000-01-01 00:00:00 +0000 (sha e528488)
┊│     add file
┊│     1#0:q A file
├╯
┊
┊╭┄ ta [target-branch]
┊● 1#1 author 2000-01-01 00:00:00 +0000 (sha a489b93) (no changes)
┊│     random commit on target-branch
┊● 1#2 author 2000-01-01 00:00:00 +0000 (sha 561a8d8) (no changes)
┊│     target commit
├╯
┊
┊╭┄ br [a-branch-1]
┊● 1#3 author 2000-01-01 00:00:00 +0000 (sha f55169f)
┊│     add three
┊│     1#3:o A three
┊● 1#4 author 2000-01-01 00:00:00 +0000 (sha f63361f)
┊│     add two
┊│     1#4:t A two
┊● 1#5 author 2000-01-01 00:00:00 +0000 (sha ea345ba)
┊│     add one
┊│     1#5:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash target-branch a-branch-1 add-file-branch -t 1#2 --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branches 'target-branch', 'a-branch-1', 'add-file-branch' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ ta [target-branch]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 0653794)
┊│     target commit
┊│     1:q A file
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_reword_with_editor() {
    let env = one_branch_three_commits();

    env.file(
        ".git/editor.sh",
        "printf 'message from editor\\n' > \"$1\"\n",
    );
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("squash a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha 7b3d915)
┊│     message from editor
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_combine_messages_with_editor() {
    let env = one_branch_three_commits();

    env.file(".git/editor.sh", "true");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("squash a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' into 1

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊● 1 author 2000-01-01 00:00:00 +0000 (sha abb21d9)
┊│     add one  add three  add two
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn cannot_squash_nothing() {
    let env = one_branch_three_commits();

    env.but("squash")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <SOURCES>...

Usage: but squash <SOURCES>...

For more information, try '--help'.

Examples:
  but squash <commit>... -t <other-commit> -m "message"   # squash commits into another commit
  but squash <branch> -m "message"                # squash a branch into a single commit
  but squash <file> -t <commit>                    # move an uncommitted file into a commit
  but squash <commit>:<file> -t <other-commit>     # move a committed file to another commit

"#]]);
}

#[test]
fn cannot_mix_sources() {
    let env = one_branch_three_commits();

    env.but("squash a-branch-1 1#0 --target 1#2")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot mix different types of sources

"#]]);
}

#[test]
fn cannot_squash_multiple_commits_without_target() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 1#2")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: When --target isn't used the source must be exactly one branch

Hint: To squash into the last source, use `but squash 1#0 -t 1#2`

"#]]);
}

#[test]
fn cannot_squash_multiple_branches_without_target() {
    let env = one_branch_three_commits();

    env.but("commit --no-message -b second-branch")
        .assert()
        .success();

    env.but("squash a-branch-1 second-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: When --target isn't used the source must be exactly one branch

Hint: To squash into the last source, use `but squash a-branch-1 -t second-branch`

"#]]);
}

#[test]
fn cannot_squash_branch_with_just_one_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.but("commit -m 'add one' one -b the-branch")
        .assert()
        .success();

    env.but("squash the-branch -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Need at least 2 commits to squash

"#]]);
}

#[test]
fn cannot_squash_commit_into_itself() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 -t 1#0")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot squash a commit into itself

"#]]);
}

#[test]
fn cannot_squash_empty_branch_into_itself() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new empty-branch").assert().success();

    env.but("squash empty-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot squash empty branch into itself

"#]]);
}

#[test]
fn cannot_squash_empty_branch_into_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("commit -m 'target commit'").assert().success();

    env.but("branch new empty-branch").assert().success();

    env.but("squash empty-branch -t 1")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Need at least 2 commits to squash

"#]]);
}

#[test]
fn aborts_on_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file.txt", "file content");
    env.but("commit -m 'add file'").assert().success();

    env.file("file.txt", "changed file content");
    env.but("commit -m 'change file'").assert().success();

    env.remove_file("file.txt");
    env.but("commit -m 'remove file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 remove file
┊│     1#0:u D file.txt
┊●   1#1 change file
┊│     1#1:u M file.txt
┊●   1#2 add file
┊│     1#2:u A file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0 -t 1#2")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot squash commits that would result in merge conflicts

"#]]);
}

#[test]
fn cannot_squash_into_commits_on_unapplied_branches() {
    let env = two_branches();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ se [second]
┊●   1#0 add four
┊│     1#0:q A four
┊●   1#1 add three
┊│     1#1:o A three
├╯
┊
┊╭┄ on [one]
┊●   1#2 add two
┊│     1#2:t A two
┊●   1#3 add one
┊│     1#3:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply second").assert().success();

    // Unapplied commits have no change ID in the workspace map, so use the commit ID intentionally.
    env.but("squash 1#0 -t d15f721")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'd15f721'

Hint: --target must be an applied commit, branch, or zz. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn cannot_squash_unapplied_branch() {
    let env = two_branches();

    env.but("unapply second").assert().success();

    env.but("squash second")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find branch: 'second'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn cannot_squash_branch_with_one_commit_into_that_one_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

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

    env.but("squash A -t tpm")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Need at least 2 commits to squash

"#]]);
}

#[test]
fn squash_with_duplicate_commit_sources() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 1#0 -t 1#1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed 1 into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add two
┊│     1#0:o A three
┊│     1#0:t A two
┊●   1#1 add one
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_with_duplicate_branch_sources() {
    let env = two_branches();

    env.but("squash one one -t 1#0 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'one' into 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ se [second]
┊●   1#0 add four
┊│     1#0:q A four
┊│     1#0:k A one
┊│     1#0:t A two
┊●   1#1 add three
┊│     1#1:o A three
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn amend_uncommitted_files_into_commit() {
    let env = scenario_with_uncommitted_changes();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   kl   A one
┊   or   A three
┊   twop A two
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("squash one two -t 1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   or A three
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message)
┊│     1:k A one
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn amend_all_uncommitted_changes_into_commit() {
    let env = scenario_with_uncommitted_changes();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   kl   A one
┊   or   A three
┊   twop A two
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("squash zz -t 1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 (no commit message)
┊│     1:k A one
┊│     1:o A three
┊│     1:t A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn amend_uncommitted_hunks_into_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let lines = std::iter::repeat_n("line\n", 10).collect::<Vec<_>>();
    env.file("file", lines.concat());

    env.but("commit -b my-branch --no-message")
        .assert()
        .success();

    env.prepend_file("file", "top");
    env.append_file("file", "bottom");

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
qs:9 file│
─────────╯
     1│+topline
   1 2│ line
   2 3│ line
   3 4│ line
─────────╮
qs:d file│
─────────╯
    7  8│ line
    8  9│ line
    9 10│ line
   10   │-line
      11│+bottom

"#]]);

    env.but("squash qs:9 -t 1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────╮
qs:d file│
─────────╯
    8  8│ line
    9  9│ line
   10 10│ line
   11   │-line
      11│+bottom

"#]]);
}

#[test]
fn amend_all_uncommitted_changes_when_zz_is_empty() {
    let env = one_branch_three_commits();

    env.but("squash zz -t 1#0 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn amend_committed_file() {
    let env = one_branch_three_commits();

    env.but("squash 1#0:o -t 1#1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three (no changes)
┊●   1#1 add two
┊│     1#1:o A three
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn cannot_amend_files_from_different_commits() {
    let env = one_branch_three_commits();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0:o 1#1:t -t 1#2 -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: All committed files must come from the same commit. Found files from f55169f and f63361f

"#]]);
}

#[test]
fn cannot_amend_files_in_ways_that_cause_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "file content");
    env.but("commit -m 'add file'").assert().success();

    env.file("file", "changed");
    env.but("commit -m 'change file'").assert().success();

    env.remove_file("file");
    env.but("commit -m 'remove file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 remove file
┊│     1#0:q D file
┊●   1#1 change file
┊│     1#1:q M file
┊●   1#2 add file
┊│     1#2:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0:q -t 1#2 -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Failed to apply changes to destination commit - merge conflict

"#]]);
}

#[test]
fn squash_into_branch_tip() {
    let env = one_branch_three_commits();

    env.file("file", "file content");

    env.but("squash file -t a-branch-1 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Amended 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three
┊│     1#0:q A file
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_into_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "file content");

    env.but("branch new bottom").assert().success();
    env.but("squash file -t bottom -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --target cannot be an empty branch

"#]]);

    // middle and bottom are stil empty even if they're stacked
    env.but("branch new middle -a bottom").assert().success();
    env.but("squash file -t middle -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --target cannot be an empty branch

"#]]);
    env.but("squash file -t bottom -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --target cannot be an empty branch

"#]]);

    env.but("commit --empty -b bottom --no-message")
        .assert()
        .success();
    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   qs A file
┊
┊╭┄ mi [middle] (no commits)
┊│
┊├┄ bo [bottom]
┊●   1 (no commit message) (no changes)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
    // middle should be considered empty even though there are commits on its parent
    env.but("squash file -t middle -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --target cannot be an empty branch

"#]]);
}

#[test]
fn cannot_squash_into_targets_that_dont_exist() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "file content");

    env.but("squash file -t does-not-exist -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'does-not-exist'

Hint: --target must be an applied commit, branch, or zz. Run `but status` for applicable targets.

"#]]);
}

#[test]
fn squash_into_zz_to_uncommit_commit() {
    let env = one_branch_three_commits();

    env.but("squash 1#0 -t zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   or A three
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add two
┊│     1#0:t A two
┊●   1#1 add one
┊│     1#1:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("undo").assert().success();

    env.but("squash 1#0 -t zz --json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#""#]]);
}

#[test]
fn squash_into_zz_to_uncommit_file() {
    let env = one_branch_three_commits();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three
┊│     1#0:o A three
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0:o -t zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted from 1

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   or A three
┊
┊╭┄ br [a-branch-1]
┊●   1#0 add three (no changes)
┊●   1#1 add two
┊│     1#1:t A two
┊●   1#2 add one
┊│     1#2:k A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

#[test]
fn cannot_uncommit_files_in_ways_that_cause_conflicts() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("file", "file content");
    env.but("commit -m 'add file'").assert().success();

    env.file("file", "changed");
    env.but("commit -m 'change file'").assert().success();

    env.remove_file("file");
    env.but("commit -m 'remove file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1#0 remove file
┊│     1#0:q D file
┊●   1#1 change file
┊│     1#1:q M file
┊●   1#2 add file
┊│     1#2:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#2 -t zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot uncommit commits that would result in merge conflicts

"#]]);

    env.but("squash 1#2:q -t zz")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot uncommit hunks that would result in merge conflicts

"#]]);
}

#[test]
fn cannot_use_source_message_with_uncommitted_changes() {
    let env = one_branch_three_commits();

    env.file("file", "file content");

    env.but("squash file -t a-branch-1 --use-source-message")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --use-source-message cannot be used when squashing uncommitted changes

"#]]);

    env.but("squash zz -t a-branch-1 --use-source-message")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --use-source-message cannot be used when squashing uncommitted changes

"#]]);
}

#[test]
fn cannot_use_source_message_when_moving_committed_files() {
    let env = one_branch_three_commits();

    env.but("squash 1#0:o -t 1#1 --use-source-message")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: --use-source-message cannot be used when moving committed changes

"#]]);
}

#[test]
fn committed_file_to_uncommitted_area() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        // .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
...
{
  "uncommittedChanges": [],
  "stacks": [
    {
      "cliId": "i0",
      "assignedChanges": [],
      "branches": [
        {
          "cliId": "g0",
          "name": "A",
          "commits": [
            {
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
            },
            {
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
            },
            {
...
              "changes": [
                {
                  "cliId": "t:t",
                  "filePath": "A",
                  "changeType": "added"
                }
              ]
            }
...

"#]]);

    env.but("squash 1#0:p -t zz").assert().success();

    // Verify that `status` reflects the move.
    env.but("--json status -f")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![""])
        .stdout_eq(snapbox::str![[r#"
{
  "uncommittedChanges": [
    {
      "cliId": "pn",
      "filePath": "b.txt",
      "changeType": "modified"
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
...
              "changes": [
                {
                  "cliId": "1#0:n",
                  "filePath": "a.txt",
                  "changeType": "modified"
                }
              ]
            },
            {
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
            },
            {
...
              "changes": [
                {
                  "cliId": "t:t",
                  "filePath": "A",
                  "changeType": "added"
                }
...
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
...
              "changes": [
                {
                  "cliId": "l:p",
                  "filePath": "B",
                  "changeType": "added"
                }
              ]
            }
...

"#]]);
}

#[test]
fn uncommitted_hunk_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    // Must set metadata to match the scenario
    env.setup_metadata(&["A", "B"]);

    commit_file_with_worktree_changes_as_two_hunks(&env, "A", "a.txt");

    let target_cli_id = branch_commit_cli_ids(&status_json(&env), "A")[0].clone();
    // The amended commit is identified by its change ID, from a freshly built
    // map that knows the post-amend workspace.
    env.but(format!("squash zz:a.txt:#0 -t {target_cli_id} -u"))
        .assert()
        .success();

    // Verify that only one hunk was assigned ("a.txt" still appears in the
    // uncommitted area because there is one hunk still unassigned).
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

    // Verify that the commit indeed received the hunk.
    snapbox::assert_data_eq!(
        env.open_repo()
            .rev_parse_single("A:a.txt")
            .unwrap()
            .object()
            .unwrap()
            .try_into_blob()
            .unwrap()
            .take_data(),
        str![[r#"
firsta
line
line
line
line
line
line
line
last

"#]],
    );
}

// Regression: filenames with dashes should not be misinterpreted as ranges.
// Before the fix, "my-file.txt" would be split on '-' and treated as a range
// from "my" to "file.txt", which would fail.

#[test]
fn uncommitted_hunk_to_commit_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("uncommitted-to-commit.txt", "content\n");

    let before = status_json(&env);
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!(
        "squash uncommitted-to-commit.txt -t {target_cli_id} -u"
    ))
    .assert()
    .success();

    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:s A uncommitted-to-commit.txt
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

#[test]
fn squash_path_prefix_into_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("prefix/a", "content of a\n");
    env.file("prefix/b", "content of b\n");
    env.file("prefixx", "content outside the prefix\n");

    let before = status_json(&env);
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!("squash prefix/ -t {target_cli_id} -u"))
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   tm A prefixx
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:y A prefix/a
┊│     tpm:u A prefix/b
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
}

#[test]
fn uncommitted_area_to_commit_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("zz-to-commit.txt", "content\n");

    let before = status_json(&env);
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!("squash zz -t {target_cli_id} -u"))
        .assert()
        .success();

    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:n A zz-to-commit.txt
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

#[test]
fn uncommitted_to_commit_consumes_renames() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let original = (1..=120)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("rename-source.txt", &original);
    env.but("commit -b A -m 'seed rename source'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("rename-source.txt"),
        env.projects_root().join("rename-target.txt"),
    )
    .unwrap();
    env.file(
        "rename-target.txt",
        original.replace("40\n41\n42\n", "40\nchanged\n42\n"),
    );

    let before = status_json(&env);
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!("squash zz -t {target_cli_id} -u"))
        .assert()
        .success();

    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 seed rename source
┊│     1:q A rename-target.txt
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
    assert_eq!(
        env.invoke_git("status --porcelain"),
        "",
        "expected all zz changes to be committed"
    );
}

#[test]
fn uncommitted_file_to_commit_consumes_renames() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    let original = (1..=120)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    env.file("rename-source-single.txt", &original);
    env.but("commit -b A -m 'seed rename source single'")
        .assert()
        .success();

    std::fs::rename(
        env.projects_root().join("rename-source-single.txt"),
        env.projects_root().join("rename-target-single.txt"),
    )
    .unwrap();
    env.file(
        "rename-target-single.txt",
        original.replace("70\n71\n72\n", "70\nchanged\n72\n"),
    );

    let before = status_json(&env);
    let source_file_cli_id = before["uncommittedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|change| change["filePath"].as_str() == Some("rename-target-single.txt"))
        .and_then(|change| change["cliId"].as_str())
        .expect("renamed uncommitted file should be present in status");
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!("squash {source_file_cli_id} -t {target_cli_id} -u"))
        .assert()
        .success();

    env.but("status -f").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 seed rename source single
┊│     1:x A rename-target-single.txt
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

    let remaining = env.invoke_git("status --porcelain");
    assert_eq!(
        remaining, "",
        "expected selected renamed file to be committed; remaining status:\n{remaining}"
    );
}

#[test]
fn uncommitted_deleted_file_to_commit_keeps_unrelated_deleted_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    env.file("a.txt", "a\n");
    env.file("b.txt", "b\n");
    env.file("c.txt", "c\n");
    env.but("commit -b A -m 'Add a.txt, b.txt, and c.txt'")
        .assert()
        .success();

    std::fs::remove_file(env.projects_root().join("a.txt")).unwrap();
    std::fs::remove_file(env.projects_root().join("b.txt")).unwrap();

    let before = status_json(&env);
    let source_file_cli_id = before["uncommittedChanges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|change| change["filePath"].as_str() == Some("a.txt"))
        .and_then(|change| change["cliId"].as_str())
        .expect("a.txt deletion should be present in the uncommitted area");
    let target_cli_id = branch_commit_cli_ids(&before, "A")[0].clone();

    env.but(format!("squash {source_file_cli_id} -t {target_cli_id} -u"))
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊   pn D b.txt
┊
┊╭┄ g0 [A]
┊●   1 Add a.txt, b.txt, and c.txt
┊│     1:p A b.txt
┊│     1:k A c.txt
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
    assert!(
        !env.projects_root().join("a.txt").exists(),
        "selected a.txt deletion should stay applied to the worktree"
    );
    assert!(
        !env.projects_root().join("b.txt").exists(),
        "unrelated b.txt deletion should stay applied to the worktree"
    );
    assert!(
        env.projects_root().join("c.txt").exists(),
        "untouched c.txt should stay in the worktree"
    );
}

#[test]
fn commit_to_uncommitted_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");

    let before = status_json(&env);
    let commit_cli_ids_before = branch_commit_cli_ids(&before, "A");
    let source_cli_id = commit_cli_ids_before[0].clone();

    env.but(format!("squash {source_cli_id} -t zz"))
        .assert()
        .success();

    let after = status_json(&env);
    let commit_cli_ids_after = branch_commit_cli_ids(&after, "A");

    assert_eq!(
        commit_cli_ids_after.len() + 1,
        commit_cli_ids_before.len(),
        "uncommitting a commit should remove that commit from branch history"
    );
    assert!(
        !commit_cli_ids_after.contains(&source_cli_id),
        "source commit should no longer be present after uncommit"
    );

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted]
┊   nk A a.txt
┊   pn A b.txt
┊
┊╭┄ g0 [A]
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
}

#[test]
fn commit_to_commit_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);

    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "first commit");
    commit_two_files_as_two_hunks_each(&env, "A", "a.txt", "b.txt", "second commit");

    let before = status_json(&env);
    let commit_cli_ids_before = branch_commit_cli_ids(&before, "A");
    let source_cli_id = commit_cli_ids_before[0].clone();
    let target_cli_id = commit_cli_ids_before[1].clone();

    env.but(format!("squash {source_cli_id} -t {target_cli_id} -u"))
        .assert()
        .success();

    let after = status_json(&env);
    let commit_cli_ids_after = branch_commit_cli_ids(&after, "A");
    assert_eq!(
        commit_cli_ids_after.len() + 1,
        commit_cli_ids_before.len(),
        "squashing should reduce commit count by one"
    );
}

#[test]
fn commit_without_message_to_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one.txt", "one.txt contents");
    env.but("commit -m 'add one.txt' one.txt")
        .assert()
        .success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add one.txt
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);

    env.but("commit --empty --no-message").assert().success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message) (no changes)
┊●   1#1 add one.txt
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);

    env.but("squash 1#0 -t 1#1 -u").assert().success();

    env.but("status --no-hint")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add one.txt
┊●   tpm add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

"#]]);
}

#[test]
fn commit_to_commit_without_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("one.txt", "one.txt contents");
    env.but("commit -m 'add one.txt' one.txt")
        .assert()
        .success();
    env.but("commit --empty --no-message").assert().success();

    env.but("squash 1#1 -t 1#0 --use-source-message")
        .assert()
        .success();

    let status = status_json(&env);
    let branch = status["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str().unwrap() == "A")
        .unwrap();
    let commit_messages = branch["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|commit| commit["message"].as_str().unwrap().trim_end_matches('\n'))
        .collect::<Vec<_>>();

    assert_eq!(commit_messages, vec!["add one.txt", "add A"]);
}

#[test]
fn committed_file_to_commit_smoke() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    commit_two_files_as_two_hunks_each(&env, "A", "source-a.txt", "source-b.txt", "source commit");
    commit_two_files_as_two_hunks_each(&env, "A", "target-a.txt", "target-b.txt", "target commit");

    let before = status_json(&env);
    let source_cli_id = branch_commit_cli_id_for_file(&before, "A", "source-a.txt")
        .expect("source commit with file");
    let target_cli_id = branch_commit_cli_id_for_file(&before, "A", "target-a.txt")
        .expect("target commit with file");

    env.but(format!(
        "squash {source_cli_id}:source-a.txt -t {target_cli_id} -u"
    ))
    .assert()
    .success();

    let after = status_json(&env);
    let branch = after["stacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["branches"].as_array().unwrap().iter())
        .find(|branch| branch["name"].as_str() == Some("A"))
        .expect("branch A should remain in status");
    let commit_contains_file = |message: &str, file_path: &str| {
        branch["commits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|commit| {
                commit["message"]
                    .as_str()
                    .is_some_and(|actual| actual.trim_end_matches('\n') == message)
            })
            .unwrap_or_else(|| panic!("commit '{message}' should remain in branch A"))["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["filePath"].as_str() == Some(file_path))
    };
    assert!(
        !commit_contains_file("create source-a.txt and source-b.txt", "source-a.txt"),
        "moved file should be absent from the rewritten source commit"
    );
    assert!(
        commit_contains_file("create target-a.txt and target-b.txt", "source-a.txt"),
        "moved file should be present in the rewritten target commit"
    );
}

#[test]
fn squash_amending_modified_and_renamed_file() {
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

    env.but("squash zz -t 1 -u").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1]
┊●   1 add files
┊│     1:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn doesnt_open_editor_if_no_sources_have_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(".git/editor.sh", "false");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("commit --empty --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message) (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1 -t tpm")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

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
fn doesnt_open_editor_if_no_target_has_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(".git/editor.sh", "false");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("commit --empty --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message) (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash tpm -t 1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 add A
┊│     1:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn doesnt_open_editor_if_both_source_and_target_doesnt_have_a_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file(".git/editor.sh", "false");
    let editor_path = env.projects_root().join(".git/editor.sh");
    let editor_command = format!("sh {}", editor_path.display());

    env.but("commit --empty --no-message").assert().success();
    env.but("commit --empty --no-message").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1#0 (no commit message) (no changes)
┊●   1#1 (no commit message) (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash 1#0 -t 1#1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   1 (no commit message) (no changes)
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squashing_into_branch_that_sits_below_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "content");

    env.but("branch new -a A").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   qs A file
┊
┊╭┄ br [a-branch-1] (no commits)
┊│
┊├┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("squash file -t A").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
┊│
┊├┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
┊│     tpm:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_rejects_merged_upstream_commits() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    // `nyq` is branch A's landed commit, `kyl` is branch B's live commit.
    // Landed commits are rejected both as source and as target.
    env.but("squash nyq --target kyl --use-target-message")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    env.but("squash kyl --target nyq --use-target-message")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);
}

#[test]
fn squash_without_source_implies_uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.file("file", "content");

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
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

    env.but("squash -t tpm")
        .assert()
        .success()
        .stderr_eq(snapbox::str![[r#"

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
┊│     tpm:q A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_uncommit_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings(
        "two-stacks-one-single-and-ready-to-mingle-one-double",
    );
    env.setup_metadata(&["A", "B", "C"]);

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
┊╭┄ h0 [C]
┊●   xwn add C
┊│     xwn:w A C
┊│
┊├┄ i0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash A B C -t zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted 'A', 'B', 'C'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted]
┊   tm A A
┊   pl A B
┊   wx A C
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("undo").assert().success();

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
┊╭┄ h0 [C]
┊●   xwn add C
┊│     xwn:w A C
┊│
┊├┄ i0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_uncommit_empty_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.but("branch new").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ br [a-branch-1] (no commits)
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("squash br -t zz")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Uncommitted 'a-branch-1'

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);
}
