use gitbutler_oplog::entry::OperationKind;

use crate::utils::Sandbox;

mod undo_rub;

/// Run an undo test tests a roundtrip `mutate` -> `but undo`, and asserts that the status output is
/// the same before and after the roundtrip.
fn run_mutate_undo_roundtrip_test<F>(env: &Sandbox, mutate: F) -> anyhow::Result<()>
where
    F: FnOnce(&Sandbox) -> anyhow::Result<()>,
{
    // Arrange
    let status_output_before = env.but("status").args(["--verbose", "--files"]).output()?;

    {
        eprintln!("Status before mutation:");
        let output = String::from_utf8(status_output_before.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    mutate(env)?;
    let status_output_after_mutate = env.but("status").args(["--verbose", "--files"]).output()?;

    {
        eprintln!();
        eprintln!("Status after mutation:");
        let output = String::from_utf8(status_output_after_mutate.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    assert_ne!(
        status_output_before, status_output_after_mutate,
        "mutate must visibly change state"
    );

    // Act
    env.but("undo").assert().success().stdout_eq(
        r#"Undoing operation...
  Reverting to: [..]
✓ Undo completed successfully! Restored to snapshot:[..]
"#,
    );

    // Assert
    env.but("status")
        .args(["--verbose", "--files"])
        .assert()
        .success()
        .stdout_eq(status_output_before.stdout)
        .stderr_eq(status_output_before.stderr);

    Ok(())
}

#[track_caller]
fn reword(
    env: &Sandbox,
    commit_before: &str,
    new_message: &str,
) -> anyhow::Result<(std::process::Output, String)> {
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

    Ok((env.but("status").output()?, reword_output.new_commit_id))
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
fn can_undo_discard() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("discard")
            .arg(path)
            .assert()
            .success()
            .stdout_eq("Successfully discarded changes to 1 item\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;
    let path = "new-file.txt";
    env.file(path, "content");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.file("new-file.txt", "content");

        env.but("commit -m 'Add file'")
            .assert()
            .success()
            .stdout_eq("✓ Created commit [..] on branch A\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not restore the worktree file A for some reason"]
fn can_undo_unapply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("unapply A")
            .assert()
            .success()
            .stdout_eq("Unapplied stack with branches 'A' from workspace\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not remove the worktree file A for some reason"]
fn can_undo_clean_apply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.but("unapply A").assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("apply A")
            .assert()
            .success()
            .stdout_eq("Applied branch 'A' to workspace\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_rewording_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["9ac4652", "-m", "reworded"])
            .assert()
            .success()
            .stdout_eq("Updated commit message for [..] (now [..])\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_rewording_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("reword")
            .args(["A", "-m", "reworded-branch"])
            .assert()
            .success()
            .stdout_eq("Renamed branch 'A' to 'reworded-branch'\n")
            .stderr_eq("");

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_but_branch_new_at_base() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["new", "foo"]).assert().success();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_but_branch_in_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    env.but("branch").args(["new", "foo"]).assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch")
            .args(["new", "bar", "-a", "foo"])
            .assert()
            .success();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_but_branch_delete() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    env.but("branch").args(["new", "foo"]).assert().success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["delete", "foo"]).assert().success();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_but_branch_delete_in_stack() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    env.but("branch").args(["new", "foo"]).assert().success();
    env.but("branch")
        .args(["new", "bar", "-a", "foo"])
        .assert()
        .success();

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("branch").args(["delete", "bar"]).assert().success();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_but_absorb() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    env.file("first", "This is new stuff");

    run_mutate_undo_roundtrip_test(&env, |env| {
        env.but("absorb").assert().success();

        Ok(())
    })?;

    Ok(())
}

#[test]
fn can_undo_repeatedly() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let (status_one, new_commit) = reword(&env, "9ac4652", "one")?;
    let (status_two, new_commit) = reword(&env, &new_commit, "two")?;
    let (status_three, new_commit) = reword(&env, &new_commit, "three")?;
    reword(&env, &new_commit, "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
800274e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    Ok(())
}

#[test]
fn can_undo_explicit_restore() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let (_, new_commit) = reword(&env, "9ac4652", "one")?;
    let (status_two, new_commit) = reword(&env, &new_commit, "two")?;
    let (_, new_commit) = reword(&env, &new_commit, "three")?;
    let (status_four, _) = reword(&env, &new_commit, "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    restore(&env, "e637109", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
10e3b45 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e637109)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshot,
        "10e3b45",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
a875d2c 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
10e3b45 2000-01-02 00:00:00 [RESTORE] Restored from snapshot: Updated commit message (e637109)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    Ok(())
}

#[test]
fn can_undo_perform_operation_then_undo_again() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let (_, new_commit) = reword(&env, "9ac4652", "one")?;
    let (status_two, new_commit) = reword(&env, &new_commit, "two")?;
    let (status_three, new_commit) = reword(&env, &new_commit, "three")?;
    reword(&env, &new_commit, "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    reword(&env, &new_commit, "three-new")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
3b50353 2000-01-02 00:00:00 [REWORD] Updated commit message
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "3b50353",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
c00e67a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (3b50353)
3b50353 2000-01-02 00:00:00 [REWORD] Updated commit message
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );

    Ok(())
}

#[test]
fn undoing_past_end_of_oplog() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let status_zero = env.but("status").output()?;
    let (status_one, new_commit) = reword(&env, "9ac4652", "one")?;
    reword(&env, &new_commit, "two")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
1c40c14 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "7665ea7",
        &status_zero,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
092cc32 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (7665ea7)
1c40c14 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("undo").assert().success().stdout_eq(
        r#"No previous operations to undo.
"#,
    );

    Ok(())
}

#[test]
fn can_redo() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let (_, new_commit) = reword(&env, "9ac4652", "one")?;
    let (_, new_commit) = reword(&env, &new_commit, "two")?;
    let (status_three, new_commit) = reword(&env, &new_commit, "three")?;
    let (status_four, _) = reword(&env, &new_commit, "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "0a0795e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
b57a0e1 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f4e985f)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );

    Ok(())
}

#[test]
fn can_mix_undo_and_redo() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let (status_one, new_commit) = reword(&env, "9ac4652", "one")?;
    let (status_two, new_commit) = reword(&env, &new_commit, "two")?;
    let (status_three, new_commit) = reword(&env, &new_commit, "three")?;
    let (status_four, _) = reword(&env, &new_commit, "four")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "f4e985f",
        &status_three,
    );
    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "e637109",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "8b8b27a",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::RestoreFromSnapshotViaRedo,
        "d07be52",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    undo(
        &env,
        OperationKind::UpdateCommitMessage,
        "90c8e9b",
        &status_one,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "5ffc4e1",
        &status_two,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "4769e9f",
        &status_three,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
62ccc54 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    redo(
        &env,
        OperationKind::RestoreFromSnapshotViaUndo,
        "0a0795e",
        &status_four,
    );

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Operations History
──────────────────────────────────────────────────
9306108 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (f4e985f)
62ccc54 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
6ba0709 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (90c8e9b)
5ffc4e1 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (90c8e9b)
4769e9f 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
d07be52 2000-01-02 00:00:00 [REDO] Restored from snapshot: Updated commit message (e637109)
8b8b27a 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (e637109)
0a0795e 2000-01-02 00:00:00 [UNDO] Restored from snapshot: Updated commit message (f4e985f)
f4e985f 2000-01-02 00:00:00 [REWORD] Updated commit message
e637109 2000-01-02 00:00:00 [REWORD] Updated commit message
90c8e9b 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message

"#]]);

    Ok(())
}

#[test]
fn cannot_redo_without_undoing_first() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    reword(&env, "9ac4652", "one")?;

    env.but("redo").assert().success().stdout_eq(
        r#"No previous undo to redo.
"#,
    );

    Ok(())
}

/*

but commit

but commit empty
but commit empty <target>
but commit empty --before <commit>
but commit empty --after <commit>

 */
