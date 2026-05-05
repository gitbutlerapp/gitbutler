use crate::utils::Sandbox;

/// Run an undo test tests a roundtrip `mutate` -> `but undo`, and asserts that the status output is
/// the same before and after the roundtrip.
fn run_mutate_undo_roundtrip_test<F>(env: &Sandbox, mutate: F) -> anyhow::Result<()>
where
    F: FnOnce(&Sandbox) -> anyhow::Result<()>,
{
    // Arrange
    let status_output_before = env.but("status").output()?;
    mutate(env)?;
    let status_output_after_mutate = env.but("status").output()?;
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

#[test]
fn can_undo_discard() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    let path = "new-file.txt";
    env.file(path, "content");

    let mutate = |env: &Sandbox| -> anyhow::Result<()> {
        env.but("discard")
            .arg(path)
            .assert()
            .success()
            .stdout_eq("Successfully discarded changes to 1 item\n")
            .stderr_eq("");

        Ok(())
    };

    run_mutate_undo_roundtrip_test(&env, mutate)?;

    Ok(())
}

#[test]
fn can_undo_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;
    let path = "new-file.txt";
    env.file(path, "content");

    let mutate = |env: &Sandbox| -> anyhow::Result<()> {
        env.file("new-file.txt", "content");

        env.but("commit -m 'Add file'")
            .assert()
            .success()
            .stdout_eq("✓ Created commit [..] on branch A\n")
            .stderr_eq("");

        Ok(())
    };

    run_mutate_undo_roundtrip_test(&env, mutate)?;

    Ok(())
}

#[test]
fn can_undo_rubbing_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    let mutate = |env: &Sandbox| -> anyhow::Result<()> {
        env.but("rub")
            .arg("9a")
            .arg("fe")
            .assert()
            .success()
            .stdout_eq("Squashed 9ac4652 → f66c907\n")
            .stderr_eq("");

        Ok(())
    };

    run_mutate_undo_roundtrip_test(&env, mutate)?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not restore the worktree file A for some reason"]
fn can_undo_unapply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let mutate = |env: &Sandbox| -> anyhow::Result<()> {
        env.but("unapply A")
            .assert()
            .success()
            .stdout_eq("Unapplied stack with branches 'A' from workspace\n")
            .stderr_eq("");

        Ok(())
    };

    run_mutate_undo_roundtrip_test(&env, mutate)?;

    Ok(())
}

#[test]
#[ignore = "Test harness runs with cv3 feature flag, and but_core::worktree::safe_checkout_from_head does not remove the worktree file A for some reason"]
fn can_undo_clean_apply() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.but("unapply A").assert().success();

    let mutate = |env: &Sandbox| -> anyhow::Result<()> {
        env.but("apply A")
            .assert()
            .success()
            .stdout_eq("Applied branch 'A' to workspace\n")
            .stderr_eq("");

        Ok(())
    };

    run_mutate_undo_roundtrip_test(&env, mutate)?;

    Ok(())
}
