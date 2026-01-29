use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn not_a_git_repository() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("setup")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Failed to set up GitButler project.

Caused by:
    No git repository found - run `but setup --init` to initialize a new repository.

"#]]);

    Ok(())
}

#[test]
fn no_remote_creates_gb_local() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Verify initial state - no remotes
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking main
  ✓ Set default target to: gb-local/main

GitButler project setup complete!
Target branch: gb-local/main
Remote: gb-local


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    // Verify gb-local remote was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "gb-local");

    // Verify remote HEAD was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/gb-local/HEAD")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "refs/remotes/gb-local/main"
    );

    Ok(())
}

#[test]
fn no_remote_with_non_standard_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote-no-main-or-master")?;

    // Run setup - should use the current branch name (development)
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking development
  ✓ Set default target to: gb-local/development

GitButler project setup complete!
Target branch: gb-local/development
Remote: gb-local


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout development`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    // Verify gb-local remote was created with development branch
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/gb-local/HEAD")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "refs/remotes/gb-local/development"
    );

    Ok(())
}

#[test]
fn remote_exists_but_no_remote_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head")?;

    // Verify remote exists but no HEAD
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    // Verify no remote HEAD exists initially
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output()?;
    assert!(!output.status.success());

    // Run setup - should fail because there's no remote HEAD to discover
    env.but("setup")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  ✓ Using existing push remote: origin
  ✓ No remote HEAD found, using origin/main
  ✓ Set default target to: origin/main

GitButler project setup complete!
Target branch: origin/main
Remote: origin


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    Ok(())
}

#[test]
fn remote_exists_with_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head")?;

    // Verify remote exists with HEAD
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output()?;
    assert!(output.status.success());

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  ✓ Using existing push remote: origin
  ✓ Set default target to: origin/main

GitButler project setup complete!
Target branch: origin/main
Remote: origin


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    Ok(())
}

#[test]
fn already_setup() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-already-setup")?;

    // Run setup once to initialize
    env.but("setup").assert().success();

    // Run setup again - should recognize it's already set up
    // Note: The project gets re-added because Sandbox.empty() creates fresh temp dirs each time,
    // but the target is detected as already configured
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

GitButler project is already set up!
Target branch: origin/main


"#]]);

    Ok(())
}

#[test]
fn json_output_new_setup() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head")?;

    env.but("--json setup")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "origin/main",
    "remoteName": "origin",
    "newlySet": true
  }
}

"#]]);

    Ok(())
}

#[test]
fn json_output_already_setup() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-already-setup")?;

    // Run setup once to initialize
    env.but("setup").assert().success();

    // Run again with JSON output
    env.but("--json setup")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "origin/main",
    "remoteName": "origin",
    "newlySet": false
  }
}

"#]]);

    Ok(())
}

#[test]
fn json_output_gb_local() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("--json setup")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "gb-local/main",
    "remoteName": "gb-local",
    "newlySet": true
  }
}

"#]]);

    Ok(())
}

#[test]
fn json_output_non_standard_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote-no-main-or-master")?;

    env.but("--json setup")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "gb-local/development",
    "remoteName": "gb-local",
    "newlySet": true
  }
}

"#]]);

    Ok(())
}

#[test]
fn json_output_remote_no_head_fallback() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head")?;

    env.but("--json setup")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "origin/main",
    "remoteName": "origin",
    "newlySet": true
  }
}

"#]]);

    Ok(())
}

#[test]
fn json_output_not_a_git_repo() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("--json setup")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Failed to set up GitButler project.

Caused by:
    No git repository found.

"#]])
        .stdout_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn init_flag_creates_repo() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Verify no git repo exists
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--git-dir")
        .output()?;
    assert!(!output.status.success());

    env.but("setup --init")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
No git repository found. Initializing new repository...
✓ Initialized repository with empty commit

Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking main
  ✓ Set default target to: gb-local/main

GitButler project setup complete!
Target branch: gb-local/main
Remote: gb-local


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    // Verify git repo was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--git-dir")
        .output()?;
    assert!(output.status.success());

    // Verify initial commit was created (may have additional workspace commit)
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-list")
        .arg("--count")
        .arg("HEAD")
        .output()?;
    assert!(output.status.success());
    let commit_count: u32 = String::from_utf8_lossy(&output.stdout).trim().parse()?;
    assert!(
        commit_count >= 1,
        "Expected at least 1 commit, found {}",
        commit_count
    );

    Ok(())
}

#[test]
fn init_flag_json_output() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("--json setup --init")
        .allow_json()
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
{
  "repositoryPath": "[..]",
  "projectStatus": "added",
  "target": {
    "branchName": "gb-local/main",
    "remoteName": "gb-local",
    "newlySet": true
  }
}

"#]]);

    // Verify git repo was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--git-dir")
        .output()?;
    assert!(output.status.success());

    Ok(())
}

#[test]
fn init_flag_with_existing_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Should work the same as without --init when repo exists
    env.but("setup --init")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking main
  ✓ Set default target to: gb-local/main

GitButler project setup complete!
Target branch: gb-local/main
Remote: gb-local


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    Ok(())
}

#[test]
fn setup_after_teardown_and_new_commit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    env.setup_metadata(&["A", "B"])?;

    // Run teardown
    env.but("teardown").assert().success();

    // Create a new commit on branch A, and verify that the commit graph is what
    // we expect
    but_testsupport::git(&env.open_repo()?)
        .env("GIT_AUTHOR_DATE", "1675176957 +0100")
        .env("GIT_COMMITTER_DATE", "1675176957 +0100")
        .arg("commit")
        .arg("--allow-empty")
        .arg("-m")
        .arg("New commit on branch A")
        .output()?;
    insta::assert_snapshot!(env.git_log()?, @r"
    * 30cd751 (HEAD -> A) New commit on branch A
    * 9477ae7 add A
    | * d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");

    // Verify that setup works
    // TODO it shouldn't say "already set up", since some setup tasks needed to
    // be done (creating the gitbutler/workspace branch etc.)
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

GitButler project is already set up!
Target branch: origin/main


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout A`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch                    


"#]]);

    env.but("status")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┊╭┄g0 [A]  
┊●   30cd751 New commit on branch A (no changes)  
┊●   9477ae7 add A  
├╯
┊
┊╭┄h0 [B]  
┊●   d3e2ba3 add B  
├╯
┊
┴ 0dc3733 (common base) [origin/main] 2000-01-02 add M 

Hint: run `but help` for all commands

"#]]);

    Ok(())
}
