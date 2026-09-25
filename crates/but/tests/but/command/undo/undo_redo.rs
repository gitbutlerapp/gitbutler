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
  "snapshotId": "9cd3d1400286c67855ec319ef7287bc9b610b7b2"
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
  "snapshotId": "9cd3d1400286c67855ec319ef7287bc9b610b7b2"
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
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c977fa4",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
dee27b0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

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
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "43eca5f", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
aaf6a61 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (43eca5f)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3cb4dfb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
aaf6a61 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (43eca5f)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

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
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
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
cf63d2c 2000-01-02 00:00:00 [REWORD] Updated commit message
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cf63d2c",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7562539 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cf63d2c)
cf63d2c 2000-01-02 00:00:00 [REWORD] Updated commit message
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
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
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c977fa4",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c8c76e9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "9cd3d14",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5ca5eed 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (9cd3d14)
c8c76e9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

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
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
10719f9 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (1406059)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

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
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7f3b145 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c977fa4",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3672b3a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
7f3b145 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c977fa4",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ba08990 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c977fa4)
3672b3a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
7f3b145 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "43eca5f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f8de399 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
ba08990 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c977fa4)
3672b3a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
7f3b145 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "1406059",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
efb493a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (1406059)
f8de399 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
ba08990 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c977fa4)
3672b3a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c977fa4)
7f3b145 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
b544620 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (43eca5f)
bfc2ff0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (43eca5f)
cae83f0 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (1406059)
1406059 2000-01-02 00:00:00 [REWORD] Updated commit message
43eca5f 2000-01-02 00:00:00 [REWORD] Updated commit message
c977fa4 2000-01-02 00:00:00 [REWORD] Updated commit message
9cd3d14 2000-01-02 00:00:00 [REWORD] Updated commit message

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
