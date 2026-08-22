use but_testsupport::invoke_bash_at_dir;

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
mi mismatch (refs/heads/qux) - [..]/worktrees/mismatch
wt wt-at (detached) - [..]/worktrees/wt-at
at wt-feature - [..]/worktrees/wt-feature

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
mi mismatch (refs/heads/qux) - [..]/worktrees/mismatch
wt wt-at (detached) - [..]/worktrees/wt-at
at wt-feature - [..]/worktrees/wt-feature

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
mi mismatch (refs/heads/qux) - [..]/worktrees/mismatch
wt wt-at (detached) - [..]/worktrees/wt-at
at wt-feature - [..]/worktrees/wt-feature

"#]]);
    env.but("worktree list --active --archived")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
mi mismatch (refs/heads/qux) - [..]/worktrees/mismatch
wt wt-at (detached) - [..]/worktrees/wt-at
at wt-feature - [..]/worktrees/wt-feature

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
      "id": "mi",
      "name": "mismatch",
      "refName": "refs/heads/qux",
      "path": "[..]/worktrees/mismatch",
      "updatedAtMs": 947376000000
    },
    {
      "id": "wt",
      "name": "wt-at",
      "refName": null,
      "path": "[..]/worktrees/wt-at",
      "updatedAtMs": 947289600000
    },
    {
      "id": "at",
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

    env.but("worktree list --active")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Active worktrees
wt wt-feature - [..]/worktrees/wt-feature

"#]]);

    env.but("worktree archive wt")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Successfully archived wt-feature

"#]]);
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
    env.but("worktree unarchive wt-feature")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Successfully unarchived wt-feature

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

    env.but("worktree archive nope")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
Error: Could not find worktree: 'nope'

Hint: Run `but worktree list` for the worktrees and their IDs.

"#]]);

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
}
