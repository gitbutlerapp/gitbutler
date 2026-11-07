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
  "snapshotId": "a85bb94a2c7b27659dc269528e5e16ce45253248"
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
  "snapshotId": "a85bb94a2c7b27659dc269528e5e16ce45253248"
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
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "8085420",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3749d2f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

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
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "b63dfa2", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ecbf0e4 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (b63dfa2)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c8a683e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
ecbf0e4 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (b63dfa2)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

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
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
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
68344cd 2000-01-02 00:00:00 [REWORD] Updated commit message
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "68344cd",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7989ae3 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (68344cd)
68344cd 2000-01-02 00:00:00 [REWORD] Updated commit message
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
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
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "8085420",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
a046d26 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a85bb94",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d6bdf04 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a85bb94)
a046d26 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

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
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b00f800 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (51f0eae)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

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
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c0a3c2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "8085420",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8cd2b5c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
c0a3c2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "8085420",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7c5ba97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (8085420)
8cd2b5c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
c0a3c2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "b63dfa2",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
ec4282d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
7c5ba97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (8085420)
8cd2b5c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
c0a3c2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "51f0eae",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
366822c 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (51f0eae)
ec4282d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
7c5ba97 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (8085420)
8cd2b5c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (8085420)
c0a3c2d 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
35fe032 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (b63dfa2)
331910c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (b63dfa2)
00e27c2 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (51f0eae)
51f0eae 2000-01-02 00:00:00 [REWORD] Updated commit message
b63dfa2 2000-01-02 00:00:00 [REWORD] Updated commit message
8085420 2000-01-02 00:00:00 [REWORD] Updated commit message
a85bb94 2000-01-02 00:00:00 [REWORD] Updated commit message

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
