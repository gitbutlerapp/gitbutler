use crate::utils::Sandbox;

mod undo_redo;
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

/*

but commit

but commit empty
but commit empty <target>
but commit empty --before <commit>
but commit empty --after <commit>

 */
