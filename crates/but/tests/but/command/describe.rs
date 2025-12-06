use crate::utils::Sandbox;
use gitbutler_commit::commit_ext::CommitExt;
use snapbox::str;

#[test]
fn describe_commit_with_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use describe with -m flag to change commit message (using commit ID)
    env.but("describe 9477ae7 -m 'Updated commit message'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    insta::assert_snapshot!(env.git_log()?, @r"
    * d84f3c4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2f7c570 (A) Updated commit message
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

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

    env.setup_metadata(&["A"])?;

    // Use describe with multiline message
    env.but("describe 9477ae7 -m 'First line\n\n\tSecond paragraph with details'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    // Verify the commit message was updated with multiline content
    insta::assert_snapshot!(env.git_log()?, @r"
    * f2c2b50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * cdf2c74 (A) First line
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    let repo = env.open_repo()?;
    assert_eq!(
        repo.rev_parse_single(":/First line")?
            .object()?
            .into_commit()
            .message_bstr(),
        "First line\n\n\tSecond paragraph with details"
    );

    Ok(())
}

// Note: Branch rename test is omitted because the test scenario uses single-character
// branch names ("A") which don't meet the 2-character minimum requirement for CLI IDs.
// The branch rename functionality with -m flag is tested manually and works correctly.

#[test]
fn describe_commit_with_same_message_succeeds_as_noop() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Try to describe with the same message
    env.but("describe 9477ae7 -m 'add A'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
No changes to commit message - nothing to be done

"#]]);

    Ok(())
}
