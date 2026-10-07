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

#[test]
#[cfg(feature = "worktree-cow")]
fn new_create_modes() {
    for (flags, copies_artifacts) in [
        ("--create-mode cow", true),
        ("--create-mode cow-ignored", true),
        ("--create-mode cow-target", false),
        ("--create-mode checkout", false),
        ("", false), // defaults to checkout
    ] {
        let env = flag_on_sandbox();
        env.invoke_bash("echo /ignored >> .git/info/exclude; echo artifact > ignored");
        env.but(format!("worktree new {flags} wt-mode"))
            .assert()
            .success()
            .stderr_eq(snapbox::str![])
            .stdout_eq(snapbox::str![[r#"
Created worktree wt-mode on 'wt-mode' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-mode

"#]]);
        let destination = env
            .home_dir()
            .join(".gitbutler-worktrees")
            .join(env.projects_root().file_name().unwrap())
            .join("wt-mode");
        assert_eq!(
            destination.join("ignored").exists(),
            copies_artifacts,
            "{flags:?} selects whether ignored artifacts are copied"
        );
    }
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_selective_cow_checks_out_before_copying_artifacts() {
    for mode in ["cow-ignored", "cow-target"] {
        let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-in-sync");
        env.invoke_git("checkout main");
        env.file(".gitignore", "/target/\n/generated/cache\n/extra\n");
        env.file("target/tracked", "committed\n");
        env.file("target/directory/tracked", "committed\n");
        env.file("generated/.gitignore", "*.log\n!keep.log\n");
        env.invoke_bash("ln -s ../outside target/link; git add .gitignore generated/.gitignore; git add -f target; git commit -qm 'tracked artifacts'");
        let base = env.invoke_git("rev-parse HEAD");
        enable_worktree_manipulation(&env);
        env.file("target/tracked", "dirty\n");
        env.file("target/debug/deps/artifact", "artifact\n");
        env.file("generated/cache", "cache\n");
        env.file("generated/untracked", "do not copy\n");
        env.file("generated/build.log", "copy\n");
        env.file("generated/keep.log", "do not copy\n");
        env.file("generated/.gitignore", "*\n");
        env.file("extra", "extra\n");
        // Destination rules, not dirty source rules, determine ignored files.
        env.file(".gitignore", "/wrong\n");
        env.file("wrong", "do not copy\n");
        env.invoke_bash("rm -r target/directory; ln -s /does-not-exist target/directory; ln -s missing target/dangling; rm target/link; mkdir target/link; echo content > target/link/file; mkfifo target/pipe");
        env.but(format!(
            "worktree new --create-mode {mode} -A {} wt-selective",
            base.trim()
        ))
        .assert()
        .success();
        let destination = env
            .home_dir()
            .join(".gitbutler-worktrees")
            .join(env.projects_root().file_name().unwrap())
            .join("wt-selective");
        snapbox::assert_data_eq!(
            std::fs::read_to_string(destination.join("target/tracked")).unwrap(),
            "committed\n"
        );
        snapbox::assert_data_eq!(
            std::fs::read_to_string(destination.join("target/directory/tracked")).unwrap(),
            "committed\n"
        );
        snapbox::assert_data_eq!(
            std::fs::read_to_string(destination.join("target/debug/deps/artifact")).unwrap(),
            "artifact\n"
        );
        assert_eq!(
            destination.join("extra").exists(),
            mode == "cow-ignored",
            "only ignored mode copies artifacts outside target"
        );
        assert_eq!(
            destination.join("generated/cache").exists(),
            mode == "cow-ignored",
            "ignored file survives beside ordinary untracked file"
        );
        assert_eq!(
            destination.join("generated/build.log").exists(),
            mode == "cow-ignored",
            "nested destination ignore rules are used"
        );
        assert!(
            !destination.join("generated/keep.log").exists(),
            "negated ignore rules are honored"
        );
        assert_eq!(
            std::fs::read_link(destination.join("target/link")).unwrap(),
            std::path::Path::new("../outside"),
            "checked-out symlinks are not replaced or traversed"
        );
        assert!(
            !destination.join("outside").exists(),
            "cloning must not write through destination symlinks"
        );
        assert!(
            !destination.join("generated/untracked").exists(),
            "ordinary untracked files are not copied"
        );
        assert!(
            !destination.join("wrong").exists(),
            "dirty source ignore rules are not used"
        );
        assert!(
            !destination.join("target/pipe").exists(),
            "special files are skipped"
        );
        assert_eq!(
            std::fs::read_link(destination.join("target/dangling")).unwrap(),
            std::path::Path::new("missing"),
            "dangling links are cloned, not followed"
        );
        env.invoke_bash(format!(
            r#"test -z "$(git -C '{}' status --porcelain --untracked-files=all)""#,
            destination.display()
        ));
    }
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_selective_cow_target_does_not_require_ignore_rules() {
    for mode in ["cow-ignored", "cow-target"] {
        let env = flag_on_sandbox();
        env.file("target/cache", "artifact\n");
        env.file("nested/target/cache", "not root target\n");
        env.but(format!("worktree new --create-mode {mode} wt-target"))
            .assert()
            .success();
        let destination = env
            .home_dir()
            .join(".gitbutler-worktrees")
            .join(env.projects_root().file_name().unwrap())
            .join("wt-target");
        assert_eq!(
            destination.join("target/cache").exists(),
            mode == "cow-target",
            "target mode copies artifacts even without ignore rules"
        );
        assert!(
            !destination.join("nested/target/cache").exists(),
            "target mode only copies root target directory"
        );
    }
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_selective_cow_target_skips_symlinked_target() {
    let env = flag_on_sandbox();
    env.invoke_bash("mkdir external; echo content > external/cache; ln -s external target");
    env.but("worktree new --create-mode cow-target wt-target")
        .assert()
        .success();
    let destination = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-target");
    assert!(
        std::fs::symlink_metadata(destination.join("target")).is_err(),
        "root target symlink is neither followed nor copied"
    );
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
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_clones_all_main_worktree_files() {
    let env = flag_on_sandbox();
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[
            r#"Created worktree wt-cow on 'wt-cow' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#
        ]]);

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
┊╭┄ wt:@ [uncommitted] {wt-cow} (no changes)
┊├┄ wt [wt-cow] (no commits)
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_with_uncommitted_gitignore_leaves_linked_worktree_clean() {
    let env = flag_on_sandbox();
    env.file(".gitignore", "/ignored\n/nested/.gitignore\n");
    env.file("nested/.gitignore", "/ignored\n");
    env.file("ignored", "content\n");
    env.file("nested/ignored", "nested-content\n");

    // The root rule hides the nested ignore file, but its rules still hide nested files.
    snapbox::assert_data_eq!(
        env.invoke_git("check-ignore -v ignored nested/.gitignore nested/ignored"),
        snapbox::str![[r#"
.gitignore:1:/ignored	ignored
.gitignore:2:/nested/.gitignore	nested/.gitignore
nested/.gitignore:1:/ignored	nested/ignored
"#]]
    );
    snapbox::assert_data_eq!(
        env.invoke_git("status --porcelain --untracked-files=all"),
        snapbox::str!["?? .gitignore"]
    );

    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    env.but("status").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   pu A .gitignore
┊
┊╭┄ g0 [A]
┊●   tpm add A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
├╯
┊
┊╭┄ wt:@ [uncommitted] {wt-cow} (no changes)
┊├┄ wt [wt-cow] (no commits)
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
}

/// Due to a bug in libgit2, a directory that contains a mix of ignored and untracked files is
/// completely wiped when removing untracked files. The expected result would be for the untracked
/// files to be removed while the ignored files should stick around.
///
/// We've determined this isn't a big deal and currently label it a WONTFIX, but this test case is
/// left around to document the behavior.
#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_fails_to_preserve_ignored_file_in_directory_with_untracked_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-in-sync");
    env.invoke_git("checkout main");
    env.file(".gitignore", "/generated/cache.bin\n");
    env.invoke_bash(
        r#"
        git add .gitignore
        git commit -m "Ignore build artifact"
        git update-ref refs/remotes/origin/main HEAD
        "#,
    );
    env.file("generated/cache.bin", "cached artifact\n");
    env.file("generated/scratch.txt", "untracked scratch\n");
    env.setup_metadata_at_target(&[], "origin/main");
    enable_worktree_manipulation(&env);
    env.but("worktree list").assert().success();

    // The directory has no tracked descendants, but only one of its files is ignored.
    snapbox::assert_data_eq!(
        env.invoke_git("status --porcelain --untracked-files=all"),
        snapbox::str![[r#"
?? generated/scratch.txt
"#]]
    );
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from [..] at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let worktree = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow");

    assert!(
        !worktree.join("generated/scratch.txt").exists(),
        "COW cleanup must remove the non-ignored untracked file"
    );
    assert!(
        !worktree.join("generated/cache.bin").exists(),
        "the ignored artifact has also been removed, even though it really shouldn't have been"
    );

    // The resulting worktree must still be clean of changes!
    snapbox::assert_data_eq!(
        env.invoke_git(&format!(
            "-C '{}' status --porcelain --untracked-files=all",
            worktree.display()
        )),
        snapbox::str![]
    );
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_with_removed_ignore_rule_preserves_target_ignored_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-in-sync");
    env.invoke_git("checkout main");
    env.file(".gitignore", "/ignored\n");
    env.invoke_bash(
        r#"
        git add .gitignore
        git commit -m "Ignore build artifact"
        git update-ref refs/remotes/origin/main HEAD
        "#,
    );
    env.file(".gitignore", "");
    env.file("ignored", "content\n");
    env.setup_metadata_at_target(&[], "origin/main");
    enable_worktree_manipulation(&env);
    env.but("worktree list").assert().success();

    // Removing the committed rule exposes the artifact in the source worktree.
    snapbox::assert_data_eq!(
        env.invoke_git("status --porcelain --untracked-files=all"),
        snapbox::str![[r#"
M .gitignore
?? ignored
"#]]
    );
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from [..] at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let worktree = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow");

    assert_eq!(
        std::fs::read_to_string(worktree.join(".gitignore")).unwrap(),
        "/ignored\n",
        "the linked worktree restores the target's ignore rule"
    );
    // Clean status alone is insufficient: deleting the ignored artifact also looks clean.
    snapbox::assert_data_eq!(
        env.invoke_git(&format!(
            "-C '{}' status --porcelain --untracked-files=all",
            worktree.display()
        )),
        snapbox::str![]
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("ignored"))
            .expect("COW cleanup must preserve files ignored by the target state"),
        "content\n",
        "the copied artifact retains its contents"
    );
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_restores_renamed_gitignore_before_artifact_cleanup() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-in-sync");
    env.invoke_git("checkout main");
    env.file(".gitignore", "/ignored\n");
    env.invoke_bash(
        r#"
        git add .gitignore
        git commit -m "Ignore build artifact"
        git update-ref refs/remotes/origin/main HEAD
        "#,
    );
    std::fs::rename(
        env.projects_root().join(".gitignore"),
        env.projects_root().join(".gitignore.bak"),
    )
    .unwrap();
    env.file("ignored", "cached artifact\n");
    env.setup_metadata_at_target(&[], "origin/main");
    enable_worktree_manipulation(&env);
    env.but("worktree list").assert().success();

    // The renamed file no longer supplies ignore rules in the source worktree.
    snapbox::assert_data_eq!(
        env.invoke_git("status --porcelain --untracked-files=all"),
        snapbox::str![[r#"
D .gitignore
?? .gitignore.bak
?? ignored
"#]]
    );
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from [..] at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let worktree = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow");

    assert_eq!(
        std::fs::read_to_string(worktree.join(".gitignore")).unwrap(),
        "/ignored\n",
        "the renamed ignore file is restored to its target path and contents"
    );
    assert!(
        !worktree.join(".gitignore.bak").exists(),
        "discarding the rename removes its destination"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("ignored"))
            .expect("restoring the renamed .gitignore must protect the copied artifact"),
        "cached artifact\n",
        "the file ignored by the target state survives cleanup"
    );
    // Artifact preservation must still leave a clean linked worktree.
    snapbox::assert_data_eq!(
        env.invoke_git(&format!(
            "-C '{}' status --porcelain --untracked-files=all",
            worktree.display()
        )),
        snapbox::str![]
    );
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_restores_nested_tracked_gitignores_before_artifact_cleanup() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("single-branch-in-sync");
    env.invoke_git("checkout main");
    env.file(".gitignore", "# Root ignore rules\n");
    env.file("nested/.gitignore", "/artifact\n");
    env.invoke_bash(
        r#"
        git add .gitignore nested/.gitignore
        git commit -m "Ignore nested artifact"
        git update-ref refs/remotes/origin/main HEAD
        "#,
    );
    env.file(".gitignore", "/nested/\n");
    env.file("nested/.gitignore", "");
    env.file("nested/artifact", "cached artifact\n");
    env.setup_metadata_at_target(&[], "origin/main");
    enable_worktree_manipulation(&env);
    env.but("worktree list").assert().success();

    // Ignoring the directory does not hide changes to its tracked .gitignore.
    snapbox::assert_data_eq!(
        env.invoke_git("status --porcelain --untracked-files=all"),
        snapbox::str![[r#"
M .gitignore
 M nested/.gitignore
"#]]
    );
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from [..] at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let worktree = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow");

    assert_eq!(
        std::fs::read_to_string(worktree.join(".gitignore")).unwrap(),
        "# Root ignore rules\n",
        "the root ignore file is restored to the target state"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("nested/.gitignore")).unwrap(),
        "/artifact\n",
        "the tracked nested ignore file is restored despite the source's ignored directory"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("nested/artifact"))
            .expect("all tracked ignore rules must be restored before artifact cleanup"),
        "cached artifact\n",
        "the artifact ignored by the target's nested rule is preserved"
    );
    // Preserving the artifact must still leave a clean linked worktree.
    snapbox::assert_data_eq!(
        env.invoke_git(&format!(
            "-C '{}' status --porcelain --untracked-files=all",
            worktree.display()
        )),
        snapbox::str![]
    );
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_file_changes_leave_main_worktree_unchanged() {
    let env = flag_on_sandbox();
    // We use an ignored file for the test as that is guaranteed not to be touched by checkout
    // cleanup
    env.invoke_bash("echo /ignored >> .git/info/exclude; echo original > ignored");

    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let cloned_file = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow/ignored");
    assert_eq!(
        std::fs::read_to_string(&cloned_file).unwrap(),
        "original\n",
        "the new worktree starts with the original file contents"
    );

    // Directly write to the file to ensure we keep the same inode
    std::fs::write(&cloned_file, "modified\n").unwrap();
    assert_eq!(
        std::fs::read_to_string(&cloned_file).unwrap(),
        "modified\n",
        "the cloned file contains the change"
    );
    assert_eq!(
        std::fs::read_to_string(env.projects_root().join("ignored")).unwrap(),
        "original\n",
        "writing to the COW clone must leave the main worktree file unchanged"
    );
}

#[cfg(unix)]
#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_copies_symlinks_without_following_them() {
    let env = flag_on_sandbox();
    // Keep these files out of the cleanup of untracked changes after cloning.
    env.invoke_bash(
        r#"
        echo /ignored/ >> .git/info/exclude
        mkdir -p ignored/directory
        echo content > ignored/directory/file
        ln -s directory/file ignored/file-link
        ln -s directory ignored/directory-link
        ln -s missing ignored/dangling-link
        "#,
    );

    // File, directory, and dangling links must all survive as links.
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    let destination = env
        .home_dir()
        .join(".gitbutler-worktrees")
        .join(env.projects_root().file_name().unwrap())
        .join("wt-cow/ignored");
    for (name, target) in [
        ("file-link", "directory/file"),
        ("directory-link", "directory"),
        ("dangling-link", "missing"),
    ] {
        assert_eq!(
            std::fs::read_link(destination.join(name)).unwrap(),
            std::path::Path::new(target),
            "{name} must remain a symlink with its original relative target"
        );
    }
}

/// Create an unreadable file to test mid-clone failure cleanup.
///
/// NOTE: If you run tests as root, this will fail as root can read files regardless of permissions.
/// You should not be running tests as root so I think that's fine.
#[cfg(unix)]
#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_removes_worktree_after_copy_failure() {
    let env = flag_on_sandbox();
    env.invoke_bash("echo content > unreadable; chmod 000 unreadable");

    // Creation must reach the copy step before failing, rather than refuse up front.
    env.but("worktree new --create-mode cow wt-cow")
        .env("LC_ALL", "C")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Failed to clone files into worktree

Caused by:
    0: Failed to clone file from '[..]/unreadable' to '[..]/home/.gitbutler-worktrees/[..]/wt-cow/unreadable'
...
    [..]Permission denied (os error 13)

"#]]);
    assert!(
        !env.home_dir()
            .join(".gitbutler-worktrees")
            .join(env.projects_root().file_name().unwrap())
            .join("wt-cow")
            .exists(),
        "a failed COW copy must remove the partially created worktree directory"
    );
    assert!(
        !env.projects_root().join(".git/worktrees/wt-cow").exists(),
        "a failed COW copy must remove the Git worktree registration"
    );
}

#[cfg(unix)]
#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_skips_special_files() {
    use std::os::unix::fs::FileTypeExt;

    let env = flag_on_sandbox();
    env.invoke_bash("mkfifo pipe");

    // A FIFO must be skipped rather than opened for copying, which would block.
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Created worktree wt-cow on 'wt-cow' from 0dc3733 at [..]/home/.gitbutler-worktrees/[..]/wt-cow

"#]]);
    assert!(
        std::fs::symlink_metadata(env.projects_root().join("pipe"))
            .unwrap()
            .file_type()
            .is_fifo(),
        "COW creation leaves the source FIFO intact"
    );
    assert_eq!(
        std::fs::symlink_metadata(
            env.home_dir()
                .join(".gitbutler-worktrees")
                .join(env.projects_root().file_name().unwrap())
                .join("wt-cow/pipe")
        )
        .unwrap_err()
        .kind(),
        std::io::ErrorKind::NotFound,
        "the special file must not be copied into the new worktree"
    );
}

#[test]
#[cfg(all(
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
fn new_cow_refuses_if_submodule_exists() {
    let env = flag_on_sandbox();
    env.invoke_bash(r#"git -c protocol.file.allow=always submodule add "$PWD" submodule"#);

    // COW creation refuses repositories with submodules before creating a worktree.
    env.but("worktree new --create-mode cow wt-cow")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: COW mode is not supported for submodules

"#]]);
    assert!(
        !env.home_dir()
            .join(".gitbutler-worktrees")
            .join(env.projects_root().file_name().unwrap())
            .join("wt-cow")
            .exists(),
        "refusing COW creation must not leave a worktree directory"
    );
    assert!(
        env.context()
            .repo
            .get()
            .unwrap()
            .find_reference("wt-cow")
            .is_err(),
        "refusing COW creation must not create the worktree branch"
    );
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
