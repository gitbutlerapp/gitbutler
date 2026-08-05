use crate::{command::undo::undo_commit::commit_empty_with_message, utils::Sandbox};

mod undo_commit;
mod undo_move;
mod undo_redo;
mod undo_squash;
mod undo_uncommit;

/// Run an undo test tests a roundtrip `mutate` -> `but undo`, and asserts that the status output is
/// the same before and after the roundtrip.
#[track_caller]
fn run_mutate_undo_roundtrip_test<F>(env: &Sandbox, mutate: F)
where
    F: FnOnce(&Sandbox),
{
    // Arrange
    let status_output_before = env
        .but("status")
        .args(["--verbose", "--files"])
        .output()
        .unwrap();

    {
        eprintln!("Status before mutation:");
        let output = String::from_utf8(status_output_before.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    mutate(env);
    let status_output_after_mutate = env
        .but("status")
        .args(["--verbose", "--files"])
        .output()
        .unwrap();

    {
        eprintln!();
        eprintln!("Status after mutation:");
        let output = String::from_utf8(status_output_after_mutate.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    assert_ne!(
        status_output_before, status_output_after_mutate,
        "mutate must visibly change state"
    );

    // Act
    env.but("undo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Undid [..] (2000-01-02 00:00:00): [..]

"#]]);

    // Assert
    env.but("status")
        .args(["--verbose", "--files"])
        .assert()
        .success()
        .stdout_eq(status_output_before.stdout)
        .stderr_eq(status_output_before.stderr);
}

#[test]
fn can_undo_discard() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard")
            .arg(path)
            .assert()
            .success()
            .stdout_eq("Discarded uncommitted changes from new-file.txt\n")
            .stderr_eq("");
    });
}

#[test]
fn can_undo_but_discard_file_modifications() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("first", "This is new stuff");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard zz").assert().success();
    });
}

#[test]
fn can_undo_but_discard_new_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("totally_new_file", "This is new stuff");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard zz").assert().success();
    });
}

#[test]
fn can_undo_but_discard_deletion() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let filepath = env.projects_root().join("first");
    std::fs::remove_file(&filepath).expect("must be able to delete file");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard zz").assert().success();
    });
}

#[test]
fn can_undo_but_discard_rename() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let filepath = env.projects_root().join("first");
    let new_filepath = env.projects_root().join("first_renamed");
    std::fs::rename(&filepath, &new_filepath).expect("must be able to move file");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard zz").assert().success();
    });
}

#[test]
fn can_undo_unapply() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("unapply A")
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
Unapplied stack with 'A' from workspace

"#]])
            .stderr_eq("");
    });
}

#[test]
fn can_undo_clean_apply() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.but("unapply A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("apply A")
            .assert()
            .success()
            .stdout_eq("Applied branch 'A' to workspace\n")
            .stderr_eq("");
    });
}

// Restoring a snapshot must revert the worktree even when a later operation moved the workspace
// commit: commit a new file, restore an earlier snapshot, and the file must be gone again.
#[test]
fn can_restore_snapshot_after_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    let status_before = env
        .but("status")
        .args(["--verbose", "--files"])
        .output()
        .unwrap();

    // `but oplog snapshot` prints a `  Snapshot ID: <hex>` line we restore from.
    let snapshot = env
        .but("oplog snapshot -m baseline")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let snapshot_output = String::from_utf8_lossy(&snapshot);
    let snapshot_id = snapshot_output
        .split("Snapshot ID:")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("snapshot output contains a `Snapshot ID:` line");
    assert!(
        !snapshot_id.is_empty() && snapshot_id.chars().all(|c| c.is_ascii_hexdigit()),
        "parsed snapshot id should be a bare hex string, got {snapshot_id:?}"
    );

    env.file("brand-new-file.txt", "content");
    env.but("commit -b A -m 'add brand new file'")
        .assert()
        .success();

    // The commit must actually change workspace state, otherwise the round-trip below would match
    // `status_before` without the restore having reverted anything.
    let status_after_commit = env
        .but("status")
        .args(["--verbose", "--files"])
        .output()
        .unwrap();
    assert_ne!(status_before.stdout, status_after_commit.stdout);

    env.but(format!("oplog restore {snapshot_id}"))
        .assert()
        .success();

    env.but("status")
        .args(["--verbose", "--files"])
        .assert()
        .success()
        .stdout_eq(status_before.stdout)
        .stderr_eq(status_before.stderr);
}

#[test]
fn can_undo_rewording_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["9ac4652", "-m", "reworded"])
            .assert()
            .success()
            .stdout_eq("Updated commit message for [..]\n")
            .stderr_eq("");
    });
}

#[test]
fn can_undo_rewording_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["A", "-m", "reworded-branch"])
            .assert()
            .success()
            .stdout_eq("Renamed branch 'A' to 'reworded-branch'\n")
            .stderr_eq("");
    });
}

#[test]
fn can_undo_but_branch_new_at_base() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["new", "foo"]).assert().success();
    });
}

#[test]
fn can_undo_but_branch_in_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("branch").args(["new", "foo"]).assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch")
            .args(["new", "bar", "-a", "foo"])
            .assert()
            .success();
    });
}

#[test]
fn can_undo_but_branch_delete() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("branch").args(["new", "foo"]).assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["delete", "foo"]).assert().success();
    });
}

#[test]
fn can_undo_but_branch_delete_in_stack() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.but("branch").args(["new", "foo"]).assert().success();
    env.but("branch")
        .args(["new", "bar", "-a", "foo"])
        .assert()
        .success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["delete", "bar"]).assert().success();
    });
}

#[test]
fn can_undo_but_absorb() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    env.file("first", "This is new stuff");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("absorb").assert().success();
    });
}

#[test]
fn can_undo_but_squash_with_two_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let commit_one = commit_empty_with_message(&env, "one");
    let commit_two = commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!("squash {commit_one} -t {commit_two} -u"))
            .assert()
            .success();
    });

    // ensure we only create one oplog entry
    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b6d1f77 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Squashed commit (5c7ea30)
5c7ea30 2000-01-02 00:00:00 [SQUASH] Squashed commit
a135744 2000-01-02 00:00:00 [COMMIT] Created commit
29b25c0 2000-01-02 00:00:00 [COMMIT] Created commit

"#]]);
}

#[test]
fn can_undo_but_squash_with_three_commits() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let commit_one = commit_empty_with_message(&env, "one");
    let commit_two = commit_empty_with_message(&env, "two");
    let commit_three = commit_empty_with_message(&env, "three");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!(
            "squash {commit_one} {commit_two} -t {commit_three} -u"
        ))
        .assert()
        .success();
    });

    // ensure we only create one oplog entry
    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
62a6316 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Squashed commit (5c36646)
5c36646 2000-01-02 00:00:00 [SQUASH] Squashed commit
8648d78 2000-01-02 00:00:00 [COMMIT] Created commit
a135744 2000-01-02 00:00:00 [COMMIT] Created commit
29b25c0 2000-01-02 00:00:00 [COMMIT] Created commit

"#]]);
}

#[test]
fn can_undo_but_squash_with_two_commits_with_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    let commit_one = commit_empty_with_message(&env, "one");
    let commit_two = commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but(format!(
            "squash {commit_one} -t {commit_two} -m 'squashed message'"
        ))
        .assert()
        .success();
    });

    // ensure we only create one oplog entry
    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ff65358 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Squashed commit (5c7ea30)
5c7ea30 2000-01-02 00:00:00 [SQUASH] Squashed commit
a135744 2000-01-02 00:00:00 [COMMIT] Created commit
29b25c0 2000-01-02 00:00:00 [COMMIT] Created commit

"#]]);
}

#[test]
fn can_undo_but_squash_with_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    commit_empty_with_message(&env, "one");
    commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("squash A -u").assert().success();
    });

    // ensure we only create one oplog entry
    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
4c80f19 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Squashed commit (5c7ea30)
5c7ea30 2000-01-02 00:00:00 [SQUASH] Squashed commit
a135744 2000-01-02 00:00:00 [COMMIT] Created commit
29b25c0 2000-01-02 00:00:00 [COMMIT] Created commit

"#]]);
}

#[test]
fn can_undo_but_squash_with_branch_and_drop_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    commit_empty_with_message(&env, "one");
    commit_empty_with_message(&env, "two");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("squash A --no-message").assert().success();
    });

    // ensure we only create one oplog entry
    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f698bb9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Squashed commit (5c7ea30)
5c7ea30 2000-01-02 00:00:00 [SQUASH] Squashed commit
a135744 2000-01-02 00:00:00 [COMMIT] Created commit
29b25c0 2000-01-02 00:00:00 [COMMIT] Created commit

"#]]);
}
