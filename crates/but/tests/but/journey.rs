//! Tests for various nice user-journeys, from different starting points, performing multiple common steps in sequence.
use snapbox::str;

use crate::utils::Sandbox;

#[cfg(not(feature = "legacy"))]
#[test]
fn from_unborn() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("unborn")?;
    insta::assert_snapshot!(env.git_log()?, @r"");

    env.but("branch apply main").assert().failure().stderr_eq(str![[r#"
Error: The reference 'main' did not exist

"#]]);

    // TODO: we should be able to use the CLI to create a commit
    Ok(())
}

// TODO: maybe this should be a non-legacy journey only as we start out without workspace?
#[cfg(feature = "legacy")]
#[test]
fn from_empty() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("status").assert().failure().stderr_eq(str![[r#"
Error: No git repository found at .
Please run 'but setup' to initialize the project.

"#]]);

    // Setup doesn't work without a Git repository
    env.but("setup").assert().failure().stderr_eq(str![[r#"
Error: No git repository found - run `but setup --init` to initialize a new repository.

"#]]);

    // TODO: this should work, but we still have requirements and can't deal with any repo.
    env.but("setup --init").assert().success().stdout_eq(str![[r#"
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

    // Forcefully add fake remote
    {
        env.append_file(
            ".git/config",
            r#"
    [remote "origin"]
        url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
        fetch = +refs/heads/*:refs/remotes/origin/*
    "#,
        );

        env.but("setup")
            .assert()
            .success()
            .stderr_eq(str![])
            .stdout_eq(str![[r#"
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
    }

    env.but("status")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄zz [unstaged changes] 
┊     no changes
┊
┴ 6f66116 [gb-local/main] 2000-01-02 Initial empty commit

Hint: run `but branch new` to create a new branch to work on

"#]])
        .stderr_eq(str![""]);

    Ok(())
}

#[cfg(feature = "legacy")]
#[test]
fn from_workspace() -> anyhow::Result<()> {
    use snapbox::file;

    use crate::utils::CommandExt;
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks")?;
    insta::assert_snapshot!(env.git_log()?, @r"
    *   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9477ae7 (A) add A
    * | d3e2ba3 (B) add B
    |/  
    * 0dc3733 (origin/main, origin/HEAD, main) add M
    ");
    insta::assert_snapshot!(env.git_status()?, @r"");

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"])?;

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/status01.stdout.term.svg"]);

    env.but("status -v")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/status01-verbose.stdout.term.svg"]);

    // List is the default
    env.but("branch")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // But list is also explicit.
    env.but("branch list")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // TODO: more operations on the repository!
    Ok(())
}
