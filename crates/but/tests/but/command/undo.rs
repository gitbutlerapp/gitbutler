use crate::utils::Sandbox;

mod undo_rub;

/// Run an undo test tests a roundtrip `mutate` -> `but undo`, and asserts that the status output is
/// the same before and after the roundtrip.
fn run_mutate_undo_roundtrip_test<F>(env: &Sandbox, mutate: F) -> anyhow::Result<()>
where
    F: FnOnce(&Sandbox) -> anyhow::Result<()>,
{
    // Arrange
    let status_output_before = env.but("status").output()?;

    {
        eprintln!("Status before mutation:");
        let output = String::from_utf8(status_output_before.stdout.clone()).unwrap();
        for line in output.lines() {
            eprintln!("    {line}");
        }
    }

    mutate(env)?;
    let status_output_after_mutate = env.but("status").output()?;

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
    operation_reverted_to: &str,
    snapshot_restored_to: &str,
    expected_status: &std::process::Output,
) {
    env.but("undo").assert().success().stdout_eq(format!(
        r#"Undoing operation...
  Reverting to: {operation_reverted_to} (2000-01-02 00:00:00)
✓ Undo completed successfully! Restored to snapshot: {snapshot_restored_to}
"#
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
    operation_reverted_to: &str,
    snapshot_restored_to: &str,
    expected_status: &std::process::Output,
) {
    env.but("redo").assert().success().stdout_eq(format!(
        r#"Redoing operation...
  Reverting to: {operation_reverted_to} (2000-01-02 00:00:00)
✓ Redo completed successfully! Restored to snapshot: {snapshot_restored_to}
"#,
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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "5129db9", &status_three);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "da67dd1", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "e23e4fa", &status_one);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
e1cac61 2000-01-02 00:00:00 [UNDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    restore(&env, "da67dd1", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
d540910 2000-01-02 00:00:00 [RESTORE] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "RestoreFromSnapshot", "d540910", &status_four);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
1cc75b3 2000-01-02 00:00:00 [UNDO] Restored from snapshot
d540910 2000-01-02 00:00:00 [RESTORE] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "5129db9", &status_three);

    reword(&env, &new_commit, "three-new")?;

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
a69c9b7 2000-01-02 00:00:00 [REWORD] Updated commit message
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "a69c9b7", &status_three);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
8d13d35 2000-01-02 00:00:00 [UNDO] Restored from snapshot
a69c9b7 2000-01-02 00:00:00 [REWORD] Updated commit message
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "da67dd1", &status_two);

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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "e23e4fa", &status_one);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
419ccaa 2000-01-02 00:00:00 [UNDO] Restored from snapshot
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "7665ea7", &status_zero);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
213dc85 2000-01-02 00:00:00 [UNDO] Restored from snapshot
419ccaa 2000-01-02 00:00:00 [UNDO] Restored from snapshot
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "5129db9", &status_three);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    redo(&env, "RestoreFromSnapshotViaUndo", "df4a12d", &status_four);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
3452ba9 2000-01-02 00:00:00 [REDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

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
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "5129db9", &status_three);
    undo(&env, "UpdateCommitMessage", "da67dd1", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    redo(&env, "RestoreFromSnapshotViaUndo", "22777d1", &status_three);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "RestoreFromSnapshotViaRedo", "5427f25", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
a7309a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    undo(&env, "UpdateCommitMessage", "e23e4fa", &status_one);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
445c1d2 2000-01-02 00:00:00 [UNDO] Restored from snapshot
a7309a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    redo(&env, "RestoreFromSnapshotViaUndo", "445c1d2", &status_two);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5748be7 2000-01-02 00:00:00 [REDO] Restored from snapshot
445c1d2 2000-01-02 00:00:00 [UNDO] Restored from snapshot
a7309a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    redo(&env, "RestoreFromSnapshotViaUndo", "a7309a9", &status_three);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
5794c5d 2000-01-02 00:00:00 [REDO] Restored from snapshot
5748be7 2000-01-02 00:00:00 [REDO] Restored from snapshot
445c1d2 2000-01-02 00:00:00 [UNDO] Restored from snapshot
a7309a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

    redo(&env, "RestoreFromSnapshotViaUndo", "df4a12d", &status_four);

    env.but("oplog")
        .args(["list"])
        .assert()
        .success()
        .stdout_eq(
            r#"Operations History
──────────────────────────────────────────────────
c5e31c1 2000-01-02 00:00:00 [REDO] Restored from snapshot
5794c5d 2000-01-02 00:00:00 [REDO] Restored from snapshot
5748be7 2000-01-02 00:00:00 [REDO] Restored from snapshot
445c1d2 2000-01-02 00:00:00 [UNDO] Restored from snapshot
a7309a9 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5427f25 2000-01-02 00:00:00 [REDO] Restored from snapshot
22777d1 2000-01-02 00:00:00 [UNDO] Restored from snapshot
df4a12d 2000-01-02 00:00:00 [UNDO] Restored from snapshot
5129db9 2000-01-02 00:00:00 [REWORD] Updated commit message
da67dd1 2000-01-02 00:00:00 [REWORD] Updated commit message
e23e4fa 2000-01-02 00:00:00 [REWORD] Updated commit message
7665ea7 2000-01-02 00:00:00 [REWORD] Updated commit message
"#,
        );

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
