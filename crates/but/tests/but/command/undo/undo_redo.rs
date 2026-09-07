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
  "snapshotId": "75a89f04f3c2b4d35f96ad0f5fce1da127692172"
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
  "snapshotId": "75a89f04f3c2b4d35f96ad0f5fce1da127692172"
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
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dad56a6",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
fb1d62a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "7439f6f", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f7e9078 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (7439f6f)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b84e43f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
f7e9078 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (7439f6f)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
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
a45fa19 2000-01-02 00:00:00 [REWORD] Updated commit message
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a45fa19",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
bc3ce81 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a45fa19)
a45fa19 2000-01-02 00:00:00 [REWORD] Updated commit message
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
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
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dad56a6",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
642732b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "75a89f0",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8183c46 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (75a89f0)
642732b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
e0ba22a 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (5bd63b0)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
fed3255 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dad56a6",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
2da992b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
fed3255 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "dad56a6",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
640969b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dad56a6)
2da992b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
fed3255 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7439f6f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cdfd8cf 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
640969b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dad56a6)
2da992b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
fed3255 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "5bd63b0",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f5ec243 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (5bd63b0)
cdfd8cf 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
640969b 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (dad56a6)
2da992b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (dad56a6)
fed3255 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
5a8f2fd 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (7439f6f)
57d4b94 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7439f6f)
02c4fab 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (5bd63b0)
5bd63b0 2000-01-02 00:00:00 [REWORD] Updated commit message
7439f6f 2000-01-02 00:00:00 [REWORD] Updated commit message
dad56a6 2000-01-02 00:00:00 [REWORD] Updated commit message
75a89f0 2000-01-02 00:00:00 [REWORD] Updated commit message

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
