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
  "snapshotId": "8e8150a2c5bc6c36f8cd9cffd16d4e45efdb5462"
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
  "snapshotId": "8e8150a2c5bc6c36f8cd9cffd16d4e45efdb5462"
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
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a15b0ee",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
307b9a0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

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
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "2c321bc", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
973e5dc 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (2c321bc)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
75e0dda 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
973e5dc 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (2c321bc)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

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
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
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
693e451 2000-01-02 00:00:00 [REWORD] Updated commit message
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "693e451",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b1308c6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (693e451)
693e451 2000-01-02 00:00:00 [REWORD] Updated commit message
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
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
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a15b0ee",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f8c903b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "8e8150a",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
74e0778 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8e8150a)
f8c903b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

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
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
630d0c4 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (aa23c29)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

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
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
700eb92 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a15b0ee",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c41a434 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
700eb92 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a15b0ee",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7c4fd21 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (a15b0ee)
c41a434 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
700eb92 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "2c321bc",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c59f09b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
7c4fd21 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (a15b0ee)
c41a434 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
700eb92 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "aa23c29",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b8a90bd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (aa23c29)
c59f09b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
7c4fd21 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (a15b0ee)
c41a434 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a15b0ee)
700eb92 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
e7bd563 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (2c321bc)
b47ee55 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (2c321bc)
956cbf6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (aa23c29)
aa23c29 2000-01-02 00:00:00 [REWORD] Updated commit message
2c321bc 2000-01-02 00:00:00 [REWORD] Updated commit message
a15b0ee 2000-01-02 00:00:00 [REWORD] Updated commit message
8e8150a 2000-01-02 00:00:00 [REWORD] Updated commit message

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
