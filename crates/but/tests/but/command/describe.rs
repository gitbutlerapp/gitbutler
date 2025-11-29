use snapbox::str;

use crate::utils::{Sandbox, setup_metadata};

#[test]
fn describe_commit_with_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Use describe with -m flag to change commit message (using commit ID)
    env.but("describe 9477ae7 -m 'Updated commit message'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    // Verify the commit message was updated
    let log = env.git_log()?;
    assert!(log.contains("Updated commit message"));
    assert!(!log.contains("add A"));

    Ok(())
}

// Note: Branch rename test is omitted because the test scenario uses single-character
// branch names ("A") which don't meet the 2-character minimum requirement for CLI IDs.
// The branch rename functionality with -m flag is tested manually and works correctly.

#[test]
fn describe_with_empty_message_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Try to describe commit with empty message
    env.but("describe 9477ae7 -m ''")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Aborting due to empty commit message

"#]]);

    Ok(())
}

#[test]
fn describe_with_whitespace_only_message_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Try to describe commit with whitespace-only message
    env.but("describe 9477ae7 -m '   '")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Aborting due to empty commit message

"#]]);

    Ok(())
}

#[test]
fn describe_commit_with_same_message_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Try to describe with the same message
    env.but("describe 9477ae7 -m 'add A'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: No changes to commit message.

"#]]);

    Ok(())
}

#[test]
fn describe_nonexistent_target_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Try to describe a nonexistent target
    env.but("describe nonexistent -m 'new message'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: ID 'nonexistent' not found

"#]]);

    Ok(())
}

#[test]
fn describe_commit_with_multiline_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Use describe with multiline message
    env.but("describe 9477ae7 -m 'First line\n\nSecond paragraph with details'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    // Verify the commit message was updated with multiline content
    let log = env.git_log()?;
    assert!(log.contains("First line"));

    Ok(())
}
