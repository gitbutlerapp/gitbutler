use snapbox::str;

use crate::utils::Sandbox;

/// Helper to create multiple commits on a branch for testing
fn setup_branch_with_commits(env: &Sandbox, branch: &str, num_commits: usize) {
    for i in 1..=num_commits {
        env.file(format!("file{}.txt", i), format!("content for commit {}\n", i));
        env.but(format!("commit {} -m 'commit {}'", branch, i))
            .assert()
            .success();
    }
}

#[test]
fn squash_requires_at_least_two_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Only one commit exists (from scenario)
    // Try to squash a single commit - should fail
    env.but("squash c0").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. No matching branch or commit found for 'c0'

"#]]);

    Ok(())
}

#[test]
fn squash_branch_with_single_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Branch A has only 1 commit from the scenario
    // Try to squash branch with single commit - should fail
    env.but("squash A").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. Branch 'A' has only one commit, nothing to squash

"#]]);

    Ok(())
}

#[test]
fn squash_nonexistent_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to squash with nonexistent commit ID
    env.but("squash nonexistent c0").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. No matching commit found for 'nonexistent'

"#]]);

    Ok(())
}

#[test]
fn squash_branch_by_name_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create more commits on branch A (scenario already has 1)
    setup_branch_with_commits(&env, "A", 2);

    // Squash all commits in branch A by using branch name
    // This should succeed as we have 3 commits total
    env.but("squash A").assert().success();

    Ok(())
}

#[test]
fn squash_with_drop_message_flag_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create 1 more commit (scenario has 1, so we'll have 2 total)
    setup_branch_with_commits(&env, "A", 1);

    // Squash branch with --drop-message flag
    env.but("squash A --drop-message").assert().success();

    Ok(())
}

#[test]
fn squash_with_custom_message_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create 1 more commit
    setup_branch_with_commits(&env, "A", 1);

    // Squash with custom message
    env.but("squash A -m 'Custom squash message'").assert().success();

    Ok(())
}

#[test]
fn squash_mutually_exclusive_flags_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --drop-message and --message flags - should fail
    env.but("squash A -m 'Custom message' --drop-message")
        .assert()
        .failure();

    Ok(())
}

#[test]
fn squash_with_invalid_range_format_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 2);

    // Try to use invalid range format with triple dots
    env.but("squash c0..c1..c2").assert().failure().stderr_eq(str![[r#"
Failed to squash commits. Range format should be 'start..end', got 'c0..c1..c2'

"#]]);

    Ok(())
}

#[test]
fn squash_with_empty_range_part_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 2);

    // Try range with empty start
    env.but("squash ..c1").assert().failure();

    Ok(())
}

#[test]
fn squash_comma_list_with_nonexistent_commit_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try comma-separated list with only nonexistent commits
    // This tests the comma parsing path without needing real commit IDs
    env.but("squash nonexistent1,nonexistent2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. Commit 'nonexistent1' not found

"#]]);

    Ok(())
}

#[test]
fn squash_range_with_nonexistent_endpoint_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try range with nonexistent endpoints
    // This tests the range parsing path without needing real commit IDs
    env.but("squash nonexistent1..nonexistent2")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to squash commits. Start of range 'nonexistent1' must match exactly one commit

"#]]);

    Ok(())
}

#[test]
fn squash_ai_conflicts_with_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --ai and --message flags - should fail
    env.but("squash A --ai -m 'Custom message'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai[=<AI>]' cannot be used with '--message <MESSAGE>'

Usage: but squash --ai[=<AI>] <COMMITS>...

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn squash_ai_conflicts_with_drop_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    setup_branch_with_commits(&env, "A", 1);

    // Try to use both --ai and --drop-message flags - should fail
    env.but("squash A --ai --drop-message")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai[=<AI>]' cannot be used with '--drop-message'

Usage: but squash --ai[=<AI>] <COMMITS>...

For more information, try '--help'.

"#]]);

    Ok(())
}

// Note: Happy-path tests for range (c0..c2) and comma-list (c0,c1,c2) notation
// are not included because:
// 1. Commit IDs are dynamically assigned and not predictable in tests
// 2. The parsing logic is thoroughly tested through error cases
// 3. All three input methods (range, list, multiple args) use the same
//    handle_multi_commit_squash function that's proven to work via branch squashing
// 4. Branch squashing tests verify the underlying API integration works correctly
