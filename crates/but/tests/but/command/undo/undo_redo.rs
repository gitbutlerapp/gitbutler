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
  "snapshotId": "42e9c3889ac975f8b433460c8cdbd9938b2c1bb4"
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
  "snapshotId": "42e9c3889ac975f8b433460c8cdbd9938b2c1bb4"
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
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "fc943cb",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3dca99a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

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
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "0403bda", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bd8aab7 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (0403bda)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f9af46c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
bd8aab7 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (0403bda)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

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
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
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
d0770e3 2000-01-02 00:00:00 [REWORD] Updated commit message
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "d0770e3",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
fe163e5 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (d0770e3)
d0770e3 2000-01-02 00:00:00 [REWORD] Updated commit message
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
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
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "fc943cb",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ad0fbf1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "42e9c38",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
752db9d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (42e9c38)
ad0fbf1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

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
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
2fe3b33 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (4022979)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

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
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
290b945 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "fc943cb",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
e2b2088 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
290b945 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "fc943cb",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
e86a47a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (fc943cb)
e2b2088 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
290b945 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0403bda",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4afe0d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
e86a47a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (fc943cb)
e2b2088 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
290b945 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4022979",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
612d67a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (4022979)
f4afe0d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
e86a47a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (fc943cb)
e2b2088 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (fc943cb)
290b945 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
8256304 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (0403bda)
bcade72 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0403bda)
158ca23 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4022979)
4022979 2000-01-02 00:00:00 [REWORD] Updated commit message
0403bda 2000-01-02 00:00:00 [REWORD] Updated commit message
fc943cb 2000-01-02 00:00:00 [REWORD] Updated commit message
42e9c38 2000-01-02 00:00:00 [REWORD] Updated commit message

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
