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
        "Undid {snapshot_restored_to} (2000-01-02 00:00:00): {}\n",
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
        "Redid {snapshot_restored_to} (2000-01-02 00:00:00): {}\n",
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
fn undo_and_redo_have_structured_json_output() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits");
    env.setup_metadata(&["A"]);
    let (_status, _new_commit) = reword(&env, "9ac4652", "one");

    env.but("undo")
        .arg("--json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "action": "undo",
  "changed": true,
  "snapshotId": "36aeea0714b6d32ac09b71a0131fc57e5887624c"
}

"#]]);

    env.but("redo")
        .arg("--json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "action": "redo",
  "changed": true,
  "snapshotId": "36aeea0714b6d32ac09b71a0131fc57e5887624c"
}

"#]]);

    env.but("redo")
        .arg("--json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "action": "redo",
  "changed": false
}

"#]]);
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
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9513ba9",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0ae7680 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "70b1117", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
859f2d1 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (70b1117)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f404f6a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
859f2d1 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (70b1117)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
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
a0b8d32 2000-01-02 00:00:00 [REWORD] Updated commit message
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a0b8d32",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9b9c8c8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a0b8d32)
a0b8d32 2000-01-02 00:00:00 [REWORD] Updated commit message
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
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
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9513ba9",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9eec3f8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "36aeea0",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
999f443 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (36aeea0)
9eec3f8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
03a9173 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (cacda62)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b9346cc 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9513ba9",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
87d40ca 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
b9346cc 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9513ba9",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
4a23f33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9513ba9)
87d40ca 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
b9346cc 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "70b1117",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d77ea33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
4a23f33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9513ba9)
87d40ca 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
b9346cc 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cacda62",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1e07449 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (cacda62)
d77ea33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
4a23f33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9513ba9)
87d40ca 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9513ba9)
b9346cc 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
30c995e 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (70b1117)
ffbfefe 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (70b1117)
b6f62d4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cacda62)
cacda62 2000-01-02 00:00:00 [REWORD] Updated commit message
70b1117 2000-01-02 00:00:00 [REWORD] Updated commit message
9513ba9 2000-01-02 00:00:00 [REWORD] Updated commit message
36aeea0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
