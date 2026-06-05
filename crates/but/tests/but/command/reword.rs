use gitbutler_commit::commit_ext::CommitMessageBstr as _;
use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn reword_commit_with_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use reword with -m flag to change commit message (using commit ID)
    env.but("reword 9477ae7 -m 'Updated commit message'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    insta::assert_snapshot!(env.git_log()?, @"
    * 8c69cf9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2f7c570 (A) Updated commit message
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}

#[test]
fn reword_commit_with_multiline_message() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use reword with multiline message
    env.but("reword 9477ae7 -m 'First line\n\n\tSecond paragraph with details'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..] (now [..])

"#]]);

    // Verify the commit message was updated with multiline content
    insta::assert_snapshot!(env.git_log()?, @"
    * e6bde18 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
fn reword_branch_from_editor_trims_trailing_newlines_in_confirmation_output() -> anyhow::Result<()>
{
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;

    env.setup_metadata(&["A"])?;
    env.but("branch new branch-to-rename-123")
        .assert()
        .success();

    env.file(
        "editor.sh",
        "#!/usr/bin/env bash\nprintf 'renamed-branch\\n\\n' > \"$1\"\n",
    );
    env.but("reword branch-to-rename-123")
        .env("GIT_EDITOR", "bash editor.sh")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Renamed branch 'branch-to-rename-123' to 'renamed-branch'

"#]]);

    Ok(())
}

#[test]
fn reword_branch_rejects_head() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new branch-to-rename").assert().success();

    env.but("reword branch-to-rename -m HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);

    Ok(())
}

#[test]
fn reword_branch_rejects_non_normalized_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("reword A -m /B")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Bad input '/B'

Invalid branch name

Hint: Try 'B' instead

"#]]);

    Ok(())
}

#[test]
fn reword_branch_rejects_branch_name_that_already_exists() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("branch new existing").assert().success();

    env.but("reword A -m existing")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: A branch named 'existing' is already applied

"#]]);

    Ok(())
}

#[test]
fn reword_branch_allows_rewording_to_same_name() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;

    env.but("reword A -m A")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Branch already named 'A' - nothing to do

"#]])
        .stderr_eq(str![[]]);

    Ok(())
}

#[test]
fn reword_commit_with_same_message_succeeds_as_noop() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Try to reword with the same message
    env.but("reword 9477ae7 -m 'add A'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
No changes to commit message - nothing to be done

"#]]);

    Ok(())
}

#[test]
fn reword_commit_with_json_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    env.setup_metadata(&["A"])?;

    // Use reword with -m flag to change commit message (using commit ID)
    env.but("reword 9477ae7 -m 'Updated commit message' --format json")
        .assert()
        .success()
        .stdout_eq(str![[r#"{
  "new_commit_id": [..]
}

"#]]);

    insta::assert_snapshot!(env.git_log()?, @"
    * 8c69cf9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 2f7c570 (A) Updated commit message
    * 0dc3733 (origin/main, origin/HEAD, main, gitbutler/target) add M
    ");

    Ok(())
}
