use snapbox::str;

use super::util;
use crate::utils::Sandbox;

#[test]
fn commit_with_message_from_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
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

    // Commit with message-file flag
    env.but("commit --message-file commit-msg.txt")
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
    env.but("commit --message-file nonexistent-file.txt")
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
    insta::assert_snapshot!(env.git_log()?, @"
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
fn commit_with_git_all_flag_prints_hint() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "test content");

    env.but("commit -am 'Add new file'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
no need for -a here my friend...
✓ Created commit [..] on branch A

"#]]);

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

    env.setup_metadata_at_target(&["A", "B"], "origin/main")?;

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

    env.setup_metadata_at_target(&["A", "B"], "origin/main")?;

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
[SHORTHASH] 2000-01-02 00:00:00 [COMMIT] Created commit
[SHORTHASH] 2000-01-02 00:00:00 [BRANCH] Created branch

"#]]);
    Ok(())
}

#[test]
fn commit_with_create_and_position_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "new feature");

    env.but("commit -m 'New feature' -c feature-x --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --create cannot be used with --before/--after.

"#]]);

    Ok(())
}

#[test]
fn commit_with_position_on_different_branch_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("new-file.txt", "content");

    let output = env
        .but("commit -m 'Wrong target' A --before d3e2ba3")
        .assert()
        .failure();
    let stderr = std::str::from_utf8(&output.get_output().stderr)?;
    assert!(
        stderr.contains("Target must belong to the branch being committed to"),
        "unexpected stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn commit_empty_default() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("commit empty")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit at the tip of branch 'A'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   2594ce3 (no commit message) (no changes)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn commit_empty_with_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    let output = env
        .but("commit empty -m 'Plan empty slot' --format json")
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;

    let status = util::status_json(&env)?;
    let commit = &status["stacks"][0]["branches"][0]["commits"][0];
    let message = commit["message"].as_str().unwrap();
    assert_eq!(message.trim_end(), "Plan empty slot");
    assert_eq!(json["commit_id"], commit["commitId"]);

    Ok(())
}

#[test]
fn commit_empty_rejects_empty_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    let log_before = env.git_log()?;

    env.but("commit empty -m '   '")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Aborting commit due to empty commit message.

"#]]);

    assert_eq!(env.git_log()?, log_before);

    Ok(())
}

#[test]
fn commit_empty_with_before_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
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
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use positional argument without flag (should default to --before behavior)
    env.but("commit empty 9477ae7")
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
/// Creating a commit above a stack head creates an anonymous segment as a direct child of the
/// workspace commit, i.e. an anonymous stack. This is not well supported and breaks `but status`.
///
/// We intend to support this together with the `--create` flag once `but commit empty` is migrated
/// up to `but commit --empty`.
fn commit_empty_after_stack_head_is_disallowed() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("commit empty --after A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input for '--after'

Cannot insert empty commit above stack head

Hint: Use '--before' to insert at the tip of the stack

"#]]);

    Ok(())
}

#[test]
fn commit_after_branch_is_disallowed() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "content");

    env.but("commit -m 'test' --after A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input for '--after'

Cannot insert commit after a branch

Hint: Use a commit ID with '--after', or use '--before <branch>' to insert at the branch tip

"#]]);

    Ok(())
}

#[test]
fn commit_empty_after_branch_for_non_stack_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack-two-commits")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new bottom")
        .arg("-a")
        .arg(env.open_repo()?.rev_parse("A~")?.to_string())
        .assert()
        .success();

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9ac4652 add second
┊│
┊├┄bo [bottom]
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    // Insert empty commit after branch A, with a new branch created to prevent an anonymous segment
    env.but("commit empty --after bottom")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit above branch 'bottom'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   76b24fa add second
┊●   32d198c (no commit message) (no changes)
┊│
┊├┄bo [bottom]
┊●   fe12bcd add first
├╯
┊
┴ 1bbc04b (common base) 2000-01-02 add Base

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn commit_empty_with_before_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("commit empty --before A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created blank commit at the tip of branch 'A'

"#]]);

    env.but("status").assert().success().stdout_eq(str![[r#"
╭┄zz [unassigned changes] (no changes)
┊
┊╭┄g0 [A]
┊●   2594ce3 (no commit message) (no changes)
┊●   9477ae7 add A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    Ok(())
}

#[test]
fn commit_empty_with_after_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
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
    env.but("commit empty")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
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
Error: Could not find target: 'nonexistent'

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_parent_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use parent --message with empty subcommand
    env.but("commit -m 'test' empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --message must be passed after 'empty'. Use `but commit empty -m "message"`.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_file_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("msg.txt", "test message");

    // Try to use --message-file with empty subcommand
    env.but("commit --message-file msg.txt empty --before A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --message-file cannot be used with 'commit empty'. Use `but commit empty -m "message"`.

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
fn commit_empty_rejects_parent_position_flags() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("commit --before A empty")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: --before/--after must be passed after 'empty'. Use `but commit empty --before <target>`.

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
error: the argument '--ai[=<AI>]' cannot be used with '--message <MESSAGE>'

Usage: but commit --ai[=<AI>] [BRANCH]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn commit_ai_conflicts_with_message_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "test content");
    env.file("msg.txt", "commit message");

    // Try to use both --ai and --message-file
    env.but("commit --ai --message-file msg.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--ai[=<AI>]' cannot be used with '--message-file <FILE>'

Usage: but commit --ai[=<AI>] [BRANCH]

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

#[test]
fn commit_changes_conflicts_with_only() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("file.txt", "content");

    // Try to use both --changes and --only
    env.but("commit -m 'test' --changes ab --only")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: the argument '--changes <CHANGES>' cannot be used with '--only'

Usage: but commit --message <MESSAGE> --changes <CHANGES> [BRANCH]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
fn commit_empty_rejects_changes_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Try to use --changes with empty subcommand
    // --changes is not a valid flag for the empty subcommand, so clap rejects it
    let output = env
        .but("commit empty --before A --changes ab")
        .assert()
        .failure();

    let stderr = std::str::from_utf8(&output.get_output().stderr)?;
    assert!(
        stderr.contains("unexpected argument"),
        "Expected clap to reject --changes with empty subcommand, got: {stderr}"
    );

    Ok(())
}

#[test]
fn commit_json_mode_requires_message_or_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Try to commit in JSON mode without -m or --message-file
    env.but("commit --format json")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Either --message (-m), --message-file, or --ai (-i) must be specified for this output format

"#]]);

    Ok(())
}

#[test]
fn commit_json_mode_with_message_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Commit in JSON mode with -m
    let output = env
        .but("commit --format json -m 'Test commit'")
        .assert()
        .success();

    // Parse JSON output
    let stdout = std::str::from_utf8(&output.get_output().stdout)?;
    let json: serde_json::Value = serde_json::from_str(stdout)?;

    // Verify JSON structure
    assert!(
        json["commit_id"].is_string(),
        "commit_id should be a string"
    );
    assert!(json["branch"].is_string(), "branch should be a string");
    assert_eq!(json["branch"].as_str().unwrap(), "A");

    Ok(())
}

#[test]
fn commit_json_mode_with_file_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Create a file with the commit message
    env.file("commit-msg.txt", "Test commit from file");

    // Commit in JSON mode with --message-file
    let output = env
        .but("commit --format json --message-file commit-msg.txt")
        .assert()
        .success();

    // Parse JSON output
    let stdout = std::str::from_utf8(&output.get_output().stdout)?;
    let json: serde_json::Value = serde_json::from_str(stdout)?;

    // Verify JSON structure
    assert!(
        json["commit_id"].is_string(),
        "commit_id should be a string"
    );
    assert!(json["branch"].is_string(), "branch should be a string");
    assert_eq!(json["branch"].as_str().unwrap(), "A");

    Ok(())
}

#[test]
fn commit_json_mode_multiple_branches_requires_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata_at_target(&["A", "B"], "origin/main")?;

    // Create a change
    env.file("new-file.txt", "test content");

    // Try to commit in JSON mode without specifying a branch
    env.but("commit --format json -m 'Test commit'")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Multiple branches found. Specify a branch to commit to using the branch argument

"#]]);

    Ok(())
}

#[test]
fn commit_json_mode_multiple_branches_with_branch_succeeds() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    // Create a change
    env.file("new-file.txt", "test content");

    // Commit in JSON mode with branch specified
    let output = env
        .but("commit --format json -m 'Test commit' B")
        .assert()
        .success();

    // Parse JSON output
    let stdout = std::str::from_utf8(&output.get_output().stdout)?;
    let json: serde_json::Value = serde_json::from_str(stdout)?;

    // Verify JSON structure
    assert!(
        json["commit_id"].is_string(),
        "commit_id should be a string"
    );
    assert!(json["branch"].is_string(), "branch should be a string");
    assert_eq!(json["branch"].as_str().unwrap(), "B");

    Ok(())
}

#[test]
fn commit_json_positioned_omits_branch_tip() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    env.file("new-file.txt", "test content");

    let output = env
        .but("commit --format json -m 'Test commit' A --before 9477ae7")
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;

    assert!(json["commit_id"].is_string());
    assert_eq!(json["branch"], "A");
    assert!(json.get("branch_tip").is_none());

    Ok(())
}

#[test]
fn commit_with_specific_file_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create two files
    env.file("file1.txt", "content 1");
    env.file("file2.txt", "content 2");

    // Get file IDs from status
    let status_output = env.but("status --format json").assert().success();
    let stdout = std::str::from_utf8(&status_output.get_output().stdout)?;
    let status: serde_json::Value = serde_json::from_str(stdout)?;

    // Find the CLI ID for file1.txt from unassignedChanges
    let file1_id = status["unassignedChanges"]
        .as_array()
        .and_then(|changes| {
            changes.iter().find_map(|c| {
                if c["filePath"].as_str() == Some("file1.txt") {
                    c["cliId"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .expect("file1.txt should have a CLI ID");

    // Commit only file1.txt using its CLI ID
    env.but(format!("commit -m 'Add file1 only' --changes {file1_id}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch A

"#]]);

    // Verify file1 was committed
    let log = env.git_log()?;
    assert!(log.contains("Add file1 only"));

    // Verify file2 is still uncommitted
    let status_after = env.but("status --format json").assert().success();
    let stdout_after = std::str::from_utf8(&status_after.get_output().stdout)?;
    let status_after: serde_json::Value = serde_json::from_str(stdout_after)?;

    let has_file2 = status_after["unassignedChanges"]
        .as_array()
        .map(|changes| {
            changes
                .iter()
                .any(|c| c["filePath"].as_str() == Some("file2.txt"))
        })
        .unwrap_or(false);
    assert!(has_file2, "file2.txt should still be uncommitted");

    Ok(())
}

#[test]
fn commit_with_multiple_file_ids() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create three files
    env.file("file1.txt", "content 1");
    env.file("file2.txt", "content 2");
    env.file("file3.txt", "content 3");

    // Get file IDs from status
    let status_output = env.but("status --format json").assert().success();
    let stdout = std::str::from_utf8(&status_output.get_output().stdout)?;
    let status: serde_json::Value = serde_json::from_str(stdout)?;

    // Find CLI IDs for file1 and file2
    let changes = status["unassignedChanges"]
        .as_array()
        .expect("should have changes");
    let file1_id = changes
        .iter()
        .find(|c| c["filePath"].as_str() == Some("file1.txt"))
        .and_then(|c| c["cliId"].as_str())
        .expect("file1 should have ID");
    let file2_id = changes
        .iter()
        .find(|c| c["filePath"].as_str() == Some("file2.txt"))
        .and_then(|c| c["cliId"].as_str())
        .expect("file2 should have ID");

    // Commit file1 and file2, leaving file3 uncommitted
    env.but(format!(
        "commit -m 'Add two files' --changes {file1_id},{file2_id}"
    ))
    .assert()
    .success();

    // Verify file3 is still uncommitted
    let status_after = env.but("status --format json").assert().success();
    let stdout_after = std::str::from_utf8(&status_after.get_output().stdout)?;
    let status_after: serde_json::Value = serde_json::from_str(stdout_after)?;

    let remaining: Vec<&str> = status_after["unassignedChanges"]
        .as_array()
        .map(|changes| {
            changes
                .iter()
                .filter_map(|c| c["filePath"].as_str())
                .collect()
        })
        .unwrap_or_default();

    assert_eq!(
        remaining,
        vec!["file3.txt"],
        "Only file3 should remain uncommitted"
    );

    Ok(())
}

#[test]
fn commit_with_short_changes_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create two files
    env.file("file1.txt", "content 1");
    env.file("file2.txt", "content 2");

    // Get file ID from status
    let status_output = env.but("status --format json").assert().success();
    let stdout = std::str::from_utf8(&status_output.get_output().stdout)?;
    let status: serde_json::Value = serde_json::from_str(stdout)?;

    let file1_id = status["unassignedChanges"]
        .as_array()
        .and_then(|changes| {
            changes.iter().find_map(|c| {
                if c["filePath"].as_str() == Some("file1.txt") {
                    c["cliId"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .expect("file1.txt should have a CLI ID");

    // Use short flag -p instead of --changes
    env.but(format!("commit -m 'Using short flag' -p {file1_id}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch A

"#]]);

    // Verify file2 is still uncommitted
    let status_after = env.but("status --format json").assert().success();
    let stdout_after = std::str::from_utf8(&status_after.get_output().stdout)?;
    let status_after: serde_json::Value = serde_json::from_str(stdout_after)?;

    let remaining: Vec<&str> = status_after["unassignedChanges"]
        .as_array()
        .map(|changes| {
            changes
                .iter()
                .filter_map(|c| c["filePath"].as_str())
                .collect()
        })
        .unwrap_or_default();

    assert_eq!(
        remaining,
        vec!["file2.txt"],
        "file2.txt should still be uncommitted"
    );

    Ok(())
}

#[test]
fn commit_with_invalid_file_id_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;

    // Create a file so we have something to potentially commit
    env.file("file.txt", "content");

    // Try to commit with a nonexistent file ID
    env.but("commit -m 'test' --changes zq")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Invalid file ID(s):
  'zq' not found. Run 'but status' to see available file IDs.

"#]]);

    Ok(())
}

#[test]
fn commit_with_wrong_entity_type_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata_at_target(&["A"], "origin/main")?;

    // Create a file
    env.file("file.txt", "content");

    // Try to commit using a branch ID instead of file ID
    // "A" is a branch name which should fail
    env.but("commit -m 'test' --changes A")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Invalid file ID(s):
  'A' is a branch but must be an uncommitted file or hunk

"#]]);

    Ok(())
}

#[test]
fn commit_with_file_assigned_to_different_stack_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    // Create a file
    env.file("file.txt", "content");

    // Stage the file to branch A
    env.but("stage file.txt A").assert().success();

    // Try to commit the file to branch B (should fail because it's staged to A)
    let output = env
        .but("commit -m 'test' B --changes A@{stack}:file.txt")
        .assert()
        .failure();

    // Verify error message contains the expected text
    let stderr = std::str::from_utf8(&output.get_output().stderr)?;
    assert!(
        stderr.contains("is assigned to a different stack"),
        "Expected error about different stack assignment, got: {stderr}"
    );

    Ok(())
}

#[test]
fn commit_with_empty_file_list_uses_all_files() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create two files
    env.file("file1.txt", "content 1");
    env.file("file2.txt", "content 2");

    // Commit without specifying files (should include both)
    env.but("commit -m 'Add both files'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
✓ Created commit [..] on branch A

"#]]);

    // Verify both files were committed (no uncommitted files left)
    let status_after = env.but("status --format json").assert().success();
    let stdout_after = std::str::from_utf8(&status_after.get_output().stdout)?;
    let status_after: serde_json::Value = serde_json::from_str(stdout_after)?;

    let unassigned = status_after["unassignedChanges"].as_array();
    assert!(
        unassigned.map(|f| f.is_empty()).unwrap_or(true),
        "All files should be committed"
    );

    Ok(())
}

#[test]
fn commit_with_multiple_hunk_ids_from_same_file() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a file with content that will result in multiple hunks when modified
    env.file(
        "multi-hunk.txt",
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
    );

    // Commit initial file
    env.but("commit -m 'Initial file'").assert().success();

    // Modify the file in two non-adjacent places to create two hunks
    env.file(
        "multi-hunk.txt",
        "MODIFIED1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nMODIFIED10\n",
    );

    // Get hunk IDs from status
    let status_output = env.but("status --format json -f").assert().success();
    let stdout = std::str::from_utf8(&status_output.get_output().stdout)?;
    let status: serde_json::Value = serde_json::from_str(stdout)?;

    // Find all hunk IDs for multi-hunk.txt
    let hunk_ids: Vec<String> = status["unassignedChanges"]
        .as_array()
        .map(|changes| {
            changes
                .iter()
                .filter(|c| c["filePath"].as_str() == Some("multi-hunk.txt"))
                .filter_map(|c| c["cliId"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // If we have multiple hunks, test that specifying both works
    if hunk_ids.len() >= 2 {
        let ids_arg = hunk_ids.join(",");
        env.but(format!("commit -m 'Both hunks' --changes {ids_arg}"))
            .assert()
            .success();

        // Verify no uncommitted changes left
        let status_after = env.but("status --format json").assert().success();
        let stdout_after = std::str::from_utf8(&status_after.get_output().stdout)?;
        let status_after: serde_json::Value = serde_json::from_str(stdout_after)?;

        let remaining: Vec<&str> = status_after["unassignedChanges"]
            .as_array()
            .map(|changes| {
                changes
                    .iter()
                    .filter_map(|c| c["filePath"].as_str())
                    .collect()
            })
            .unwrap_or_default();

        assert!(
            !remaining.contains(&"multi-hunk.txt"),
            "All hunks from multi-hunk.txt should be committed"
        );
    } else {
        // If only one hunk was created (due to context lines merging them),
        // just verify commit works with that single ID
        env.but(format!("commit -m 'Single hunk' --changes {}", hunk_ids[0]))
            .assert()
            .success();
    }

    Ok(())
}

#[test]
fn commit_single_hunk_leaves_other_hunks_uncommitted() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    // Create a file with content that will result in multiple hunks when modified
    env.file(
        "multi-hunk.txt",
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
    );

    // Commit initial file
    env.but("commit -m 'Initial file'").assert().success();

    // Modify the file in two non-adjacent places to create two hunks
    env.file(
        "multi-hunk.txt",
        "MODIFIED1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nMODIFIED10\n",
    );

    // Get hunk IDs from diff (which shows individual hunks)
    let diff_output = env.but("diff --format json").assert().success();
    let stdout = std::str::from_utf8(&diff_output.get_output().stdout)?;
    let diff: serde_json::Value = serde_json::from_str(stdout)?;

    // Collect all change IDs from diff output
    let change_ids: Vec<String> = diff["changes"]
        .as_array()
        .map(|changes| {
            changes
                .iter()
                .filter_map(|c| c["id"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // If we have multiple changes (hunks), test committing just the first one
    if change_ids.len() >= 2 {
        let first_hunk_id = &change_ids[0];

        // Commit only the first hunk
        env.but(format!(
            "commit -m 'First hunk only' --changes {first_hunk_id}"
        ))
        .assert()
        .success();

        // Verify there are still uncommitted changes (the second hunk)
        let diff_after = env.but("diff --format json").assert().success();
        let stdout_after = std::str::from_utf8(&diff_after.get_output().stdout)?;
        let diff_after: serde_json::Value = serde_json::from_str(stdout_after)?;

        let remaining_changes = diff_after["changes"]
            .as_array()
            .map(|c| c.len())
            .unwrap_or(0);
        assert!(
            remaining_changes >= 1,
            "Second hunk should still be uncommitted, found {remaining_changes} changes"
        );
    }

    Ok(())
}

#[test]
fn committing_to_existing_branch_with_same_name_as_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    // create a file with the same name as the branch
    env.file("A", "data");

    env.but("commit -m 'my commit msg' -c A")
        .assert()
        .stderr_eq(snapbox::str![[""]])
        .stdout_eq(snapbox::str![[r#"
✓ Created commit 500ca64 on branch A

"#]])
        .success();
}

#[test]
fn committing_to_new_branch_with_same_name_as_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("foo", "data");

    // commit to a new branch with the same name as the file
    env.but("commit -m 'my commit msg' -c foo")
        .assert()
        .stderr_eq(snapbox::str![[""]])
        .stdout_eq(snapbox::str![[r#"
Created new independent branch 'foo'
✓ Created commit 5ee3c3d on branch foo

"#]])
        .success();
}

#[test]
fn committing_to_existing_branch_via_short_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    env.file("file.txt", "data");

    env.but("commit -m 'my commit msg' -c g0")
        .assert()
        .stderr_eq(snapbox::str![[""]])
        .stdout_eq(snapbox::str![[r#"
✓ Created commit [..] on branch A

"#]])
        .success();
}

#[test]
fn commit_batch_creates_multiple_selected_commits() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.file("one.txt", "one");
    env.file("two.txt", "two");

    let status = util::status_json(&env)?;
    let one_id = find_unassigned_cli_id(&status, "one.txt").expect("one.txt should have a CLI ID");
    let two_id = find_unassigned_cli_id(&status, "two.txt").expect("two.txt should have a CLI ID");

    env.but(format!(
        "commit batch A -m 'Add one' --changes {one_id} -m 'Add two' --changes {two_id}"
    ))
    .assert()
    .success()
    .stdout_eq(str![[r#"
✓ Created commit [..] on branch A
✓ Created commit [..] on branch A

"#]]);

    assert_eq!(unassigned_file_count(&env), 0);
    let messages = branch_commit_messages(&env, "A");
    let newest_messages = messages
        .iter()
        .take(2)
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(newest_messages, ["Add two", "Add one"]);
    assert_eq!(
        branch_commit_file_paths_by_message(&env, "A", "Add one"),
        ["one.txt"]
    );
    assert_eq!(
        branch_commit_file_paths_by_message(&env, "A", "Add two"),
        ["two.txt"]
    );

    Ok(())
}

#[test]
fn commit_batch_with_position_on_different_branch_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;
    env.file("new-file.txt", "content");

    let status = util::status_json(&env)?;
    let file_id =
        find_unassigned_cli_id(&status, "new-file.txt").expect("new-file.txt should have a CLI ID");

    let output = env
        .but(format!(
            "commit batch A --before d3e2ba3 -m 'Wrong target' --changes {file_id}"
        ))
        .assert()
        .failure();
    let stderr = std::str::from_utf8(&output.get_output().stderr)?;
    assert!(
        stderr.contains("Target must belong to the branch being committed to"),
        "unexpected stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn commit_batch_after_commit_preserves_batch_order() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.file("one.txt", "one");
    env.file("two.txt", "two");

    let status = util::status_json(&env)?;
    let one_id = find_unassigned_cli_id(&status, "one.txt").expect("one.txt should have a CLI ID");
    let two_id = find_unassigned_cli_id(&status, "two.txt").expect("two.txt should have a CLI ID");

    env.but(format!(
        "commit batch A --after 9477ae7 -m 'Add one' --changes {one_id} -m 'Add two' --changes {two_id}"
    ))
    .assert()
    .success();

    let messages = branch_commit_messages(&env, "A");
    let newest_messages = messages
        .iter()
        .take(3)
        .map(|message| message.trim_end())
        .collect::<Vec<_>>();
    assert_eq!(newest_messages, ["Add two", "Add one", "add A"]);

    Ok(())
}

#[test]
fn commit_batch_json_outputs_single_document() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.file("one.txt", "one");
    env.file("two.txt", "two");

    let status = util::status_json(&env)?;
    let one_id = find_unassigned_cli_id(&status, "one.txt").expect("one.txt should have a CLI ID");
    let two_id = find_unassigned_cli_id(&status, "two.txt").expect("two.txt should have a CLI ID");

    let output = env
        .but(format!(
            "commit --format json batch A -m 'Add one' --changes {one_id} -m 'Add two' --changes {two_id}"
        ))
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)?;

    assert_eq!(json["branch"], "A");
    let commits = json["commits"]
        .as_array()
        .expect("commits should be an array");
    assert_eq!(commits.len(), 2);
    assert!(commits[0]["commit_id"].is_string());
    assert!(commits[1]["commit_id"].is_string());
    assert!(json.get("branch_tip").is_none());

    Ok(())
}

#[test]
fn commit_batch_json_mode_multiple_branches_requires_branch() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata_at_target(&["A", "B"], "origin/main")?;

    env.file("new-file.txt", "test content");

    env.but("commit --format json batch -m 'Test commit' --changes zz:new-file.txt")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Multiple branches found. Specify a branch to commit to using the branch argument

"#]]);

    Ok(())
}

/// Helper to build an isolated `std::process::Command` for `but` with the same
/// environment as the Sandbox test harness.
/// That way it can be spawned, which isn't possible in the [`Sandbox`] version.
fn but_std_cmd(env: &Sandbox, args: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(snapbox::cmd::cargo_bin!("but"));
    cmd.args(shell_words::split(args).unwrap());
    cmd.current_dir(env.projects_root());
    cmd.env("E2E_TEST_APP_DATA_DIR", env.app_data_dir());
    cmd.env("GITBUTLER_CHANGE_ID", "42");
    cmd.env("NOPAGER", "1");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    but_testsupport::isolate_env_std_cmd(&mut cmd);
    cmd
}

/// Helper: find CLI IDs for unassigned files matching a path pattern.
fn find_unassigned_cli_id(status: &serde_json::Value, path_contains: &str) -> Option<String> {
    status["unassignedChanges"]
        .as_array()?
        .iter()
        .find(|c| {
            c["filePath"]
                .as_str()
                .map(|p| p.contains(path_contains))
                .unwrap_or(false)
        })
        .and_then(|c| c["cliId"].as_str().map(|s| s.to_string()))
}

/// Helper: parse `--format json status` and find a branch by name, returning its commit messages.
fn branch_commit_messages(env: &Sandbox, branch_name: &str) -> Vec<String> {
    let status = util::status_json(env).expect("status should be valid JSON");
    let branch = util::find_branch(&status, branch_name).expect("branch should exist in status");
    branch["commits"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|c| c["message"].as_str().map(|s| s.to_string()))
        .collect()
}

fn branch_commit_file_paths_by_message(
    env: &Sandbox,
    branch_name: &str,
    message: &str,
) -> Vec<String> {
    let status = util::status_json_with_files(env).expect("status should be valid JSON");
    let branch = util::find_branch(&status, branch_name).expect("branch should exist in status");
    let commits = branch["commits"]
        .as_array()
        .expect("branch commits should be an array");
    let commit = commits
        .iter()
        .find(|commit| commit["message"].as_str().map(str::trim_end) == Some(message))
        .expect("commit message should exist on branch");

    commit["changes"]
        .as_array()
        .expect("commit changes should be an array")
        .iter()
        .filter_map(|change| change["filePath"].as_str().map(ToOwned::to_owned))
        .collect()
}

/// Helper: count unassigned (uncommitted) changes in `--format json status`.
fn unassigned_file_count(env: &Sandbox) -> usize {
    util::status_json(env).expect("status should be valid JSON")["unassignedChanges"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0)
}

mod concurrent_commits {
    use bstr::ByteSlice;

    use super::*;

    fn commit_matching_change(
        env: &Sandbox,
        branch_name: &str,
        message: &str,
        path_contains: &str,
    ) -> anyhow::Result<()> {
        // Refresh status before each commit so this control test isolates
        // serialization from any possible CLI-ID churn across commands.
        let status = util::status_json(env)?;
        let cli_id = find_unassigned_cli_id(&status, path_contains)
            .expect("should find CLI ID for requested unassigned change");

        env.but(format!(
            "commit {branch_name} -m {message} --changes {cli_id}"
        ))
        .assert()
        .success();
        Ok(())
    }

    /// Concurrent commits to independent (parallel) branches should all succeed.
    ///
    /// This test creates three independent branches, adds a file to each, then
    /// fires three `but commit` processes simultaneously. All three should
    /// succeed without errors or lost data.
    #[test]
    fn concurrent_commits_to_independent_branches() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
        env.setup_metadata(&["A"])?;

        // Create two more independent branches
        env.but("branch new branchB").assert().success();
        env.but("branch new branchC").assert().success();

        // Add files for each branch
        env.file("src/a/new.ts", "export const a = true;");
        env.file("src/b/new.ts", "export const b = true;");
        env.file("src/c/new.ts", "export const c = true;");

        // Get file CLI IDs from status
        let status = util::status_json(&env)?;
        let id_a =
            find_unassigned_cli_id(&status, "a/new").expect("should find CLI ID for src/a/new.ts");
        let id_b =
            find_unassigned_cli_id(&status, "b/new").expect("should find CLI ID for src/b/new.ts");
        let id_c =
            find_unassigned_cli_id(&status, "c/new").expect("should find CLI ID for src/c/new.ts");

        // Fire three concurrent commits
        let child_a =
            but_std_cmd(&env, &format!("commit A -m commit-a --changes {id_a}")).spawn()?;
        let child_b = but_std_cmd(
            &env,
            &format!("commit branchB -m commit-b --changes {id_b}"),
        )
        .spawn()?;
        let child_c = but_std_cmd(
            &env,
            &format!("commit branchC -m commit-c --changes {id_c}"),
        )
        .spawn()?;

        let out_a = child_a.wait_with_output()?;
        let out_b = child_b.wait_with_output()?;
        let out_c = child_c.wait_with_output()?;

        // All should succeed
        assert!(
            out_a.status.success(),
            "commit to A failed: {}",
            out_a.stderr.as_bstr(),
        );
        assert!(
            out_b.status.success(),
            "commit to branchB failed: {}",
            out_b.stderr.as_bstr()
        );
        assert!(
            out_c.status.success(),
            "commit to branchC failed: {}",
            out_c.stderr.as_bstr()
        );

        // All files should be committed (not left unassigned)
        let remaining = unassigned_file_count(&env);
        assert_eq!(
            remaining, 0,
            "all files should be committed, but {remaining} are still unassigned"
        );

        // Each branch should have the new commit
        let a_msgs = branch_commit_messages(&env, "A");
        let b_msgs = branch_commit_messages(&env, "branchB");
        let c_msgs = branch_commit_messages(&env, "branchC");

        assert!(
            a_msgs.iter().any(|m| m.contains("commit-a")),
            "branch A should have commit-a, got: {a_msgs:?}"
        );
        assert!(
            b_msgs.iter().any(|m| m.contains("commit-b")),
            "branch branchB should have commit-b, got: {b_msgs:?}"
        );
        assert!(
            c_msgs.iter().any(|m| m.contains("commit-c")),
            "branch branchC should have commit-c, got: {c_msgs:?}"
        );

        Ok(())
    }

    /// The serial version of [`concurrent_commits_to_independent_branches`] to validate
    /// it can work if done in serial, if there is definitely no race.
    #[test]
    fn serialized_commits_to_independent_branches() -> anyhow::Result<()> {
        let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
        env.setup_metadata(&["A"])?;

        // Create two more independent branches
        env.but("branch new branchB").assert().success();
        env.but("branch new branchC").assert().success();

        // Add files for each branch
        env.file("src/a/new.ts", "export const a = true;");
        env.file("src/b/new.ts", "export const b = true;");
        env.file("src/c/new.ts", "export const c = true;");

        commit_matching_change(&env, "A", "commit-a", "a/new")?;
        commit_matching_change(&env, "branchB", "commit-b", "b/new")?;
        commit_matching_change(&env, "branchC", "commit-c", "c/new")?;

        let remaining = unassigned_file_count(&env);
        assert_eq!(
            remaining, 0,
            "all files should be committed, but {remaining} are still unassigned"
        );

        let a_msgs = branch_commit_messages(&env, "A");
        let b_msgs = branch_commit_messages(&env, "branchB");
        let c_msgs = branch_commit_messages(&env, "branchC");

        assert!(
            a_msgs.iter().any(|m| m.contains("commit-a")),
            "branch A should have commit-a, got: {a_msgs:?}"
        );
        assert!(
            b_msgs.iter().any(|m| m.contains("commit-b")),
            "branch branchB should have commit-b, got: {b_msgs:?}"
        );
        assert!(
            c_msgs.iter().any(|m| m.contains("commit-c")),
            "branch branchC should have commit-c, got: {c_msgs:?}"
        );

        Ok(())
    }
}
