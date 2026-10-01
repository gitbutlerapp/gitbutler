use but_testsupport::invoke_bash_at_dir;
use snapbox::IntoData;

use crate::{
    command::util::{add_dirty_worktree, add_worktree_with_commit, enable_worktree_manipulation},
    utils::{CommandExt, Sandbox},
};

/// A flag-on sandbox on `two-stacks` after its first flag-on read, so that worktrees added
/// afterwards start out active.
fn flag_on_sandbox() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    enable_worktree_manipulation(&env);
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
    enable_worktree_manipulation(&env);
    // Worktrees that predate the first flag-on read are adopted as archived.
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
    let env = flag_on_sandbox();
    add_worktree_with_commit(&env, "wt-feature", "A");
    let sentinel = env.context().project_data_dir.join("INVALIDATE");
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
    assert_eq!(
        but_project_handle::invalidation_by_others(
            &std::fs::read_to_string(&sentinel).unwrap(),
            &watcher_token,
        ),
        ["Worktrees", "Workspace"],
        "archiving notifies the app to refresh the worktree listing and workspace"
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
    assert_eq!(
        but_project_handle::invalidation_by_others(
            &std::fs::read_to_string(&sentinel).unwrap(),
            &watcher_token,
        ),
        ["Worktrees", "Workspace"],
        "unarchiving emits a fresh invalidation as well"
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
        "a failed archive leaves the app's caches valid"
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

#[test]
fn remove_requires_force_for_a_dirty_checkout() {
    let env = flag_on_sandbox();
    add_dirty_worktree(&env, "wt-dirty", "A");
    add_worktree_with_commit(&env, "wt-clean", "B");
    env.but("worktree archive wt-clean").assert().success();

    // Git's own refusal is shown; it is localized, so pin the language.
    env.but("worktree remove wt-dirty")
        .env("LC_ALL", "C")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: fatal: '[..]/worktrees/wt-dirty' contains modified or untracked files, use --force to delete it

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

#[test]
fn refuses_without_the_feature_flag() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("worktree list")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: worktree manipulation is not enabled (featureFlags.worktreeManipulation)

"#]]);
    env.but("worktree archive wt")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: worktree manipulation is not enabled (featureFlags.worktreeManipulation)

"#]]);
    env.but("worktree new")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: worktree manipulation is not enabled (featureFlags.worktreeManipulation)

"#]]);
}

#[test]
fn new_checks_out_a_new_branch_at_the_highest_base() {
    let env = flag_on_sandbox();
    env.but("worktree new Feature/One")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree feature-one on 'Feature/One' from 0dc3733 at [..]/.git/gb-wts/feature-one

"#]]);
    // Without a name the branch is canned, and its directory is the slug of that name.
    env.but("--json worktree new")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(
            snapbox::str![[r#"
{
  "name": "a-branch-1",
  "path": "[..]/.git/gb-wts/a-branch-1",
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
br a-branch-1 - [..]/.git/gb-wts/a-branch-1
at feature-one (refs/heads/Feature/One) - [..]/.git/gb-wts/feature-one

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

    // Names are validated like `but branch new`, and git refuses what already exists.
    env.but("worktree new A")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: A branch named 'A' is already applied

"#]]);
    env.but("worktree new feature-one")
        .env("LC_ALL", "C")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: '[..]/.git/gb-wts/feature-one' already exists

"#]]);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn cow_is_refused_without_creating_a_worktree_on_other_platforms() {
    let env = flag_on_sandbox();
    env.but("worktree new --cow cloned")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: --cow is only supported on macOS

"#]]);
    assert!(
        !env.projects_root().join(".git/gb-wts/cloned").exists(),
        "unsupported platforms must not create a checkout"
    );
    assert!(
        env.context()
            .repo
            .get()
            .unwrap()
            .try_find_reference("cloned")
            .unwrap()
            .is_none(),
        "unsupported platforms must not create a branch"
    );
}

/// Requires the sandbox to reside on a filesystem supporting clonefile(2), typically APFS.
#[cfg(unix)]
#[cfg_attr(not(target_os = "macos"), ignore = "requires macOS clonefile(2)")]
#[test]
fn cow_clones_primary_files_and_index_even_when_invoked_from_a_linked_worktree() {
    use std::os::unix::fs::PermissionsExt;

    let env = flag_on_sandbox();
    env.but("worktree new caller").assert().success();
    env.invoke_bash(
        r#"
        printf 'cache/\n' > .gitignore
        mkdir -p cache/empty nested/.git
        printf 'ignored\n' > cache/build
        printf 'nested metadata\n' > nested/.git/sentinel
        printf 'staged\n' > A
        git add A
        printf 'unstaged\n' >> A
        git rm -q B
        rm M
        printf 'untracked\n' > untracked
        printf 'intent\n' > intent
        git add -N intent
        printf '#!/bin/sh\n' > executable
        chmod +x executable
        ln -s cache/build link
        ln -s missing dangling
        ln -s cache directory-link
        printf 'caller only\n' > .git/gb-wts/caller/caller-only
        git update-index --split-index
    "#,
    );
    let source = env.projects_root();
    let caller = source.join(".git/gb-wts/caller");
    let destination = source.join(".git/gb-wts/cloned");
    env.but("worktree new --cow cloned")
        .current_dir(&caller)
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree cloned on 'cloned' from [..] at [..]/.git/gb-wts/cloned

"#]]);
    for file in [
        "A",
        ".gitignore",
        "cache/build",
        "nested/.git/sentinel",
        "untracked",
        "intent",
        "executable",
    ] {
        assert_eq!(
            std::fs::read(source.join(file)).unwrap(),
            std::fs::read(destination.join(file)).unwrap(),
            "{file} was cloned regardless of Git tracking or ignore rules"
        );
    }
    assert!(
        destination.join("cache/empty").is_dir(),
        "empty directories survive"
    );
    for absent in ["B", "M", "caller-only"] {
        assert!(
            !destination.join(absent).exists(),
            "{absent} is absent in the primary worktree"
        );
    }
    assert!(
        destination.join(".git").is_file(),
        "Git's linked-worktree file wasn't overwritten"
    );
    for link in ["link", "dangling", "directory-link"] {
        assert_eq!(
            std::fs::read_link(source.join(link)).unwrap(),
            std::fs::read_link(destination.join(link)).unwrap(),
            "symlinks are cloned without following them"
        );
    }
    assert_ne!(
        std::fs::metadata(destination.join("executable"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0,
        "executable bits survive"
    );
    env.invoke_bash(r#"
        test "$(git rev-parse HEAD)" = "$(git -C .git/gb-wts/cloned rev-parse HEAD)"
        diff <(git ls-files --stage -v) <(git -C .git/gb-wts/cloned ls-files --stage -v)
        diff <(git status --porcelain=v1 --untracked-files=all) <(git -C .git/gb-wts/cloned status --porcelain=v1 --untracked-files=all)
        printf 'changed clone\n' > .git/gb-wts/cloned/cache/build
        test "$(cat cache/build)" = ignored
    "#);
}

#[cfg(unix)]
#[cfg_attr(not(target_os = "macos"), ignore = "requires macOS clonefile(2)")]
#[test]
fn cow_cleans_up_after_encountering_a_special_file() {
    let env = flag_on_sandbox();
    env.invoke_bash("mkfifo unsupported-pipe");
    env.but("worktree new --cow cloned")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Cannot clone special file '[..]/unsupported-pipe'

"#]]);
    let repo = env.context().repo.get().unwrap().clone();
    assert!(
        repo.try_find_reference("cloned").unwrap().is_none(),
        "failed clone's branch is removed"
    );
    assert!(
        !env.projects_root().join(".git/gb-wts/cloned").exists(),
        "partial checkout is removed"
    );
    assert!(
        repo.worktree_proxy_by_id(gix::bstr::BStr::new("cloned"))
            .is_none(),
        "worktree registration is removed"
    );
    // Cleanup makes the same name usable again.
    env.invoke_bash("rm unsupported-pipe");
    env.but("worktree new --cow cloned").assert().success();
}

/// A worktree is named by its checkout, never by a branch below it, which it merely holds.
#[test]
fn a_lower_branch_does_not_name_its_worktree() {
    let env = flag_on_sandbox();
    crate::command::util::add_worktree_with_lower_branch(&env, "wt-inside", "A");

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
