use crate::utils::{Sandbox, setup_metadata};
use snapbox::str;

#[test]
fn commit_with_message_flag() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("one-stack")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * edd3eb7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 9477ae7 (A) add A
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A"])?;

    // Create a change in the worktree
    env.file("new-file.txt", "test content");

    // Commit with message flag
    env.but("commit -m 'Add new file'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created commit [..] on branch A

"#]]);

    // Verify the commit was created
    let log = env.git_log()?;
    assert!(log.contains("Add new file"));

    Ok(())
}

#[test]
fn commit_with_branch_hint() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A", "B"])?;

    // Create a change
    env.file("file-for-b.txt", "content for B");

    // Commit to specific branch
    env.but("commit -m 'Change for B' B")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created commit [..] on branch B

"#]]);

    let log = env.git_log()?;
    assert!(log.contains("Change for B"));

    Ok(())
}

#[test]
fn commit_with_nonexistent_branch_fails() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A", "B"])?;

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
    let env = Sandbox::init_scenario_with_target("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    setup_metadata(&env, &["A", "B"])?;

    env.file("new-feature.txt", "new feature");

    env.but("commit -m 'New feature' -c feature-x")
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(str![[r#"
Created new independent branch 'feature-x'
Created commit [..] on branch feature-x

"#]]);

    env.but("oplog")
        .with_assert(env.assert_with_oplog_redactions())
        .assert()
        .success()
        .stderr_eq(str![])
        .stdout_eq(snapbox::file![
            "snapshots/from-workspace/commit-oplog.stdout.term.svg"
        ]);
    Ok(())
}
