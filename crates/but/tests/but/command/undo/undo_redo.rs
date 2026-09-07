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
  "snapshotId": "cc69fe723b88345a93eaa39c5abff92eeeaf1756"
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
  "snapshotId": "cc69fe723b88345a93eaa39c5abff92eeeaf1756"
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
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "37054a8",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cad44b6 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

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
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "c611d4d", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8f3151f 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (c611d4d)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d9d816b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
8f3151f 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (c611d4d)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

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
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
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
0bf5e2d 2000-01-02 00:00:00 [REWORD] Updated commit message
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "0bf5e2d",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d0bd584 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (0bf5e2d)
0bf5e2d 2000-01-02 00:00:00 [REWORD] Updated commit message
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
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
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "37054a8",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
62c94f8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "cc69fe7",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
83a9d3a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (cc69fe7)
62c94f8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

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
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3044865 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (ff56b1c)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

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
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8a6a1ea 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "37054a8",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bfe5186 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
8a6a1ea 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "37054a8",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1bd0e97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (37054a8)
bfe5186 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
8a6a1ea 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c611d4d",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
167843f 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
1bd0e97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (37054a8)
bfe5186 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
8a6a1ea 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "ff56b1c",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9959d4f 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (ff56b1c)
167843f 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
1bd0e97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (37054a8)
bfe5186 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (37054a8)
8a6a1ea 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
a9bbf72 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c611d4d)
d6d7810 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c611d4d)
99d82bb 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (ff56b1c)
ff56b1c 2000-01-02 00:00:00 [REWORD] Updated commit message
c611d4d 2000-01-02 00:00:00 [REWORD] Updated commit message
37054a8 2000-01-02 00:00:00 [REWORD] Updated commit message
cc69fe7 2000-01-02 00:00:00 [REWORD] Updated commit message

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
