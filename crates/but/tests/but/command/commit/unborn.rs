use crate::utils::Sandbox;

#[test]
fn creating_the_first_commit_in_an_unborn_repo() {
    let env = Sandbox::open_with_default_settings("unborn");
    snapbox::assert_data_eq!(env.git_log(), snapbox::str![""]);

    env.but("status --verbose")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ no commits (main)

Hint: run `but commit` to create the initial commit

"#]]);

    env.but("commit -m 'initial commit'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit lto on branch 'main'

"#]]);

    env.but("status --verbose")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ f2be42d (common base, main, HEAD) 2000-01-02 initial commit

Hint: run `but branch new` to create a new branch to work on

"#]]);

    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
* f2be42d (HEAD -> main) initial commit

"#]]
    );

    // TODO: make this work, requires a bigger change to the oplog internals
    env.but("undo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
No previous operations to undo.

"#]]);
}

#[test]
fn initial_commit_includes_status_after() {
    let env = Sandbox::open_with_default_settings("unborn");

    env.but("commit -m initial --status-after")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit qnt on branch 'main'

╭┄ @ [uncommitted] (no changes)
┊
┴ aa4c8c7 (common base, main, HEAD) 2000-01-02 initial

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[test]
fn initial_commit_in_empty_clone_initializes_target() {
    let env = Sandbox::empty();
    env.invoke_bash(
        "git init --bare --initial-branch=main remote.git\n\
         git clone remote.git repo\n\
         git -C repo config gitbutler.testing.changeId content-hash",
    );

    env.but("-C repo commit -m initial --status-after")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit qnt on branch 'main'

╭┄ @ [uncommitted] (no changes)
┊
┴ aa4c8c7 (common base, main, HEAD) 2000-01-02 initial

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("-C repo status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ aa4c8c7 (common base, main, HEAD) 2000-01-02 initial

Hint: run `but branch new` to create a new branch to work on

"#]]);
}

#[test]
fn first_commit_on_an_orphan_branch_stands_without_a_target() {
    let env = Sandbox::empty();
    env.invoke_bash(
        "git init --bare --initial-branch=main remote.git\n\
         git clone remote.git repo\n\
         git -C repo commit --allow-empty -m first\n\
         git -C repo push origin main\n\
         git -C repo checkout --orphan gh-pages\n\
         echo hi >repo/index.html",
    );

    // main is the target it would infer, but a root commit shares no history with it.
    env.but("-C repo commit -m docs")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created commit [..] on branch 'gh-pages'

"#]]);
    snapbox::assert_data_eq!(
        env.invoke_git("-C repo log --format=%s gh-pages"),
        snapbox::str!["docs"]
    );
}

#[test]
fn initial_commit_synchronizes_the_index() {
    let env = Sandbox::open_with_default_settings("unborn");
    env.file("hello.txt", "hello");

    env.but("commit -m initial")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    snapbox::assert_data_eq!(
        env.invoke_git("show HEAD:hello.txt"),
        snapbox::str!["hello"]
    );
    snapbox::assert_data_eq!(env.invoke_git("status --short"), snapbox::str![""]);
}

#[test]
fn initial_commit_synchronizes_the_index_with_selective_committing() {
    let env = Sandbox::open_with_default_settings("unborn");
    env.file("one", "one");
    env.file("two", "two");

    env.but("commit -m initial one")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    snapbox::assert_data_eq!(env.invoke_git("show HEAD:one"), snapbox::str!["one"]);
    snapbox::assert_data_eq!(env.invoke_git("status --short"), snapbox::str!["?? two"]);
}

#[test]
fn must_commit_to_main_in_unborn_repo() {
    let env = Sandbox::open_with_default_settings("unborn");
    snapbox::assert_data_eq!(env.git_log(), snapbox::str![""]);

    env.but("commit -b not-main -m 'initial commit'")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Creating the first commit on a different branch is not supported

Hint: Run `but commit` without a branch target to commit on the current branch.

"#]]);

    env.but("commit --above main -m 'initial commit'")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Creating the first commit relative to another branch is not supported

Hint: Run `but commit` without a branch target to commit on the current branch.

"#]]);

    env.but("commit -b main -m 'initial commit'")
        .assert()
        .success();
}

#[test]
fn unborn_message_from_editor() {
    let env = Sandbox::open_with_default_settings("unborn");
    env.file("editor.sh", "printf 'commit from editor\\n' > \"$1\"\n");
    let editor_path = env.projects_root().join("editor.sh");

    env.but("commit")
        .env("GIT_EDITOR", format!("sh {}", editor_path.display()))
        .assert()
        .success();

    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%s"),
        snapbox::str!["commit from editor\n"]
    );
}

#[test]
fn commit_infers_target_in_an_existing_repo() {
    let env = Sandbox::open_with_default_settings("first-commit");

    env.but("commit --empty -b feature -m 'new commit'")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%s feature"),
        snapbox::str!["new commit\n"]
    );

    assert_eq!(
        env.invoke_git("rev-parse feature^"),
        env.invoke_git("rev-parse main"),
        "the new branch should start from the inferred target"
    );
}
