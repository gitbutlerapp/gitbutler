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
        .args([commit_before, "-m", new_message, "--format", "json"])
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
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "36b830e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e60a85c",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2e000dd",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3d96603 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn can_undo_explicit_restore() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "e60a85c", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
eb2a7af 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e60a85c)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshot,
        "eb2a7af",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0ffd9de 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
eb2a7af 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e60a85c)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn can_undo_perform_operation_then_undo_again() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "36b830e",
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
be72e62 2000-01-02 00:00:00 [REWORD] Updated commit message
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "be72e62",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b7c157d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (be72e62)
be72e62 2000-01-02 00:00:00 [REWORD] Updated commit message
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e60a85c",
        &status_two,
    );
}

#[test]
fn undoing_past_end_of_oplog() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2e000dd",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
2c2fcde 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0a8d5dd",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
68ac121 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0a8d5dd)
2c2fcde 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("undo").assert().success().stdout_eq(
        r#"No previous operations to undo.
"#,
    );
}

#[test]
fn can_redo() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "36b830e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "73abdf5",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1162b51 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (36b830e)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );
}

#[test]
fn can_mix_undo_and_redo() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

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
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "36b830e",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e60a85c",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "b366502",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshotViaRedo,
        "b38fad0",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d313d2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2e000dd",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
75de26e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
d313d2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "75de26e",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c84f8e5 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2e000dd)
75de26e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
d313d2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "d313d2d",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0b144fe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
c84f8e5 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2e000dd)
75de26e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
d313d2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "73abdf5",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b3ec65b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (36b830e)
0b144fe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
c84f8e5 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2e000dd)
75de26e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2e000dd)
d313d2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
b38fad0 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e60a85c)
b366502 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e60a85c)
73abdf5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36b830e)
36b830e 2000-01-02 00:00:00 [REWORD] Updated commit message
e60a85c 2000-01-02 00:00:00 [REWORD] Updated commit message
2e000dd 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);
}

#[test]
fn cannot_redo_without_undoing_first() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);

    reword(&env, "9ac4652", "one");

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );
}
