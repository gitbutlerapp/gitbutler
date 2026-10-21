use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn creates_empty_repository_without_setup() {
    let env = Sandbox::empty();
    env.file("existing.txt", "keep me\n");
    env.but("init")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Initialized empty Git repository in [..]/.git

"#]])
        .stderr_eq(snapbox::str![]);
    let repo = gix::open(env.projects_root()).unwrap();
    assert!(
        repo.head().unwrap().is_unborn(),
        "init must not create a commit"
    );
    assert_eq!(env.invoke_git("remote"), "", "init must not add remotes");
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("existing.txt")).unwrap(),
        "keep me\n",
        "existing files must remain intact"
    );
    assert!(
        !repo.git_dir().join("gitbutler").exists(),
        "init must not set up a workspace"
    );
    env.but("init")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Git repository already exists in [..]/.git

"#]]);
}

#[test]
fn commit_flag_creates_an_empty_initial_commit() {
    let env = Sandbox::empty();
    env.file("existing.txt", "keep me\n");

    env.but("init --commit")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Initialized empty Git repository in [..]/.git with initial commit [..]

"#]])
        .stderr_eq(snapbox::str![]);

    env.but("init --commit")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Git repository already exists in [..]/.git

"#]])
        .stderr_eq(snapbox::str![]);
    env.but("status --verbose")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   pr A existing.txt
┊
┴ [..] (common base, main, HEAD) 2000-01-02 Initial commit

Hint: run `but branch new` to create a new branch to work on

"#]])
        .stderr_eq(snapbox::str![]);
}

#[test]
fn nested_repository_does_not_select_parent() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let head = env.invoke_git("rev-parse HEAD");
    env.but("-C nested init")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Initialized empty Git repository in [..]/.git

"#]]);
    let repo = gix::open(env.projects_root().join("nested")).unwrap();
    assert!(
        repo.head().unwrap().is_unborn(),
        "nested repository must be empty"
    );
    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head,
        "parent HEAD must not change"
    );
}

#[test]
fn invalid_git_directory_is_not_overwritten() {
    let env = Sandbox::empty();
    env.file(".git/config", "preserve invalid repository\n");
    env.but("init")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Failed to initialize Git repository at .
...
"#]]);
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join(".git/config")).unwrap(),
        "preserve invalid repository\n",
        "init must not overwrite an invalid repository"
    );
}

#[test]
fn json_and_current_directory() {
    let env = Sandbox::empty();
    env.but("-C nested/repo init --json")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]/nested/repo/.git",
  "created": true
}

"#]]);
    assert!(
        gix::open(env.projects_root().join("nested/repo")).is_ok(),
        "-C selects the repository location"
    );
    assert!(
        !env.projects_root().join(".git").exists(),
        "init must not initialize the caller's directory"
    );
}

#[test]
fn existing_history_and_changes_are_preserved() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    env.file("untracked.txt", "keep me\n");
    let head = env.invoke_git("rev-parse HEAD");
    let status = env.invoke_git("status --porcelain");
    let config = std::fs::read(env.projects_root().join(".git/config")).unwrap();
    env.but("init --json")
        .allow_json()
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]/.git",
  "created": false
}

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse HEAD"),
        head,
        "init must preserve HEAD"
    );
    assert_eq!(
        env.invoke_git("status --porcelain"),
        status,
        "init must preserve changes"
    );
    assert_eq!(
        std::fs::read(env.projects_root().join(".git/config")).unwrap(),
        config,
        "init must preserve config"
    );
}
