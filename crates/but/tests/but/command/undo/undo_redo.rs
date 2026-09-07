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
  "snapshotId": "585d5188113bce05cab2f164587818351f8645c7"
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
  "snapshotId": "585d5188113bce05cab2f164587818351f8645c7"
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
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4e699e0",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
367cc16 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

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
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "3d9a7b2", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5c50eb3 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (3d9a7b2)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
fd4271e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
5c50eb3 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (3d9a7b2)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

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
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
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
a3b7f2b 2000-01-02 00:00:00 [REWORD] Updated commit message
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "a3b7f2b",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cb2790b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (a3b7f2b)
a3b7f2b 2000-01-02 00:00:00 [REWORD] Updated commit message
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
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
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4e699e0",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
63b1af4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "585d518",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
6d62c5b 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (585d518)
63b1af4 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

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
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cff3f01 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c18c62c)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

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
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
2955ea9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4e699e0",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b4423f9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
2955ea9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "4e699e0",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
7649130 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (4e699e0)
b4423f9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
2955ea9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3d9a7b2",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
e1c038d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
7649130 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (4e699e0)
b4423f9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
2955ea9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::UpdateCommitMessage,
        "c18c62c",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5e52b12 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (c18c62c)
e1c038d 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
7649130 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (4e699e0)
b4423f9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (4e699e0)
2955ea9 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
9b187f2 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (3d9a7b2)
cca3ac8 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3d9a7b2)
c447d01 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (c18c62c)
c18c62c 2000-01-02 00:00:00 [REWORD] Updated commit message
3d9a7b2 2000-01-02 00:00:00 [REWORD] Updated commit message
4e699e0 2000-01-02 00:00:00 [REWORD] Updated commit message
585d518 2000-01-02 00:00:00 [REWORD] Updated commit message

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
