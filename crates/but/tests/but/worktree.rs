use crate::utils::{Sandbox, setup_metadata};

#[test]
fn worktree_new_with_default_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree without custom name - should use shortened branch name
    env.but("worktree new A")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_default_name.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_with_custom_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree with custom name
    env.but("worktree new A -b experiment")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_custom_name.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_deduplication() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create first worktree with name "test"
    env.but("worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_first.stdout.term.svg"
        ]);

    // Create second worktree with same name - should get "test-a"
    env.but("worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_second.stdout.term.svg"
        ]);

    // Create third worktree with same name - should get "test-b"
    env.but("worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_third.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_json_output() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Test JSON output
    let output = env
        .but("--json worktree new A -b json-test")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output)?;
    let json: serde_json::Value = serde_json::from_str(&json_str)?;

    // Verify JSON structure
    assert!(json["created"].is_object());
    assert!(json["branchName"].is_string());
    assert_eq!(
        json["branchName"].as_str().unwrap(),
        "gitbutler/worktree/json-test"
    );
    assert!(json["created"]["path"].is_string());
    // Note: id might be serialized as a structured object, so we just check it exists
    assert!(
        json["created"]["id"].is_string()
            || json["created"]["id"].is_object()
            || json["created"]["id"].is_array()
    );

    Ok(())
}

#[test]
fn worktree_new_with_nonexistent_branch_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Try to create worktree from non-existent branch
    env.but("worktree new nonexistent")
        .assert()
        .failure()
        .stderr_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_nonexistent_branch_fails.stderr.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_list_after_creation() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create a worktree
    env.but("worktree new A -b list-test").assert().success();

    // List worktrees - should show our newly created one
    env.but("worktree list")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_list_after_creation.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_multiple_from_different_branches() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree from branch A
    env.but("worktree new A -b from-a")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_from_a.stdout.term.svg"
        ]);

    // Create worktree from branch B
    env.but("worktree new B -b from-b")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_from_b.stdout.term.svg"
        ]);

    // List should show both
    env.but("worktree list")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_list.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_with_long_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Test with --name long flag instead of -b
    env.but("worktree new A --name long-flag-test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_long_flag.stdout.term.svg"
        ]);

    Ok(())
}

#[test]
fn worktree_new_default_uses_shortened_branch_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create without custom name - should deduplicate using shortened ref
    env.but("worktree new A").assert().success();

    // Second one should get -a suffix
    env.but("worktree new A")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_default_deduplication.stdout.term.svg"
        ]);

    Ok(())
}

// JSON snapshot tests - mirroring all terminal output tests

#[test]
fn worktree_new_with_default_name_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree without custom name - should use shortened branch name
    env.but("--json worktree new A")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_default_name.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_new_with_custom_name_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree with custom name
    env.but("--json worktree new A -b experiment")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_custom_name.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_new_deduplication_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create first worktree with name "test"
    env.but("--json worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_first.stdout.json"
        ]);

    // Create second worktree with same name - should get "test-a"
    env.but("--json worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_second.stdout.json"
        ]);

    // Create third worktree with same name - should get "test-b"
    env.but("--json worktree new A -b test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_deduplication_third.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_list_after_creation_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create a worktree
    env.but("worktree new A -b list-test").assert().success();

    // List worktrees - should show our newly created one
    env.but("--json worktree list")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_list_after_creation.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_new_multiple_from_different_branches_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create worktree from branch A
    env.but("--json worktree new A -b from-a")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_from_a.stdout.json"
        ]);

    // Create worktree from branch B
    env.but("--json worktree new B -b from-b")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_from_b.stdout.json"
        ]);

    // List should show both
    env.but("--json worktree list")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_multiple_list.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_new_with_long_flag_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Test with --name long flag instead of -b
    env.but("--json worktree new A --name long-flag-test")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_with_long_flag.stdout.json"
        ]);

    Ok(())
}

#[test]
fn worktree_new_default_uses_shortened_branch_name_json() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    setup_metadata(&env, &["A", "B"])?;

    // Create without custom name - should deduplicate using shortened ref
    env.but("--json worktree new A").assert().success();

    // Second one should get -a suffix
    env.but("--json worktree new A")
        .with_assert(env.assert_with_path_redactions())
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/worktree/worktree_new_default_deduplication.stdout.json"
        ]);

    Ok(())
}
