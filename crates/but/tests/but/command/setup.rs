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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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
  },
  "hookManager": null
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
  },
  "hookManager": null
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
  },
  "hookManager": null
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
  },
  "hookManager": null
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
  },
  "hookManager": null
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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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
  },
  "hookManager": null
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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

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
fn no_hooks_flag_skips_hook_installation() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("setup --no-hooks")
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

  Skipping hook installation (--no-hooks)

Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should not be installed when --no-hooks is used"
    );
    assert!(
        !env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should not be installed when --no-hooks is used"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should not be installed when --no-hooks is used"
    );

    Ok(())
}

#[test]
fn persisted_no_hooks_config_skips_hook_installation() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.invoke_git("config --local gitbutler.installHooks false");

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

  Skipping hook installation (--no-hooks is configured for this repository)
  To switch back to GitButler-managed hooks: but setup --force-hooks

Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    assert!(
        !env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should stay absent when the repo-local opt-out is already configured"
    );
    assert!(
        !env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should stay absent when the repo-local opt-out is already configured"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should stay absent when the repo-local opt-out is already configured"
    );

    Ok(())
}

#[test]
fn no_hooks_switches_repo_to_external_mode_and_removes_managed_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    assert!(
        env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be installed after the initial managed-mode setup"
    );
    assert!(
        env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should be installed after the initial managed-mode setup"
    );
    assert!(
        env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should be installed after the initial managed-mode setup"
    );

    env.but("setup --no-hooks")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository already in project registry

GitButler project is already set up!
Target branch: gb-local/main

  Skipping hook installation (--no-hooks)


 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be removed when switching to externally-managed hook mode"
    );
    assert!(
        !env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should be removed when switching to externally-managed hook mode"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should be removed when switching to externally-managed hook mode"
    );

    Ok(())
}

#[test]
fn json_output_with_prek_hook_manager_detected() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Install a prek-managed hook before running setup
    env.invoke_bash(
        "mkdir -p .git/hooks && \
         printf '#!/bin/sh\\n# File generated by prek\\nexec prek hook-impl pre-commit\\n' > .git/hooks/pre-commit && \
         chmod +x .git/hooks/pre-commit",
    );

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
  },
  "hookManager": {
    "name": "prek",
    "hooksInstalled": false
  }
}

"#]]);

    Ok(())
}

#[test]
fn setup_detects_prek_managed_hooks_and_prints_instructions() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Install a prek-managed hook before running setup
    env.invoke_bash(
        "mkdir -p .git/hooks && \
         printf '#!/bin/sh\\n# File generated by prek\\nexec prek hook-impl pre-commit\\n' > .git/hooks/pre-commit && \
         chmod +x .git/hooks/pre-commit",
    );

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


  Detected prek managing your git hooks.
  GitButler will not overwrite hooks owned by your hook manager.
  This repository is now configured for externally-managed hooks.

  To integrate GitButler's workspace guard with your hook manager:
  Add the following to your prek.toml to integrate GitButler's workspace guard:
  
    [[repos]]
    repo = "local"
    hooks = [
        { id = "gitbutler-workspace-guard",
          name = "GitButler Workspace Guard",
          language = "system",
          entry = "but hook pre-commit",
          pass_filenames = false,
          always_run = true,
          stages = ["pre-commit"] },
        { id = "gitbutler-post-checkout",
          name = "GitButler Post-Checkout Cleanup",
          language = "system",
          entry = "but hook post-checkout",
          pass_filenames = false,
          always_run = true,
          stages = ["post-checkout"] },
        { id = "gitbutler-push-guard",
          name = "GitButler Push Guard",
          language = "system",
          entry = "but hook pre-push",
          pass_filenames = false,
          always_run = true,
          stages = ["pre-push"] },
    ]

  To switch back to GitButler-managed hooks: but setup --force-hooks


Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    // Verify the prek hook was NOT overwritten
    let hook_content = std::fs::read_to_string(env.projects_root().join(".git/hooks/pre-commit"))?;
    assert!(
        hook_content.contains("File generated by prek"),
        "Prek hook should be preserved, got: {hook_content}"
    );

    // Verify no backup was created (GitButler did not touch the hook)
    assert!(
        !env.projects_root()
            .join(".git/hooks/pre-commit-user")
            .exists(),
        "No backup should be created for externally managed hooks"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );

    Ok(())
}

#[test]
fn force_hooks_overrides_prek_detection_and_installs_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.invoke_git("config --local gitbutler.installHooks false");

    // Install a prek-managed hook before running setup
    env.invoke_bash(
        "mkdir -p .git/hooks && \
         printf '#!/bin/sh\\n# File generated by prek\\nexec prek hook-impl pre-commit\\n' > .git/hooks/pre-commit && \
         chmod +x .git/hooks/pre-commit",
    );

    env.but("setup --force-hooks")
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

  Forcing hook installation (--force-hooks), skipping detection.

Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Installing Git hooks to help manage commits on the workspace branch

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch



 █████      █████    ██████╗ ██╗   ██╗████████╗
   █████  █████      ██╔══██╗██║   ██║╚══██╔══╝
     ████████        ██████╔╝██║   ██║   ██║
   █████  █████      ██╔══██╗██║   ██║   ██║
 █████      █████    ██████╔╝╚██████╔╝   ██║

The command-line interface for GitButler

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    // Verify the prek hook WAS overwritten with GitButler's hook
    let hook_content = std::fs::read_to_string(env.projects_root().join(".git/hooks/pre-commit"))?;
    assert!(
        hook_content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "GitButler hook should be installed after --force-hooks, got: {hook_content}"
    );

    // Verify the original prek hook was backed up
    let backup_path = env.projects_root().join(".git/hooks/pre-commit-user");
    assert!(
        backup_path.exists(),
        "Original prek hook should be backed up to pre-commit-user"
    );
    let backup_content = std::fs::read_to_string(&backup_path)?;
    assert!(
        backup_content.contains("File generated by prek"),
        "Backup should contain original prek hook, got: {backup_content}"
    );
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "true"
    );

    Ok(())
}

#[test]
fn rerun_setup_after_manager_removed_stays_external_with_hint() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // First setup with prek detected → externally-managed mode
    env.invoke_bash(
        "mkdir -p .git/hooks && \
         printf '#!/bin/sh\\n# File generated by prek\\nexec prek hook-impl pre-commit\\n' > .git/hooks/pre-commit && \
         chmod +x .git/hooks/pre-commit",
    );
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );

    // Remove prek (delete hook + no config file)
    env.invoke_bash("rm -f .git/hooks/pre-commit");

    // Re-run setup without --force-hooks → should stay in externally-managed mode
    // and print a hint about how to switch back
    env.but("setup")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
  To switch back to GitButler-managed hooks: but setup --force-hooks
...
"#]]);

    // Should still be in externally-managed mode
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );

    Ok(())
}

#[test]
fn toggle_no_hooks_then_force_hooks_roundtrip() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Step 1: Setup → managed mode, hooks installed
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert!(
        env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be installed in managed mode"
    );
    assert!(
        env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should be installed in managed mode"
    );
    assert!(
        env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should be installed in managed mode"
    );

    // Step 2: Setup --no-hooks → externally-managed, hooks removed
    env.but("setup --no-hooks")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );
    assert!(
        !env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be removed after --no-hooks"
    );

    // Step 3: Setup --force-hooks → managed again, hooks reinstalled
    env.but("setup --force-hooks")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "true"
    );
    let hook_content = std::fs::read_to_string(env.projects_root().join(".git/hooks/pre-commit"))?;
    assert!(
        hook_content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "pre-commit should be reinstalled with current content after --force-hooks"
    );
    assert!(
        env.projects_root()
            .join(".git/hooks/post-checkout")
            .exists(),
        "post-checkout should be reinstalled after --force-hooks"
    );
    assert!(
        env.projects_root().join(".git/hooks/pre-push").exists(),
        "pre-push should be reinstalled after --force-hooks"
    );

    Ok(())
}

#[test]
fn core_hooks_path_change_warns_about_orphaned_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // First setup → managed mode, hooks installed to .git/hooks/
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert!(
        env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be installed in .git/hooks/"
    );

    // Set core.hooksPath to a custom directory
    env.invoke_bash("mkdir -p .git/custom-hooks");
    env.invoke_git("config --local core.hooksPath .git/custom-hooks");

    // Re-run setup → should warn about orphaned hooks in .git/hooks/
    env.but("setup")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
  Warning: GitButler-managed hooks found in old hooks directory ([..]).
...
"#]]);

    // Hooks should be installed in the new directory
    assert!(
        env.projects_root()
            .join(".git/custom-hooks/pre-commit")
            .exists(),
        "pre-commit should be installed in custom-hooks/"
    );

    Ok(())
}

#[test]
fn rerun_setup_detects_new_hook_manager_and_transitions_to_external() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // First setup → managed mode
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);
    assert!(
        env.projects_root().join(".git/hooks/pre-commit").exists(),
        "pre-commit should be installed after initial managed-mode setup"
    );

    // Simulate prek being installed between setups
    env.invoke_bash(
        "printf '#!/bin/sh\\n# File generated by prek\\nexec prek hook-impl pre-commit\\n' > .git/hooks/pre-commit && \
         chmod +x .git/hooks/pre-commit",
    );

    // Re-run setup → should detect prek and transition to externally-managed
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // GB hooks should be removed, prek hook preserved
    let hook_content = std::fs::read_to_string(env.projects_root().join(".git/hooks/pre-commit"))?;
    assert!(
        hook_content.contains("File generated by prek"),
        "Prek hook should be preserved after transition, got: {hook_content}"
    );

    // Config should switch to externally-managed
    assert_eq!(
        env.invoke_git("config --local --get gitbutler.installHooks"),
        "false"
    );

    Ok(())
}
