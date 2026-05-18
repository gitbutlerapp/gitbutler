use gitbutler_oplog::entry::OperationKind;

use crate::utils::Sandbox;

#[track_caller]
fn reword(env: &Sandbox, commit_before: &str, new_message: &str) -> (std::process::Output, String) {
    #[derive(serde::Deserialize)]
    struct RewordOutput {
        new_commit_id: String,
    }

    let reword_output = env
        .but("reword")
        .args([commit_before, "-m", new_message, "--json"])
        .assert()
        .success();

    let reword_output =
        serde_json::from_slice::<RewordOutput>(&reword_output.get_output().stdout).unwrap();

    (
        env.but("status").output().unwrap(),
        reword_output.new_commit_id,
    )
}

#[track_caller]
fn undo(
    env: &Sandbox,
    operation_reverted_to: OperationKind,
    snapshot_restored_to: &str,
    expected_status: &std::process::Output,
) {
    env.but("undo").assert().success().stdout_eq(format!(
        r#"Undoing operation...
  Reverting to: {} (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: {snapshot_restored_to}
"#,
        operation_reverted_to.title()
    ));

    env.but("status")
        .assert()
        .success()
        .stdout_eq(expected_status.stdout.clone())
        .stderr_eq(expected_status.stderr.clone());
}

#[track_caller]
fn redo(
    env: &Sandbox,
    operation_reverted_to: OperationKind,
    snapshot_restored_to: &str,
    expected_status: &std::process::Output,
) {
    env.but("redo").assert().success().stdout_eq(format!(
        r#"Redoing operation...
  Reverting to: {} (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: {snapshot_restored_to}
"#,
        operation_reverted_to.title()
    ));

    env.but("status")
        .assert()
        .success()
        .stdout_eq(expected_status.stdout.clone())
        .stderr_eq(expected_status.stderr.clone());
}

#[track_caller]
fn restore(env: &Sandbox, operation_to_restore_to: &str, expected_status: &std::process::Output) {
    env.but("oplog")
        .args(["restore", operation_to_restore_to])
        .assert()
        .success()
        .stdout_eq(
            r#"
✓ Restore completed successfully!

Workspace has been restored to the selected snapshot.
"#,
        );
    env.but("status")
        .assert()
        .success()
        .stdout_eq(expected_status.stdout.clone())
        .stderr_eq(expected_status.stderr.clone());
}

#[test]
fn can_undo_repeatedly() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let (status_one, new_commit) = reword(&env, "9ac4652", "one");
    let (status_two, new_commit) = reword(&env, &new_commit, "two");
    let (status_three, new_commit) = reword(&env, &new_commit, "three");
    reword(&env, &new_commit, "four");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
800274e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn can_undo_explicit_restore() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let (_, new_commit) = reword(&env, "9ac4652", "one");
    let (status_two, new_commit) = reword(&env, &new_commit, "two");
    let (_, new_commit) = reword(&env, &new_commit, "three");
    let (status_four, _) = reword(&env, &new_commit, "four");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "e637109", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
10e3b45 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e637109)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshot,
        "10e3b45",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
a875d2c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
10e3b45 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e637109)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn can_undo_perform_operation_then_undo_again() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let (_, new_commit) = reword(&env, "9ac4652", "one");
    let (status_two, new_commit) = reword(&env, &new_commit, "two");
    let (status_three, new_commit) = reword(&env, &new_commit, "three");
    reword(&env, &new_commit, "four");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    reword(&env, &new_commit, "three-new");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3b50353 2000-01-02 00:00:00 [REWORD] Updated commit message
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3b50353",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c00e67a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3b50353)
3b50353 2000-01-02 00:00:00 [REWORD] Updated commit message
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );
}

#[test]
fn undoing_past_end_of_oplog() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let status_zero = env.but("status").output().unwrap();
    let (status_one, new_commit) = reword(&env, "9ac4652", "one");
    reword(&env, &new_commit, "two");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1c40c14 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7665ea7",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
092cc32 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7665ea7)
1c40c14 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("undo").assert().success().stdout_eq(
        r#"No previous operations to undo.
"#,
    );
}

#[test]
fn can_redo() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let (_, new_commit) = reword(&env, "9ac4652", "one");
    let (_, new_commit) = reword(&env, &new_commit, "two");
    let (status_three, new_commit) = reword(&env, &new_commit, "three");
    let (status_four, _) = reword(&env, &new_commit, "four");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "0a0795e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b57a0e1 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f4e985f)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );
}

#[test]
fn can_mix_undo_and_redo() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let (status_one, new_commit) = reword(&env, "9ac4652", "one");
    let (status_two, new_commit) = reword(&env, &new_commit, "two");
    let (status_three, new_commit) = reword(&env, &new_commit, "three");
    let (status_four, _) = reword(&env, &new_commit, "four");

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "8b8b27a",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshotViaRedo,
        "d07be52",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "5ffc4e1",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "4769e9f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
62ccc54 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "0a0795e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9306108 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f4e985f)
62ccc54 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn cannot_redo_without_undoing_first() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    reword(&env, "9ac4652", "one");

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );
}
