use but_testsupport::invoke_bash_at_dir;
use snapbox::IntoData;

use crate::{
    command::util::{add_dirty_worktree, add_worktree_with_commit},
    utils::{CommandExt, Sandbox},
};

/// A sandbox on `two-stacks` after its first read, so that worktrees added afterwards start
/// out active.
fn adopted_sandbox() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("worktree list").assert().success();
    env
}

/// Add the worktree `name` at `commit` with the extra `git worktree add` `flags`, stamping its
/// reflog entries with `committer_date`.
fn add_worktree_at(env: &Sandbox, name: &str, flags: &str, commit: &str, committer_date: &str) {
    let wt = env.app_data_dir().join("worktrees");
    invoke_bash_at_dir(
        &format!(
            r#"GIT_COMMITTER_DATE="{committer_date}" git worktree add -q {flags} "{wt}/{name}" {commit}"#,
            wt = wt.display()
        ),
        env.projects_root(),
    );
}

#[test]
fn list_previews_archived_worktrees_and_sorts_by_recency() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    // Worktrees that predate the first read are adopted as archived.
    for day in 3..=6 {
        add_worktree_at(
            &env,
            &format!("old-{day}"),
            &format!("-b old-{day}"),
            "main",
            &format!("2000-01-0{day} 00:00:00 +0000"),
        );
    }

    // Newest first, cut off after three.
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
(none)

Archived worktrees
old-6 - [..]/worktrees/old-6
old-5 - [..]/worktrees/old-5
old-4 - [..]/worktrees/old-4
and 1 more... Use `--archived` to list all.

"#]]);

    // Created after adoption, so active. The detached one and the one whose branch name
    // differs from its directory name show what they have checked out.
    add_worktree_at(
        &env,
        "wt-feature",
        "-b wt-feature",
        "A",
        "2000-01-07 00:00:00 +0000",
    );
    add_worktree_at(&env, "wt-at", "--detach", "B", "2000-01-08 00:00:00 +0000");
    add_worktree_at(
        &env,
        "mismatch",
        "-b qux",
        "main",
        "2000-01-09 00:00:00 +0000",
    );

    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
qu mismatch (refs/heads/qux) - [..]/worktrees/mismatch
i0 wt-at (detached) - [..]/worktrees/wt-at
wt wt-feature - [..]/worktrees/wt-feature

Archived worktrees
old-6 - [..]/worktrees/old-6
old-5 - [..]/worktrees/old-5
old-4 - [..]/worktrees/old-4
and 1 more... Use `--archived` to list all.

"#]]);
    // Without a subcommand it lists as well.
    env.but("worktree")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
qu mismatch (refs/heads/qux) - [..]/worktrees/mismatch
i0 wt-at (detached) - [..]/worktrees/wt-at
wt wt-feature - [..]/worktrees/wt-feature

Archived worktrees
old-6 - [..]/worktrees/old-6
old-5 - [..]/worktrees/old-5
old-4 - [..]/worktrees/old-4
and 1 more... Use `--archived` to list all.

"#]]);

    env.but("worktree list --archived")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Archived worktrees
old-6 - [..]/worktrees/old-6
old-5 - [..]/worktrees/old-5
old-4 - [..]/worktrees/old-4
old-3 - [..]/worktrees/old-3

"#]]);
    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
qu mismatch (refs/heads/qux) - [..]/worktrees/mismatch
i0 wt-at (detached) - [..]/worktrees/wt-at
wt wt-feature - [..]/worktrees/wt-feature

"#]]);
    env.but("worktree list --active --archived")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
qu mismatch (refs/heads/qux) - [..]/worktrees/mismatch
i0 wt-at (detached) - [..]/worktrees/wt-at
wt wt-feature - [..]/worktrees/wt-feature

Archived worktrees
old-6 - [..]/worktrees/old-6
old-5 - [..]/worktrees/old-5
old-4 - [..]/worktrees/old-4
old-3 - [..]/worktrees/old-3

"#]]);

    // JSON is never cut off.
    env.but("--json worktree list")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "active": [
    {
      "id": "qu",
      "name": "mismatch",
      "refName": "refs/heads/qux",
      "path": "[..]/worktrees/mismatch",
      "updatedAtMs": 947376000000
    },
    {
      "id": "i0",
      "name": "wt-at",
      "refName": null,
      "path": "[..]/worktrees/wt-at",
      "updatedAtMs": 947289600000
    },
    {
      "id": "wt",
      "name": "wt-feature",
      "refName": "refs/heads/wt-feature",
      "path": "[..]/worktrees/wt-feature",
      "updatedAtMs": 947203200000
    }
  ],
  "archived": [
    {
      "id": null,
      "name": "old-6",
      "refName": "refs/heads/old-6",
      "path": "[..]/worktrees/old-6",
      "updatedAtMs": 947116800000
    },
    {
      "id": null,
      "name": "old-5",
      "refName": "refs/heads/old-5",
      "path": "[..]/worktrees/old-5",
      "updatedAtMs": 947030400000
    },
    {
      "id": null,
      "name": "old-4",
      "refName": "refs/heads/old-4",
      "path": "[..]/worktrees/old-4",
      "updatedAtMs": 946944000000
    },
    {
      "id": null,
      "name": "old-3",
      "refName": "refs/heads/old-3",
      "path": "[..]/worktrees/old-3",
      "updatedAtMs": 946857600000
    }
  ]
}

"#]]);
}

#[test]
fn archive_and_unarchive_by_id_or_name() {
    let env = adopted_sandbox();
    add_worktree_with_commit(&env, "wt-feature", "A");
    let sentinel = env.context().project_data_dir.join("REFRESH");
    let watcher_token = but_project_handle::process_sentinel_token();

    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-feature - [..]/worktrees/wt-feature

"#]]);

    std::fs::write(&sentinel, "").unwrap();
    env.but("worktree archive wt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Successfully archived wt-feature

"#]]);
    let writer = std::fs::read_to_string(&sentinel).unwrap();
    assert!(
        !writer.is_empty() && writer != watcher_token,
        "archiving tells the apps to re-read the worktree listing and workspace"
    );
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
(none)

Archived worktrees
wt-feature - [..]/worktrees/wt-feature

"#]]);

    // Archived worktrees have no ID, so the name is the way to address them.
    std::fs::write(&sentinel, "").unwrap();
    env.but("worktree unarchive wt-feature")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Successfully unarchived wt-feature

"#]]);
    let writer = std::fs::read_to_string(&sentinel).unwrap();
    assert!(
        !writer.is_empty() && writer != watcher_token,
        "unarchiving tells the apps to re-read as well"
    );
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-feature - [..]/worktrees/wt-feature

Archived worktrees
(none)

"#]]);

    std::fs::write(&sentinel, "").unwrap();
    env.but("worktree archive nope")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find worktree: 'nope'

Hint: Run `but worktree list` for the worktrees and their IDs.

"#]]);
    assert_eq!(
        std::fs::read_to_string(&sentinel).unwrap(),
        "",
        "a failed archive tells the apps nothing"
    );

    env.but("--json worktree archive wt")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "name": "wt-feature",
  "archived": true
}

"#]]);
}

/// Agent harnesses make a worktree before their first `but` call, so the first read adopts it as
/// archived. Running `but` in it unarchives it, or no command there could see its changes.
#[test]
fn running_but_in_an_archived_worktree_unarchives_it() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    let wt_dir = add_dirty_worktree(&env, "wt-feature", "A");
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
(none)

Archived worktrees
wt-feature - [..]/worktrees/wt-feature

"#]]);

    env.but("diff")
        .current_dir(&wt_dir)
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 nl:a A note.txt │
─────────────────╯

@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +dirty

"#]]);
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-feature - [..]/worktrees/wt-feature

Archived worktrees
(none)

"#]]);
}

#[test]
fn remove_of_a_dirty_checkout_hints_at_a_commit_that_works() {
    let env = adopted_sandbox();
    let worktrees = env.app_data_dir().join("worktrees");

    // Archived, so it has no `<name>:@`, and at a path that needs quoting.
    add_worktree_at(
        &env,
        "wt dirty",
        "-b wt-dirty",
        "A",
        "2000-01-03 00:00:00 +0000",
    );
    std::fs::write(worktrees.join("wt dirty/note.txt"), "dirty\n").unwrap();
    env.but("worktree archive wt-dirty").assert().success();
    env.but("worktree remove wt-dirty")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-dirty has uncommitted changes

Hint: Run `but -C '[..]/worktrees/wt dirty' commit -m "<message>"` to keep them, or `but worktree remove wt-dirty --force` to discard them

"#]]);
    env.but(format!(
        "-C '{}' commit -m keep",
        worktrees.join("wt dirty").display()
    ))
    .assert()
    .success();
    env.but("worktree remove wt-dirty").assert().success();

    // Detached, so it has no branch to commit to until `-b` names one.
    add_worktree_at(&env, "wt-at", "--detach", "B", "2000-01-04 00:00:00 +0000");
    std::fs::write(worktrees.join("wt-at/note.txt"), "dirty\n").unwrap();
    env.but("worktree remove wt-at")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-at has uncommitted changes

Hint: Run `but -C [..]/worktrees/wt-at commit -b <new-branch> -m "<message>"` to keep them, or `but worktree remove wt-at --force` to discard them

"#]]);
    env.but(format!(
        "-C {} commit -b kept -m keep",
        worktrees.join("wt-at").display()
    ))
    .assert()
    .success();
    env.but("worktree remove wt-at").assert().success();
}

#[test]
fn remove_of_a_checkout_with_a_submodule_needs_force() {
    let env = adopted_sandbox();
    let path = add_worktree_with_commit(&env, "wt-sub", "A");
    invoke_bash_at_dir(
        &format!(
            r#"git -c protocol.file.allow=always submodule add -q "{}" sub"#,
            env.projects_root().display()
        ),
        &path,
    );

    env.but("worktree remove wt-sub")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-sub contains an initialized submodule

Hint: Run `but worktree remove wt-sub --force` to remove it anyway

"#]]);
    env.but("worktree remove wt-sub --force").assert().success();
}

#[test]
fn remove_requires_force_for_a_dirty_checkout() {
    let env = adopted_sandbox();
    add_dirty_worktree(&env, "wt-dirty", "A");
    add_worktree_with_commit(&env, "wt-clean", "B");
    env.but("worktree archive wt-clean").assert().success();

    env.but("worktree remove wt-dirty")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree wt-dirty has uncommitted changes

Hint: Run `but -C [..]/worktrees/wt-dirty commit -m "<message>"` to keep them, or `but worktree remove wt-dirty --force` to discard them

"#]]);
    // `wt` is a default alias for `worktree`.
    env.but("wt remove nope")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find worktree: 'nope'

Hint: Run `but worktree list` for the worktrees and their IDs.

"#]]);
    env.but("worktree remove -f wt-dirty")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree wt-dirty

"#]]);
    // Archived worktrees can be removed too.
    env.but("worktree remove wt-clean")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree wt-clean

"#]]);
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
(none)

Archived worktrees
(none)

"#]]);

    // The branch survives like with `git worktree remove`, and a worktree re-created under a
    // removed name starts out active rather than inheriting the archived state.
    add_worktree_at(
        &env,
        "wt-clean",
        "",
        "wt-clean",
        "2000-01-03 00:00:00 +0000",
    );
    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-clean - [..]/worktrees/wt-clean

"#]]);
}

/// `git worktree remove` takes the checkout's path, so `but worktree remove` does too.
#[test]
fn remove_by_path() {
    let env = adopted_sandbox();
    let absolute = add_worktree_with_commit(&env, "wt-absolute", "A");
    // Like the worktrees agent harnesses make inside the main checkout.
    invoke_bash_at_dir(
        "git worktree add -q -b wt-relative .claude/worktrees/wt-relative B",
        env.projects_root(),
    );

    env.but(format!("worktree remove '{}'", absolute.display()))
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree wt-absolute

"#]]);
    // A relative path is taken from the directory `-C` names, with or without a trailing slash.
    env.but(format!(
        "-C '{}' worktree remove .claude/worktrees/wt-relative/",
        env.projects_root().display()
    ))
    .current_dir(env.app_data_dir())
    .assert()
    .success()
    .stderr_eq(snapbox::str![])
    .stdout_eq(snapbox::str![[r#"
Removed worktree wt-relative

"#]]);
    // Either way the branch survives.
    snapbox::assert_data_eq!(
        env.invoke_git("branch --list --format=%(refname:short) wt-*"),
        snapbox::str![[r#"
wt-absolute
wt-relative
"#]]
    );

    // A directory that isn't a worktree, like the main checkout, still isn't one.
    env.but(format!(
        "worktree remove '{}'",
        env.projects_root().display()
    ))
    .assert()
    .failure()
    .stdout_eq(snapbox::str![])
    .stderr_eq(snapbox::str![[r#"
Error: Could not find worktree: '[..]'

Hint: Run `but worktree list` for the worktrees and their IDs.

"#]]);
}

/// `git worktree move` keeps a worktree's name, so a name can also be the relative path of another
/// worktree's checkout. The name wins, or `--force` could discard the wrong checkout.
#[test]
fn remove_prefers_a_name_over_a_path() {
    let env = adopted_sandbox();
    add_worktree_with_commit(&env, "new", "A");
    invoke_bash_at_dir(
        "git worktree add -q -b moved a-moved B && git worktree move a-moved new",
        env.projects_root(),
    );

    env.but("worktree remove new")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree new

"#]]);
    assert!(
        env.projects_root().join("new").is_dir(),
        "the worktree `a-moved`, checked out at ./new, is left alone"
    );
}

/// Git keeps a worktree whose checkout was deleted behind its back, hidden from everything else
/// here, until it is removed by name, path or the branch it has checked out.
#[test]
fn remove_forgets_a_worktree_whose_checkout_is_gone() {
    let env = adopted_sandbox();
    let by_name = add_worktree_with_commit(&env, "by-name", "A");
    let by_path = add_worktree_with_commit(&env, "by-path", "A");
    let locked = add_worktree_with_commit(&env, "locked", "A");
    env.invoke_git(&format!("worktree lock {}", locked.display()));
    env.but("worktree new feat/by-branch").assert().success();
    let by_branch = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("feat-by-branch");
    for checkout in [&by_name, &by_path, &locked, &by_branch] {
        std::fs::remove_dir_all(checkout).unwrap();
    }

    for (arg, name) in [
        ("by-name".to_owned(), "by-name"),
        (format!("'{}'", by_path.display()), "by-path"),
        ("feat/by-branch".to_owned(), "feat-by-branch"),
    ] {
        env.but(format!("worktree remove {arg}"))
            .assert()
            .success()
            .stderr_eq(snapbox::str![])
            .stdout_eq(format!("Removed worktree {name}\n"));
    }

    // The lock may mean the checkout is only unmounted, and nothing can tell if it has changes.
    env.but("worktree remove locked")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Worktree locked is locked and its checkout is unavailable; use --force to forget it

"#]]);
    env.but("worktree remove locked --force").assert().success();

    assert_eq!(
        std::fs::read_dir(env.projects_root().join(".git/worktrees"))
            .map(Iterator::count)
            .unwrap_or_default(),
        0,
        "Git no longer knows any of them"
    );
}

/// A name can also be the branch of another worktree, and the name wins there too.
#[test]
fn remove_prefers_a_name_over_a_branch() {
    let env = adopted_sandbox();
    let wt = env.app_data_dir().join("worktrees");
    env.invoke_git(&format!(
        "worktree add -q -b second {}/first A",
        wt.display()
    ));
    env.invoke_git(&format!(
        "worktree add -q -b third {}/second A",
        wt.display()
    ));

    env.but("worktree remove second")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree second

"#]]);
    assert!(wt.join("first").is_dir(), "the worktree on `second` stays");
}

/// The name wins over a CLI ID here too, or the wrong worktree would go.
#[test]
fn remove_prefers_the_name_of_a_gone_worktree_over_a_cli_id() {
    let env = adopted_sandbox();
    let alive = add_worktree_with_commit(&env, "alive", "A");
    env.but("worktree list --active")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Active worktrees
al alive - [..]/alive

"#]]);
    let gone = env.app_data_dir().join("worktrees/al");
    env.invoke_git(&format!("worktree add -q --detach {} A", gone.display()));
    std::fs::remove_dir_all(&gone).unwrap();

    env.but("worktree remove al")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Removed worktree al

"#]]);
    assert!(alive.is_dir(), "the worktree whose ID is `al` stays");
}

#[test]
fn new_recreates_a_worktree_whose_checkout_is_gone() {
    let env = adopted_sandbox();
    let checkout = add_worktree_with_commit(&env, "gone", "A");
    env.but("worktree archive gone").assert().success();
    std::fs::remove_dir_all(&checkout).unwrap();

    env.but("worktree new gone")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree gone on 'gone' from [..] at [..]/gone

"#]]);
    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%s gone"),
        snapbox::str!["add W"]
    );
    // The archived state went with the old checkout.
    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
[..] gone - [..]/gone

"#]]);
}

/// Creating a worktree on one branch has no business forgetting any other worktree, as a detached
/// one may be all that keeps its commits alive.
#[test]
fn new_keeps_the_gone_worktrees_of_other_branches() {
    let env = adopted_sandbox();
    let other = add_worktree_with_commit(&env, "other", "A");
    std::fs::remove_dir_all(&other).unwrap();

    env.but("worktree new fresh").assert().success();
    assert!(
        env.projects_root().join(".git/worktrees/other").is_dir(),
        "other is still registered"
    );
}

/// A location that can't be read may hold a healthy checkout, so its worktree isn't gone.
#[cfg(unix)]
#[test]
fn a_checkout_that_cannot_be_read_is_not_gone() {
    use std::os::unix::fs::PermissionsExt as _;

    let env = adopted_sandbox();
    let checkout = add_worktree_with_commit(&env, "wt", "A");
    let parent = checkout.parent().unwrap();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(parent).is_ok() {
        return; // Root can read anything, so there is no such location here.
    }
    let new = env.but("worktree new wt").assert();
    let delete = env.but("branch delete wt").assert();
    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755)).unwrap();

    new.failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'wt' is already checked out in [..]/wt

Hint: Run `but worktree new <new-name> -A wt` to start a new branch from it

"#]]);
    delete
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Refusing to delete a branch that is checked out. Worktrees are: [..]/wt

"#]]);
    assert!(checkout.join(".git").is_file());
    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%s wt"),
        snapbox::str!["add W"]
    );
}

#[test]
fn new_checks_out_a_new_branch_at_the_highest_base() {
    let env = adopted_sandbox();
    env.but("worktree new Feature/One")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree feature-one on 'Feature/One' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/feature-one

"#]]);
    let checkout = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("feature-one");
    assert!(
        checkout.join(".git").is_file(),
        "new checkouts live under the user's home, grouped by repository basename"
    );
    // Without a name the branch is canned, and its directory is the slug of that name.
    env.but("--json worktree new")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(
            snapbox::str![[r#"
{
  "name": "a-branch-1",
  "path": "[..]/home/.gitbutler-worktrees/[..]/a-branch-1",
  "refName": "refs/heads/a-branch-1",
  "base": "0dc37334a458df421bf67ea806103bf5004845dd"
}
"#]]
            .is_json(),
        );
    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
br a-branch-1 - [..]/home/.gitbutler-worktrees/[..]/a-branch-1
at feature-one (refs/heads/Feature/One) - [..]/home/.gitbutler-worktrees/[..]/feature-one

"#]]);
    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┊╭┄ br:@ [uncommitted] {a-branch-1} (no changes)
┊├┄ br [a-branch-1] (no commits)
├╯
┊
┊╭┄ at:@ [uncommitted] {feature-one} (no changes)
┊├┄ at [Feature/One] (no commits)
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    let ctx = env.context();
    let (_guard, repo, ws, _db) = ctx.workspace_and_db().unwrap();
    let base = ws.highest_base().expect("the sandbox has a target");
    assert_eq!(
        repo.find_reference("Feature/One")
            .unwrap()
            .peel_to_id()
            .unwrap()
            .detach(),
        base,
        "the branch starts where the applied stacks rest"
    );

    // A branch in the workspace stays there, with or without --above.
    for args in ["A", "A --above 0dc3733"] {
        env.but(format!("worktree new {args}"))
            .assert()
            .failure()
            .stdout_eq(snapbox::str![])
            .stderr_eq(snapbox::str![[r#"
Error: Bad input 'A' for '<NAME>'

Branch 'A' is already in the workspace

Hint: Run `but unapply A` first to move it out of the workspace, or pass a new name

"#]]);
    }
    // What the hint says works.
    env.but("unapply A").assert().success();
    env.but("worktree new A").assert().success();
}

/// Git allows `&` in a branch name, so a hinted command has to quote it for the shell.
#[test]
fn new_hints_quote_the_branch_name() {
    let env = adopted_sandbox();
    env.but("branch new 'in&workspace'").assert().success();
    env.but("worktree new 'in&workspace'")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'in&workspace' for '<NAME>'

Branch 'in&workspace' is already in the workspace

Hint: Run `but unapply 'in&workspace'` first to move it out of the workspace, or pass a new name

"#]]);

    env.but("worktree new 'par&ked'").assert().success();
    env.but("worktree archive par-ked").assert().success();
    env.but("worktree new 'par&ked'")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'par&ked' is already checked out in [..]

Hint: Run `but worktree new <new-name> -A 'par&ked'` to start a new branch from it

"#]]);
}

#[test]
fn new_refuses_an_invalid_name_and_suggests_a_valid_one() {
    let env = adopted_sandbox();
    env.but("worktree new 'a b'")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'a b'

Invalid branch name

Hint: Try 'a-b' instead

"#]]);
    // `git worktree add` takes a path here, so this is what its users try.
    env.but("worktree new ../other --above main")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input '../other'

Invalid branch name

Hint: Try 'other' instead

"#]]);
}

/// Git refuses these, and they are neither in the workspace nor among the active worktrees.
#[test]
fn new_refuses_a_branch_checked_out_elsewhere() {
    let env = adopted_sandbox();
    env.but("worktree new parked").assert().success();
    env.but("worktree archive parked").assert().success();
    env.but("worktree new parked")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'parked' is already checked out in [..]/parked

Hint: Run `but worktree new <new-name> -A parked` to start a new branch from it

"#]]);
    // What the hint says works.
    env.but("worktree new fresh -A parked").assert().success();

    env.invoke_git("checkout -q main");
    env.but("worktree new main")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch 'main' is already checked out in [..]

Hint: Run `but worktree new <new-name> -A main` to start a new branch from it

"#]]);
}

#[test]
fn new_accepts_the_git_worktree_add_spelling() {
    let env = adopted_sandbox();
    env.but("worktree add -b git-habit")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree git-habit on 'git-habit' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/git-habit

"#]]);
}

#[test]
fn new_checks_out_an_existing_branch_outside_the_workspace() {
    let env = adopted_sandbox();
    env.invoke_git("branch loose main");
    env.but("worktree new loose")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree loose on 'loose' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/loose

"#]]);

    // A branch can be checked out in one worktree only.
    env.but("worktree new loose")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'loose' for '<NAME>'

Branch 'loose' is already in worktree 'loose'

Hint: Run `but worktree list` for its path, or pass a new name

"#]]);
    // An existing branch keeps its own history, so it can't be placed.
    env.invoke_git("branch spare main");
    env.but("worktree new spare --above 0dc3733")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'spare' for '<NAME>'

Branch 'spare' already exists

Hint: Omit --above to check out the existing branch, or pass a new name

"#]]);
}

#[test]
fn new_may_take_a_name_that_only_a_remote_has() {
    let env = adopted_sandbox();
    env.invoke_git("update-ref refs/remotes/origin/taken main");

    env.but("worktree new taken")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
}

#[test]
fn new_refuses_a_name_nesting_with_another_branch() {
    let env = adopted_sandbox();
    env.invoke_git("branch fix main");

    env.but("worktree new fix/thing")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Branch name 'fix/thing' collides with existing branch 'fix'

Hint: Pick another name

"#]]);
}

#[test]
fn new_retried_after_a_failing_post_checkout_hook_names_the_worktree() {
    let env = adopted_sandbox();
    env.file(
        ".git/hooks/post-checkout",
        "#!/bin/sh\necho hook-failed >&2\nexit 1\n",
    );
    env.invoke_bash("chmod +x .git/hooks/post-checkout");

    // Like Git, a failing hook keeps the worktree, so the error says it exists.
    env.but("worktree new w --above tpm")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Created the worktree at [..]/home/.gitbutler-worktrees/[..]/w, but its post-checkout hook failed

Caused by:
    Post-checkout hook failed (exit status: 1): hook-failed

"#]]);
    // The same command again blames the name, not `--above`, and says where to look.
    env.but("worktree new w --above tpm")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'w' for '<NAME>'

Branch 'w' is already in worktree 'w'

Hint: Run `but worktree list` for its path, or pass a new name

"#]]);
    env.but("worktree list")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
[..] w - [..]/home/.gitbutler-worktrees/[..]/w

Archived worktrees
(none)

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse w"),
        env.invoke_git("rev-parse A"),
        "the kept worktree's branch starts at the --above commit"
    );
}

#[cfg(unix)]
#[test]
fn new_worktreeinclude_preserves_symlinks_and_skips_special_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(".gitignore", "/included/\n");
    env.but("commit -b my-branch -m 'Ignore included artifacts'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");

    env.file("included/directory/file", "content\n");
    let links = [
        ("file-link", "directory/file"),
        ("directory-link", "directory"),
        ("dangling-link", "missing"),
    ];
    for (name, target) in links {
        std::os::unix::fs::symlink(target, env.projects_root().join("included").join(name))
            .unwrap();
    }
    env.invoke_bash("mkfifo included/pipe");
    env.file(".worktreeinclude", "/included/\n");

    env.but(format!(
        "worktree new included-artifacts --above {}",
        destination_base.trim()
    ))
    .assert()
    .success()
    .stderr_eq(snapbox::str![]);

    let destination = env
        .linked_worktree_root("included-artifacts")
        .join("included");
    assert_eq!(
        std::fs::read_to_string(destination.join("directory/file")).unwrap(),
        "content\n",
        "ordinary ignored files are copied alongside symlinks"
    );
    for (name, target) in links {
        assert_eq!(
            std::fs::read_link(destination.join(name)).unwrap(),
            std::path::Path::new(target),
            "symlinks retain their targets instead of being followed"
        );
    }
    assert!(
        !destination.join("pipe").exists(),
        "special files are skipped without opening them"
    );
}

#[cfg(unix)]
#[test]
fn new_worktreeinclude_preserves_explicitly_included_symlinks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    let outside = tempfile::tempdir().unwrap();

    env.file(".gitignore", "/included/\n");
    env.but("commit -b my-branch -m 'Ignore included artifacts'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");

    env.file("included/directory/file", "source target contents\n");
    env.file(outside.path().join("file"), "outside target contents\n");
    let links = [
        ("file-link", std::path::Path::new("directory/file")),
        ("directory-link", std::path::Path::new("directory")),
        ("dangling-link", std::path::Path::new("missing")),
        ("outside-link", outside.path()),
    ];
    for (name, target) in links {
        std::os::unix::fs::symlink(target, env.projects_root().join("included").join(name))
            .unwrap();
    }
    env.file(
        ".worktreeinclude",
        "included/file-link\nincluded/directory-link\nincluded/dangling-link\nincluded/outside-link\n",
    );

    env.but(format!(
        "worktree new explicit-links --above {}",
        destination_base.trim()
    ))
    .assert()
    .success()
    .stderr_eq(snapbox::str![]);

    let destination = env.linked_worktree_root("explicit-links").join("included");
    // Collect all outcomes so failures show which kinds of root symlinks were not preserved.
    let actual: Vec<_> = links
        .iter()
        .map(|(name, _)| {
            (
                *name,
                std::fs::read_link(destination.join(name)).map_err(|err| err.kind()),
            )
        })
        .collect();
    let expected: Vec<_> = links
        .iter()
        .map(|(name, target)| (*name, Ok(target.to_path_buf())))
        .collect();
    assert_eq!(
        actual, expected,
        "explicit include roots must preserve symlinks, including dangling links and links outside source worktree"
    );
    assert!(
        !destination.join("directory").exists(),
        "including a symlink must not implicitly copy its target"
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("file")).unwrap(),
        "outside target contents\n",
        "copying a symlink must leave external target contents unchanged"
    );
}

#[cfg(unix)]
#[test]
fn new_worktreeinclude_handles_overlapping_roots() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.file(".gitignore", "/included/\n");
    env.but("commit -b my-branch -m 'Ignore artifacts'")
        .assert()
        .success();
    let base = env.invoke_git("rev-parse my-branch");

    env.file("included/file", "artifact\n");
    std::os::unix::fs::symlink("file", env.projects_root().join("included/link")).unwrap();
    env.file(".worktreeinclude", "included/\nincluded/link\n");
    env.but(format!("worktree new overlapping --above {}", base.trim()))
        .assert()
        .success();
    assert_eq!(
        std::fs::read_link(
            env.linked_worktree_root("overlapping")
                .join("included/link")
        )
        .unwrap(),
        std::path::Path::new("file"),
        "overlapping roots must copy a symlink only once"
    );
}

#[test]
fn new_worktreeinclude_handles_specific_file_inclusion() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file(".gitignore", "included/\n");
    env.but("commit -m 'Add gitignore'").assert().success();
    env.file("included/ignored/untracked", "copy this artifact\n");

    env.file(".worktreeinclude", "included/ignored/untracked\n");

    env.but("worktree new filtered-artifacts --above qmo")
        .assert()
        .success();

    env.but("status -f").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   wx A .worktreeinclude
┊
┊╭┄ br [a-branch-1]
┊┊
┊┊╭┄ fi:@ [uncommitted] {filtered-artifacts} (no changes)
┊┊├┄ fi [filtered-artifacts] (no commits)
┊├╯
┊●   qmo Add gitignore
┊│     qmo:p A .gitignore
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let linked_worktree_workdir = env.linked_worktree_root("filtered-artifacts");
    assert_eq!(
        std::fs::read_to_string(linked_worktree_workdir.join("included/ignored/untracked"))
            .unwrap(),
        "copy this artifact\n",
        "included, destination-ignored files untracked in both indexes must be copied"
    );
}

#[test]
fn new_worktreeinclude_ignores_negative_patterns_and_wildcards() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file(".gitignore", "file\n");
    env.but("commit -m 'Add gitignore'").assert().success();
    env.file("file", "should not be copied\n");

    env.file(".worktreeinclude", "!file\n*le\n");

    env.but("worktree new filtered-artifacts --above rlx")
        .assert()
        .stderr_eq("")
        .success();

    env.but("status -f").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   wx A .worktreeinclude
┊
┊╭┄ br [a-branch-1]
┊┊
┊┊╭┄ fi:@ [uncommitted] {filtered-artifacts} (no changes)
┊┊├┄ fi [filtered-artifacts] (no commits)
┊├╯
┊●   rlx Add gitignore
┊│     rlx:p A .gitignore
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let linked_worktree_workdir = env.linked_worktree_root("filtered-artifacts");
    assert!(
        !linked_worktree_workdir.join("file").exists(),
        "file should not be copied from negative .worktreeinclude pattern"
    );
}

#[test]
fn new_worktreeinclude_skips_files_blocked_by_tracked_destination_ancestor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(".gitignore", "/my/nice/file\n");
    env.file("my", "destination tracked file\n");
    env.but("commit -b my-branch -m 'Track destination ancestor and ignore artifact'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");

    env.remove_file("my");
    env.but("commit -m 'Remove ancestor file from source'")
        .assert()
        .success();
    env.file("my/nice/file", "source ignored artifact\n");
    // Exercise both direct file inclusion and recursive directory copying.
    for (name, pattern) in [
        ("colliding-file", "my/nice/file\n"),
        ("colliding-directory", "my/\n"),
    ] {
        env.file(".worktreeinclude", pattern);
        env.but(format!(
            "worktree new {name} --above {}",
            destination_base.trim()
        ))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

        let checkout = env.linked_worktree_root(name);
        assert_eq!(
            std::fs::read_to_string(checkout.join("my")).unwrap(),
            "destination tracked file\n",
            "copying must preserve a tracked destination file blocking an artifact's parent directory"
        );
        assert!(
            !checkout.join("my/nice/file").exists(),
            "an artifact must be skipped when its parent path collides with a tracked file"
        );
    }
    assert_eq!(
        env.read_file("my/nice/file").unwrap(),
        "source ignored artifact\n",
        "skipping a colliding artifact must leave source contents unchanged"
    );
}

#[test]
fn new_worktreeinclude_skips_files_blocked_by_tracked_immediate_parent() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(".gitignore", "/my/nice/file\n");
    env.file("my/nice", "destination tracked file\n");
    env.but("commit -b my-branch -m 'Track immediate parent'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");
    env.remove_file("my/nice");
    env.but("commit -m 'Remove immediate parent from source'")
        .assert()
        .success();
    env.file("my/nice/file", "source ignored artifact\n");

    for (name, pattern) in [
        ("blocked-file", "my/nice/file\n"),
        ("blocked-directory", "my/\n"),
    ] {
        env.file(".worktreeinclude", pattern);
        env.but(format!(
            "worktree new {name} --above {}",
            destination_base.trim()
        ))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
        let checkout = env.linked_worktree_root(name);
        assert_eq!(
            std::fs::read_to_string(checkout.join("my/nice")).unwrap(),
            "destination tracked file\n",
            "a tracked file at the artifact's immediate parent must remain intact"
        );
        assert!(
            !checkout.join("my/nice/file").exists(),
            "artifacts blocked by a tracked immediate parent must be skipped"
        );
    }
    assert_eq!(
        env.read_file("my/nice/file").unwrap(),
        "source ignored artifact\n",
        "source artifact must remain intact"
    );
}

#[test]
fn new_worktreeinclude_skips_files_colliding_with_destination_directory() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(".gitignore", "/my/nice\n");
    env.file("my/nice/child", "destination tracked child\n");
    env.invoke_git("add --force my/nice/child");
    env.but("commit -b my-branch -m 'Track child inside destination directory'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");
    env.remove_file("my/nice/child");
    env.but("commit -m 'Remove child from source'")
        .assert()
        .success();
    if env.projects_root().join("my/nice").exists() {
        std::fs::remove_dir(env.projects_root().join("my/nice")).unwrap();
    }
    env.file("my/nice", "source ignored artifact\n");

    for (name, pattern) in [
        ("colliding-file", "my/nice\n"),
        ("colliding-directory", "my/\n"),
    ] {
        env.file(".worktreeinclude", pattern);
        env.but(format!(
            "worktree new {name} --above {}",
            destination_base.trim()
        ))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
        assert_eq!(
            std::fs::read_to_string(env.linked_worktree_root(name).join("my/nice/child")).unwrap(),
            "destination tracked child\n",
            "an ignored file must not replace a directory containing tracked files"
        );
    }
    assert_eq!(
        env.read_file("my/nice").unwrap(),
        "source ignored artifact\n",
        "source artifact must remain intact"
    );
}

#[cfg(unix)]
#[test]
fn new_worktreeinclude_skips_files_beneath_tracked_destination_symlink() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    let outside = tempfile::tempdir().unwrap();
    env.file(
        outside.path().join("nice/file"),
        "outside contents must not change\n",
    );

    env.file(".gitignore", "/my/nice/file\n");
    std::os::unix::fs::symlink(outside.path(), env.projects_root().join("my")).unwrap();
    env.but("commit -b my-branch -m 'Track symlink to outside directory'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");
    env.remove_file("my");
    env.but("commit -m 'Remove symlink from source'")
        .assert()
        .success();
    env.file("my/nice/file", "source ignored artifact\n");

    for (name, pattern) in [
        ("symlink-file", "my/nice/file\n"),
        ("symlink-directory", "my/\n"),
    ] {
        env.file(".worktreeinclude", pattern);
        env.but(format!(
            "worktree new {name} --above {}",
            destination_base.trim()
        ))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
        assert_eq!(
            std::fs::read_link(env.linked_worktree_root(name).join("my")).unwrap(),
            outside.path(),
            "tracked destination symlink must remain intact"
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("nice/file")).unwrap(),
            "outside contents must not change\n",
            "copying must not follow a tracked ancestor symlink outside destination worktree"
        );
    }
    assert_eq!(
        env.read_file("my/nice/file").unwrap(),
        "source ignored artifact\n",
        "source artifact must remain intact"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn new_worktreeinclude_skips_case_alias_of_tracked_destination_symlink() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    // Probe where linked worktrees are created, not the source repository's filesystem.
    std::fs::create_dir_all(env.home_dir()).unwrap();
    let probe = tempfile::tempdir_in(env.home_dir()).unwrap();
    std::fs::write(probe.path().join("case-probe"), "probe").unwrap();
    if !probe.path().join("CASE-PROBE").exists() {
        eprintln!("Skipping case-alias regression: destination filesystem is case-sensitive");
        return;
    }

    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("sentinel"),
        "outside contents must not change\n",
    )
    .unwrap();
    env.file(".gitignore", "/cache/artifact\n");
    std::os::unix::fs::symlink(outside.path(), env.projects_root().join("Cache")).unwrap();
    env.but("commit -b my-branch -m 'Track uppercase symlink to outside directory'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");

    env.remove_file("Cache");
    env.but("commit -m 'Remove symlink from source'")
        .assert()
        .success();
    env.file("cache/artifact", "source ignored artifact\n");

    for (name, pattern) in [
        ("case-alias-file", "cache/artifact\n"),
        ("case-alias-directory", "cache/\n"),
    ] {
        env.file(".worktreeinclude", pattern);
        env.but(format!(
            "worktree new {name} --above {}",
            destination_base.trim()
        ))
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

        let checkout = env.linked_worktree_root(name);
        assert_eq!(
            std::fs::read_link(checkout.join("Cache")).unwrap(),
            outside.path(),
            "tracked destination symlink must remain intact"
        );
        // Leave this leaf absent initially: clonefile would reject an existing leaf,
        // masking the unintended traversal with an EEXIST error instead of copying.
        assert!(
            std::fs::symlink_metadata(outside.path().join("artifact"))
                .is_err_and(|err| err.kind() == std::io::ErrorKind::NotFound),
            "copying through lowercase cache must not follow tracked Cache symlink outside worktree"
        );
        assert_eq!(
            std::fs::read_to_string(outside.path().join("sentinel")).unwrap(),
            "outside contents must not change\n",
            "external contents must remain unchanged"
        );
    }
    assert_eq!(
        env.read_file("cache/artifact").unwrap(),
        "source ignored artifact\n",
        "skipping a destination case alias must leave source artifact intact"
    );
}

#[test]
fn new_worktreeinclude_copies_only_destination_ignored_files_untracked_in_both_indexes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file(
        ".gitignore",
        "/included/ignored/untracked
/included/source/tracked
/included/destination/tracked
",
    );
    env.file(
        "included/destination/tracked",
        "destination committed contents\n",
    );
    env.invoke_git("add --force included/destination/tracked");
    env.but("commit -b my-branch -m 'gitignore and destination/tracked file'")
        .assert()
        .success();
    let destination_base = env.invoke_git("rev-parse my-branch");

    env.remove_file("included/destination/tracked");
    env.file("included/source/tracked", "source tracked contents\n");
    env.invoke_git("add --force included/source/tracked");
    env.but("commit -m 'Remove destination/tracked file and add source/tracked file'")
        .assert()
        .success();

    // now add some garbage data to the destination/tracked file but in the source working
    // directory, so we can detect if we accidentally overwrite the tracked file in the destination.
    env.file(
        "included/destination/tracked",
        "source contents must not overwrite destination\n",
    );
    env.file("included/ignored/untracked", "copy this artifact\n");
    env.file("included/not-ignored", "ordinary untracked contents\n");
    env.file(".worktreeinclude", "included/\n");

    env.but(format!(
        "worktree new filtered-artifacts --above {}",
        destination_base.trim()
    ))
    .assert()
    .success();

    env.but("status -f").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   wx A .worktreeinclude
┊   ls A included/not-ignored
┊
┊╭┄ my [my-branch]
┊●   ulu Remove destination/tracked file and add source/tracked file
┊│     ulu:tt D included/destination/tracked
┊│     ulu:ts A included/source/tracked
┊┊
┊┊╭┄ fi:@ [uncommitted] {filtered-artifacts} (no changes)
┊┊├┄ fi [filtered-artifacts] (no commits)
┊├╯
┊●   xzo gitignore and destination/tracked file
┊│     xzo:p A .gitignore
┊│     xzo:t A included/destination/tracked
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    let linked_worktree_workdir = env.linked_worktree_root("filtered-artifacts");
    assert!(
        !linked_worktree_workdir
            .join("included/not-ignored")
            .exists(),
        "inclusion alone must not copy an ordinary untracked file"
    );
    assert!(
        !linked_worktree_workdir
            .join("included/source/tracked")
            .exists(),
        "files tracked only in source must not be copied"
    );
    assert_eq!(
        std::fs::read_to_string(linked_worktree_workdir.join("included/destination/tracked"))
            .unwrap(),
        "destination committed contents\n",
        "copying must not overwrite files tracked in destination"
    );
    assert_eq!(
        std::fs::read_to_string(linked_worktree_workdir.join("included/ignored/untracked"))
            .unwrap(),
        "copy this artifact\n",
        "included, destination-ignored files untracked in both indexes must be copied"
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("included/destination/tracked")).unwrap(),
        "source contents must not overwrite destination\n",
        "copying must leave source contents unchanged"
    );
}

#[test]
fn new_handles_non_ascii_name() {
    let env = adopted_sandbox();

    env.but("worktree new '日本語'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created worktree wt on '日本語' from 0dc3733 at [..]/wt

"#]]);

    env.but("worktree list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Active worktrees
i0 wt (refs/heads/日本語) - [..]/wt

Archived worktrees
(none)

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┊╭┄ i0:@ [uncommitted] {wt} (no changes)
┊├┄ i0 [日本語] (no commits)
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn new_handles_directory_collision_with_random_suffix() {
    let env = adopted_sandbox();

    // collision because `_` is stripped, but the `my-worktree` stem still remains
    env.but("worktree new 'my-worktree_'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created worktree my-worktree on 'my-worktree_' from 0dc3733 at [..]/my-worktree

"#]]);
    env.but("worktree new 'my-worktree__'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created worktree my-worktree-[..] on 'my-worktree__' from 0dc3733 at [..]/my-worktree-[..]

"#]]);

    // collision because the entire name is stripped, which should result in the `wt` fallback slug
    env.but("worktree new '日本語'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created worktree wt on '日本語' from 0dc3733 at [..]/wt

"#]]);
    env.but("worktree new '日本語日本語'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-[..] on '日本語日本語' from 0dc3733 at [..]/wt-[..]

"#]]);

    env.but("worktree list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Active worktrees
my my-worktree (refs/heads/my-worktree_) - [..]/my-worktree
wo my-worktree-[..] (refs/heads/my-worktree__) - [..]/my-worktree-[..]
i0 wt (refs/heads/日本語) - [..]/wt
j0 wt-[..] (refs/heads/日本語日本語) - [..]/wt-[..]

Archived worktrees
(none)

"#]]);
    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┊╭┄ my:@ [uncommitted] {my-worktree} (no changes)
┊├┄ my [my-worktree_] (no commits)
├╯
┊
┊╭┄ wo:@ [uncommitted] {my-worktree-[..]} (no changes)
┊├┄ wo [my-worktree__] (no commits)
├╯
┊
┊╭┄ i0:@ [uncommitted] {wt} (no changes)
┊├┄ i0 [日本語] (no commits)
├╯
┊
┊╭┄ j0:@ [uncommitted] {wt-[..]} (no changes)
┊├┄ j0 [日本語日本語] (no commits)
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn new_random_suffix_handles_multiple_collisions() {
    let env = adopted_sandbox();

    env.but("worktree new my-worktree").assert().success();
    env.but("worktree new my-worktree_").assert().success();
    env.but("worktree new my-worktree__").assert().success();

    let output = env
        .but("worktree list --json")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let active = json["active"].as_array().unwrap();
    let names: std::collections::HashSet<_> = active
        .iter()
        .map(|worktree| worktree["name"].as_str().unwrap())
        .collect();

    assert_eq!(active.len(), 3, "all three worktrees were created");
    assert_eq!(names.len(), 3, "each worktree has a distinct name");
}

#[test]
fn new_above_a_commit_cli_id() {
    let env = adopted_sandbox();
    env.but("worktree new feature --above tpm")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree feature on 'feature' from [..] at [..]/home/.gitbutler-worktrees/[..]/feature

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse feature"),
        env.invoke_git("rev-parse A"),
        "the new branch starts at the selected commit, not the workspace base"
    );
    let checkout = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("feature");
    assert_eq!(
        std::fs::read(checkout.join("A")).unwrap(),
        std::fs::read(env.projects_root().join("A")).unwrap(),
        "the checkout includes the selected commit's content"
    );
}

#[test]
fn new_above_a_commit_sha_with_a_generated_name() {
    let env = adopted_sandbox();
    let commit = env.invoke_git("rev-parse B");
    env.but(format!("worktree new -A {}", commit.trim()))
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree a-branch-1 on 'a-branch-1' from [..] at [..]/home/.gitbutler-worktrees/[..]/a-branch-1

"#]]);
    assert_eq!(
        env.invoke_git("rev-parse a-branch-1"),
        commit,
        "the short flag accepts a full commit SHA with no branch name"
    );
}

#[test]
fn new_above_any_revision() {
    let env = adopted_sandbox();
    env.invoke_bash("git branch loose $(git commit-tree -m loose 'main^{tree}')");
    // The base that `but status` prints, an applied branch, and a branch outside the workspace.
    for (index, rev) in ["0dc3733", "A", "loose"].into_iter().enumerate() {
        let name = format!("wt-{index}");
        env.but(format!("worktree new {name} -A {rev}"))
            .assert()
            .success()
            .stderr_eq(snapbox::str![]);
        assert_eq!(
            env.invoke_git(&format!("rev-parse {name}")),
            env.invoke_git(&format!("rev-parse {rev}")),
            "the new branch starts at what `{rev}` names"
        );
    }
}

#[test]
fn new_above_rejects_the_workspace_commit() {
    let env = adopted_sandbox();
    env.but("worktree new feature --above HEAD")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'HEAD' for '--above'

Cannot create a worktree from the GitButler workspace commit

Hint: Pass a branch or commit instead, or omit --above to start at the workspace base

"#]]);
}

#[test]
fn new_above_rejects_an_unknown_commit() {
    let env = adopted_sandbox();
    env.but("worktree new feature --above nonexistent")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find commit: 'nonexistent'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn new_above_rejects_a_conflicted_commit() {
    let env = crate::command::util::sandbox_with_conflicted_commit();
    let commit = env.invoke_git("rev-parse A");
    env.but(format!("worktree new feature --above {}", commit.trim()))
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input '[..]' for '--above'

Cannot create a worktree from a conflicted commit

"#]]);
}

#[test]
fn new_from_a_linked_worktree_uses_the_main_repository_basename() {
    let env = adopted_sandbox();
    let root = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap());
    env.but("worktree new first").assert().success();
    env.but("worktree new second")
        .current_dir(root.join("first"))
        .assert()
        .success();
    assert!(
        root.join("second/.git").is_file(),
        "creation from a linked checkout keeps the main repository's directory grouping"
    );

    env.but("worktree remove second").assert().success();
    assert!(
        !root.join("second").exists(),
        "worktree removal also removes checkouts outside the repository"
    );
}

/// A worktree is named by its checkout, never by a branch below it, which it merely holds.
#[test]
fn a_lower_branch_does_not_name_its_worktree() {
    let env = adopted_sandbox();
    crate::command::util::add_worktree_with_lower_branch(&env, "wt-inside", "A");

    // It can't be unapplied like a branch of a stack, as `but unapply` refuses a worktree's.
    env.but("worktree new wt-lower")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'wt-lower' for '<NAME>'

Branch 'wt-lower' is already in worktree 'wt-inside'

Hint: Run `but worktree list` for its path, or pass a new name

"#]]);
    env.but("worktree remove wt-lower")
        .assert()
        .stdout_eq(snapbox::str![""])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find worktree: 'wt-lower'

Hint: Run `but worktree list` for the worktrees and their IDs.

"#]]);
    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-inside - [..]/worktrees/wt-inside

"#]]);
}
