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
  "snapshotId": "0a8d5dd3d17a15fd88d4857d2421f41ff1a1c2a8"
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
  "snapshotId": "0a8d5dd3d17a15fd88d4857d2421f41ff1a1c2a8"
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
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9d45564",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b5154a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "dfe9b0e", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0516392 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (dfe9b0e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
338a88f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
0516392 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (dfe9b0e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
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
81deab5 2000-01-02 00:00:00 [REWORD] Updated commit message
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "81deab5",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9f8fdc0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (81deab5)
81deab5 2000-01-02 00:00:00 [REWORD] Updated commit message
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
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
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9d45564",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
df1649d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
c92555f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0a8d5dd)
df1649d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
96ca827 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f82096e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
84c7fb7 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9d45564",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
798926a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
84c7fb7 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9d45564",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bb7fa37 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9d45564)
798926a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
84c7fb7 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dfe9b0e",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1c4cfb6 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
bb7fa37 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9d45564)
798926a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
84c7fb7 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
0a8d5dd 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f82096e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
28ec153 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f82096e)
1c4cfb6 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
bb7fa37 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (9d45564)
798926a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9d45564)
84c7fb7 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
3cc6dfe 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dfe9b0e)
d3636ab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dfe9b0e)
f1a5105 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f82096e)
f82096e 2000-01-02 00:00:00 [REWORD] Updated commit message
dfe9b0e 2000-01-02 00:00:00 [REWORD] Updated commit message
9d45564 2000-01-02 00:00:00 [REWORD] Updated commit message
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
