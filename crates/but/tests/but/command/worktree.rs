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
Error: '[..]/home/.gitbutler-worktrees/[..]/feature-one' already exists

"#]]);
}

#[cfg(unix)]
#[test]
fn new_worktreeinclude_preserves_symlinks_and_skips_special_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);
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
    enable_worktree_manipulation(&env);
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
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);

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
    enable_worktree_manipulation(&env);
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

#[test]
fn new_worktreeinclude_copies_only_destination_ignored_files_untracked_in_both_indexes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    enable_worktree_manipulation(&env);

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
fn new_above_a_commit_cli_id() {
    let env = flag_on_sandbox();
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
    let env = flag_on_sandbox();
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
fn new_above_rejects_a_branch() {
    let env = flag_on_sandbox();
    env.but("worktree new feature --above A")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find commit: 'A'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn new_above_rejects_an_unknown_commit() {
    let env = flag_on_sandbox();
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
    enable_worktree_manipulation(&env);
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
    let env = flag_on_sandbox();
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
