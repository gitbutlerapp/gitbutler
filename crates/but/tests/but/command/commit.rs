use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn commit_with_message_from_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Create a file with the commit message
    env.file(
        "commit-msg.txt",
        "Add new file from message file\n\nThis is the body of the commit message.",
    );

    // Commit with file flag
    env.but("commit -f commit-msg.txt")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch A

"#]]);

    // Verify the commit was created with the correct message
    let log = env.git_log()?;
    assert!(log.contains("Add new file from message file"));

    Ok(())
}

#[test]
fn commit_with_message_file_not_found() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Try to commit with a non-existent file
    env.but("commit -f nonexistent-file.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Failed to read commit message from file: nonexistent-file.txt

Caused by:
    No such file or directory (os error 2)

"#]]);

    Ok(())
}

#[test]
fn commit_with_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Commit with message flag
    env.but("commit -m 'Add new file'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch A

"#]]);

    // Verify the commit was created
    let log = env.git_log()?;
    assert!(log.contains("Add new file"));

    Ok(())
}

#[test]
fn commit_with_branch_hint() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;

    // Create a change
    env.file("file-for-b.txt", "content for B");

    // Commit to specific branch
    env.but("commit -m 'Change for B' B")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch B

"#]]);

    let log = env.git_log()?;
    assert!(log.contains("Change for B"));

    Ok(())
}

#[test]
fn commit_with_nonexistent_branch_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;

    env.file("file.txt", "content");

    env.but("commit -m 'test' nonexistent")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch 'nonexistent' not found

"#]]);

    Ok(())
}

#[test]
fn commit_with_create_flag_creates_new_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A", "B"])?;

    env.file("new-feature.txt", "new feature");

    env.but("commit -m 'New feature' -c feature-x")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created new independent branch 'feature-x'
✓ Created commit [..] on branch feature-x

"#]]);

    env.but("oplog")
        .with_assert(env.assert_with_oplog_redactions())
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Operations History
──────────────────────────────────────────────────
[HASH] 2000-01-02 00:00:00 [CREATE] CreateCommit

"#]]);
    Ok(())
}

#[test]
fn commit_empty_with_before_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Get the commit ID from the CLI ID map
    // Use the short git hash for the commit on branch A
    env.but("commit empty --before 9477ae7")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit before commit 9477ae7

"#]]);

    // Verify a new commit was created
    let log = env.git_log()?;
    // Should have one more commit than before
    assert!(log.lines().filter(|l| l.starts_with("*")).count() > 3);

    Ok(())
}

#[test]
fn commit_empty_with_positional_target_defaults_to_before() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use positional argument without flag (should default to --before behavior)
    env.but("commit empty 9477ae7").assert().success().stdout_eq(str![[r#"
Created blank commit before commit 9477ae7

"#]]);

    // Verify a new commit was created
    let log = env.git_log()?;
    // Should have one more commit than before
    assert!(log.lines().filter(|l| l.starts_with("*")).count() > 3);

    Ok(())
}

#[test]
#[ignore = "Inserting after a branch reference is not currently supported by the underlying API"]
fn commit_empty_with_after_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Insert empty commit after (at top of) branch A
    env.but("commit empty --after A").assert().success().stdout_eq(str![[r#"
Created blank commit at the top of stack 'A'

"#]]);

    // Verify a new commit was created
    let log = env.git_log()?;
    // Should have one more commit than before
    assert!(log.lines().filter(|l| l.starts_with("*")).count() > 3);

    Ok(())
}

#[test]
#[ignore = "Inserting before a branch reference is not currently supported by the underlying API"]
fn commit_empty_with_before_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Insert empty commit before branch A (at bottom of stack)
    // Note: This currently fails with "Commit has parents that are not referenced"
    // which suggests the underlying API doesn't properly support InsertSide::Below with branches
    env.but("commit empty --before A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit at the bottom of stack 'A'

"#]]);

    // Verify a new commit was created
    let log = env.git_log()?;
    // Should have one more commit than before
    assert!(log.lines().filter(|l| l.starts_with("*")).count() > 3);

    Ok(())
}

#[test]
fn commit_empty_with_after_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Insert empty commit after a specific commit
    env.but("commit empty --after 9477ae7")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit after commit 9477ae7

"#]]);

    // Verify a new commit was created
    let log = env.git_log()?;
    // Should have one more commit than before
    assert!(log.lines().filter(|l| l.starts_with("*")).count() > 3);

    Ok(())
}

#[test]
fn commit_empty_without_branches_fails() -> anyhow::Result<()> {
    // This test uses a scenario with no GitButler branches to verify error handling
    let env = Sandbox::init_scenario_with_target_and_default_settings("first-commit")?;

    // Try to run without any arguments when there are no branches
    env.but("commit empty").assert().failure().stderr_eq(str![[r#"
Error: No branches found. Create a branch first or specify a target explicitly.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_both_flags() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use both --before and --after
    env.but("commit empty --before A --after A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--before <BEFORE>' cannot be used with '--after <AFTER>'

Usage: but commit empty --before <BEFORE> [TARGET]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_with_nonexistent_target() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to insert before a nonexistent target
    env.but("commit empty --before nonexistent")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Target 'nonexistent' not found

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --message with empty subcommand
    env.but("commit -m 'test' empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --message cannot be used with 'commit empty'. Empty commits have no message by default.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_file_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("msg.txt", "test message");

    // Try to use --file with empty subcommand
    env.but("commit -f msg.txt empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --file cannot be used with 'commit empty'. Empty commits have no message by default.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_branch_argument() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use branch argument with empty subcommand
    env.but("commit A empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: branch argument cannot be used with 'commit empty'. Use the target positional argument or --before/--after flags.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_only_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --only with empty subcommand
    env.but("commit --only empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --only cannot be used with 'commit empty'.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_create_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --create with empty subcommand
    env.but("commit --create empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --create cannot be used with 'commit empty'.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_no_hooks_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --no-hooks with empty subcommand
    env.but("commit --no-hooks empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --no-hooks cannot be used with 'commit empty'.

"#]]);

    Ok(())
}

#[test]
fn commit_ai_conflicts_with_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "test content");

    // Try to use both --ai and -m
    env.but("commit --ai -m 'test message'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai [<AI>]' cannot be used with '--message <MESSAGE>'

Usage: but commit --ai [<AI>] [BRANCH]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn commit_ai_conflicts_with_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "test content");
    env.file("msg.txt", "commit message");

    // Try to use both --ai and -f
    env.but("commit --ai -f msg.txt").assert().failure().stderr_eq(str![[r#"
error: the argument '--ai [<AI>]' cannot be used with '--file <FILE>'

Usage: but commit --ai [<AI>] [BRANCH]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_ai_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --ai with empty subcommand
    // Note: Using -i= (with explicit empty value) to avoid clap treating "empty" as the flag's value
    env.but("commit -i= empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --ai cannot be used with 'commit empty'.

"#]]);

    Ok(())
}
