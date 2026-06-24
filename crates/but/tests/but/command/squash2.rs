use crate::utils::Sandbox;

fn one_branch_three_commits() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");

    env.but("_commit2 -m 'add one' one").assert().success();
    env.but("_commit2 -m 'add two' two").assert().success();
    env.but("_commit2 -m 'add three' three").assert().success();

    env
}

fn two_branches() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.file("two", "content of two");
    env.file("three", "content of three");
    env.file("four", "content of four");

    env.but("_commit2 -b one -m 'add one' one")
        .assert()
        .success();
    env.but("_commit2 -b one -m 'add two' two")
        .assert()
        .success();

    env.but("_commit2 -b second -m 'add three' three")
        .assert()
        .success();
    env.but("_commit2 -b second -m 'add four' four")
        .assert()
        .success();

    env
}

#[test]
fn squash_two_commits() {
    let env = one_branch_three_commits();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   f55169f add three
┊│     f5:or A three
┊●   f63361f add two
┊│     f6:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_squash2 f55169f --target f63361f --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed f55169f into f63361f to create 7251301

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   7251301 squashed
┊│     72:or A three
┊│     72:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("undo").assert().success();

    env.but("_squash2 f55169f --target f63361f --message 'squashed' --format json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "new_commit": "725130139e9f0178e29afbe9eff6a988afbca3fa"
}

"#]]);

    env.but("undo").assert().success();

    env.but("_squash2 f55169f --target f63361f --message 'squashed' --format shell")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
725130139e9f0178e29afbe9eff6a988afbca3fa

"#]]);
}

#[test]
fn squash_multiple_sources() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f f63361f --target ea345ba --message 'squashed'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed f55169f, f63361f into ea345ba to create e355a10

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   e355a10 squashed
┊│     e3:kl A one
┊│     e3:or A three
┊│     e3:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_target_message() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f --target f63361f --use-target-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 5ab5165 author 2000-01-01 00:00:00 +0000
┊│     add two
┊│     5a:or A three
┊│     5a:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn use_source_message() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f --target f63361f --use-source-message")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● c441d34 author 2000-01-01 00:00:00 +0000
┊│     add three
┊│     c4:or A three
┊│     c4:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch() {
    let env = one_branch_three_commits();

    env.but("_squash2 a-branch-1 -m 'squashed a branch'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![["
Squashed branch 'a-branch-1' to create commit a694042

"]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● a694042 author 2000-01-01 00:00:00 +0000
┊│     squashed a branch
┊│     a6:kl A one
┊│     a6:or A three
┊│     a6:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch_into_commit_on_same_branch() {
    let env = one_branch_three_commits();

    env.but("_squash2 a-branch-1 -t f63361f --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' to create commit 17b59a2

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 17b59a2 author 2000-01-01 00:00:00 +0000
┊│     add two
┊│     17:kl A one
┊│     17:or A three
┊│     17:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_whole_branch_into_commit_on_other_branch() {
    let env = one_branch_three_commits();

    env.but("_commit2 -b target-branch -m 'new commit on new branch'")
        .assert()
        .success();

    env.file("file", "new file");
    env.but("_commit2 file -b add-file-branch -m 'add file'")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [a-branch-1]
┊● f55169f author 2000-01-01 00:00:00 +0000
┊│     add three
┊│     f5:or A three
┊● f63361f author 2000-01-01 00:00:00 +0000
┊│     add two
┊│     f6:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┊╭┄ta [target-branch]
┊● d1d6a19 author 2000-01-01 00:00:00 +0000 (no changes)
┊│     new commit on new branch
├╯
┊
┊╭┄fi [add-file-branch]
┊● e528488 author 2000-01-01 00:00:00 +0000
┊│     add file
┊│     e5:qs A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_squash2 a-branch-1 add-file-branch -t d1d6a19 --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branches 'a-branch-1', 'add-file-branch' to create commit 44aa30a

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄ta [target-branch]
┊● 44aa30a author 2000-01-01 00:00:00 +0000
┊│     new commit on new branch
┊│     44:qs A file
┊│     44:kl A one
┊│     44:or A three
┊│     44:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_multiple_branches_into_commit_on_one_of_the_branch_sources() {
    let env = one_branch_three_commits();

    env.but("_commit2 -b target-branch -m 'target commit'")
        .assert()
        .success();
    env.but("_commit2 -b target-branch -m 'random commit on target-branch'")
        .assert()
        .success();

    env.file("file", "new file");
    env.but("_commit2 file -b add-file-branch -m 'add file'")
        .assert()
        .success();

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [a-branch-1]
┊● f55169f author 2000-01-01 00:00:00 +0000
┊│     add three
┊│     f5:or A three
┊● f63361f author 2000-01-01 00:00:00 +0000
┊│     add two
┊│     f6:tw A two
┊● ea345ba author 2000-01-01 00:00:00 +0000
┊│     add one
┊│     ea:kl A one
├╯
┊
┊╭┄ta [target-branch]
┊● a489b93 author 2000-01-01 00:00:00 +0000 (no changes)
┊│     random commit on target-branch
┊● 561a8d8 author 2000-01-01 00:00:00 +0000 (no changes)
┊│     target commit
├╯
┊
┊╭┄fi [add-file-branch]
┊● e528488 author 2000-01-01 00:00:00 +0000
┊│     add file
┊│     e5:qs A file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_squash2 target-branch a-branch-1 add-file-branch -t 561a8d8 --use-target-message")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branches 'target-branch', 'a-branch-1', 'add-file-branch' to create commit 0653794

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄ta [target-branch]
┊● 0653794 author 2000-01-01 00:00:00 +0000
┊│     target commit
┊│     06:qs A file
┊│     06:kl A one
┊│     06:or A three
┊│     06:tw A two
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

    env.but("_squash2 a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![["
Squashed branch 'a-branch-1' to create commit 7b3d915

"]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● 7b3d915 author 2000-01-01 00:00:00 +0000
┊│     message from editor
┊│     7b:kl A one
┊│     7b:or A three
┊│     7b:tw A two
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

    env.but("_squash2 a-branch-1")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'a-branch-1' to create commit abb21d9

"#]]);

    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊● abb21d9 author 2000-01-01 00:00:00 +0000
┊│     add one  add three  add two
┊│     ab:kl A one
┊│     ab:or A three
┊│     ab:tw A two
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn cannot_squash_into_branches() {
    let env = one_branch_three_commits();

    env.but("_squash2 a-branch-1 --target a-branch-1")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find commit: 'a-branch-1'

Hint: --target must always target a commit on an applied branch

"#]]);
}

#[test]
fn cannot_squash_nothing() {
    let env = one_branch_three_commits();

    env.but("_squash2")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <SOURCES>...

Usage: but _squash2 <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn cannot_squash_only_target() {
    let env = one_branch_three_commits();

    env.but("_squash2 --target f55169f")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  <SOURCES>...

Usage: but _squash2 --target <TARGET> <SOURCES>...

For more information, try '--help'.

"#]]);
}

#[test]
fn cannot_mix_sources() {
    let env = one_branch_three_commits();

    env.but("_squash2 a-branch-1 f55169f --target ea345ba")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Cannot mix different types of sources. Got both branches and commits

"#]]);
}

#[test]
fn cannot_squash_multiple_commits_without_target() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f ea345ba")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: When --target isn't used the source must be exactly one branch

"#]]);
}

#[test]
fn cannot_squash_multiple_branches_without_target() {
    let env = one_branch_three_commits();

    env.but("_commit2 --no-message -b second-branch")
        .assert()
        .success();

    env.but("_squash2 a-branch-1 second-branch")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: When --target isn't used the source must be exactly one branch

"#]]);
}

#[test]
fn cannot_squash_branch_with_just_one_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("one", "content of one");
    env.but("_commit2 -m 'add one' one -b the-branch")
        .assert()
        .success();

    env.but("_squash2 the-branch -u")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Need at least 2 commits to squash

"#]]);
}

#[test]
fn cannot_squash_commit_into_itself() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f -t f55169f")
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

    env.but("_squash2 empty-branch")
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

    env.but("_commit2 -m 'target commit'").assert().success();

    env.but("branch new empty-branch").assert().success();

    env.but("_squash2 empty-branch -t 561a8d8")
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
    env.but("_commit2 -m 'add file'").assert().success();

    env.file("file.txt", "changed file content");
    env.but("_commit2 -m 'change file'").assert().success();

    env.remove_file("file.txt");
    env.but("_commit2 -m 'remove file'").assert().success();

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   d5e51af remove file
┊│     d5:uv D file.txt
┊●   5b59611 change file
┊│     5b:uv M file.txt
┊●   11a2a8a add file
┊│     11:uv A file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_squash2 d5e51af -t 11a2a8a")
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
╭┄zz [uncommitted] (no changes)
┊
┊╭┄ne [one]
┊●   f63361f add two
┊│     f6:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┊╭┄se [second]
┊●   d15f721 add four
┊│     d1:qk A four
┊●   66a5286 add three
┊│     66:or A three
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("unapply second").assert().success();

    env.but("_squash2 f63361f -t d15f721")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find commit: 'd15f721'

Hint: --target must always target a commit on an applied branch

"#]]);
}

#[test]
fn cannot_squash_unapplied_branch() {
    let env = two_branches();

    env.but("unapply second").assert().success();

    env.but("_squash2 second")
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
╭┄zz [uncommitted] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
┊│     94:tm A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_squash2 A -t 9477ae7")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Need at least 2 commits to squash

"#]]);
}

#[test]
fn squash_with_duplicate_commit_sources() {
    let env = one_branch_three_commits();

    env.but("_squash2 f55169f f55169f -t f63361f -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed f55169f into f63361f to create 5ab5165

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄br [a-branch-1]
┊●   5ab5165 add two
┊│     5a:or A three
┊│     5a:tw A two
┊●   ea345ba add one
┊│     ea:kl A one
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn squash_with_duplicate_branch_sources() {
    let env = two_branches();

    env.but("_squash2 one one -t d15f721 -u")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Squashed branch 'one' to create commit 00e6751

"#]]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄zz [uncommitted] (no changes)
┊
┊╭┄se [second]
┊●   00e6751 add four
┊│     00:qk A four
┊│     00:kl A one
┊│     00:tw A two
┊●   66a5286 add three
┊│     66:or A three
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}
