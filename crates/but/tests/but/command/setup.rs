use serde_json::json;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn not_a_git_repository() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("setup")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No git repository found - run `but setup --init` to initialize a new repository.

"#]]);

    Ok(())
}

#[test]
fn no_remote_creates_gb_local() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Verify initial state - no remotes
    let output = env.invoke_git("remote");
    assert_eq!(output, "");

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    // Verify gb-local remote was created
    let output = env.invoke_git("remote");
    assert_eq!(output, "gb-local");

    // Verify remote HEAD was created
    let output = env.invoke_git("symbolic-ref refs/remotes/gb-local/HEAD");
    assert_eq!(output, "refs/remotes/gb-local/main");

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
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    // Verify gb-local remote was created with development branch
    let output = env.invoke_git("symbolic-ref refs/remotes/gb-local/HEAD");
    assert_eq!(output, "refs/remotes/gb-local/development");

    Ok(())
}

#[test]
fn remote_exists_but_no_remote_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head")?;

    // Verify remote exists but no HEAD
    let output = env.invoke_git("remote");
    assert_eq!(output, "origin");

    // Verify no remote HEAD exists initially
    env.invoke_git_fails(
        "symbolic-ref refs/remotes/origin/HEAD",
        "remote exists but has no HEAD initially",
    );

    // Run setup - should fail because there's no remote HEAD to discover
    env.but("setup")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    Ok(())
}

#[test]
fn remote_exists_with_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head")?;

    // Verify remote exists with HEAD
    let output = env.invoke_git("remote");
    assert_eq!(output, "origin");

    env.invoke_git("symbolic-ref refs/remotes/origin/HEAD");

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


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
  ✓ Repository already in project registry

GitButler project is already set up!
Target branch: origin/main



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


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
  "projectStatus": "alreadyexists",
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
  "projectStatus": "alreadyexists",
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
  "projectStatus": "alreadyexists",
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
  "projectStatus": "alreadyexists",
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
  "projectStatus": "alreadyexists",
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
Error: No git repository found.

"#]])
        .stdout_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn init_flag_creates_repo() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Verify no git repo exists
    env.invoke_git_fails(
        "rev-parse --git-dir",
        "empty sandbox should not contain a git repository",
    );

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    // Verify git repo was created
    let output = env.invoke_git("rev-parse --git-dir");
    assert!(!output.is_empty());

    // Verify initial commit was created (may have additional workspace commit)
    let commit_count: u32 = env.invoke_git("rev-list --count HEAD").parse()?;
    assert!(
        commit_count >= 1,
        "Expected at least 1 commit, found {commit_count}"
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
    let output = env.invoke_git("rev-parse --git-dir");
    assert!(!output.is_empty());

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
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    Ok(())
}

#[test]
fn setup_called_on_unmigrated_projects_json() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Run first to create the metadata in `projects.json` which we then mutate
    // to create the "legacy" metadata scenario.
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

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



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    let projects_file = env.app_data_dir().join("com.gitbutler.app/projects.json");
    let mut file: serde_json::Value = std::fs::read_to_string(&projects_file)?.parse()?;

    file.as_array_mut().unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("git_dir".into(), json!(""));

    std::fs::write(projects_file, serde_json::to_string_pretty(&file)?)?;

    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

GitButler project is already set up!
Target branch: gb-local/main



 ██████▄      ▄██████    ██████╗ ██╗   ██╗████████╗
 ██▀▀▀▀██▄  ▄██▀▀▀▀██    ██╔══██╗██║   ██║╚══██╔══╝
 ██     ▀████▀     ██    ██████╔╝██║   ██║   ██║
 ██▄▄▄▄██▀  ▀██▄▄▄▄██    ██╔══██╗██║   ██║   ██║
 ██████▀      ▀██████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    Ok(())
}
