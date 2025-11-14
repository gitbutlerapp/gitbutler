use snapbox::{file, str};

use crate::utils::{Sandbox, setup_metadata};

#[test]
fn nice_help() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    env.but(None)
        .assert()
        .success()
        .stdout_eq(file!["snapshots/no-arg.stdout.term.svg"]);

    env.but("-h")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/no-arg.stdout.term.svg"]);

    env.but("--help")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/no-arg.stdout.term.svg"]);

    // The help should be nice, as it's a complex command.
    env.but("rub --help")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/rub-long-help.stdout.term.svg"]);
    env.but("rub -h")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/rub-short-help.stdout.term.svg"]);
    Ok(())
}

#[test]
fn from_scratch_needs_work() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("status").assert().failure().stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    // Init doesn't work without a Git repository
    env.but("init")
        .assert()
        .failure()
        .stderr_eq(str![
            r#"
Error: Failed to initialize GitButler project.

Caused by:
    0: You can run `but init --repo` to initialize a new Git repository
    1: "." does not appear to be a git repository
    2: Missing HEAD at '.git/HEAD'

"#
        ])
        .stdout_eq(str![]);

    // TODO: this should work, but we still have requirements and can't deal with any repo.
    env.but("init --repo")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Failed to initialize GitButler project.

Caused by:
    No push remote set

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

        // Doing it again also doesn't work, we need a proper remote.
        env.but("init --repo")
            .assert()
            .failure()
            .stdout_eq(str![])
            .stderr_eq(str![[r#"
Error: Failed to initialize GitButler project.

Caused by:
    No HEAD reference found for remote origin

"#]]);
    }

    // Status really wants the target, still.
    env.but("status")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: errors.projects.default_target.not_found

Caused by:
    there is no default target

"#]]);

    // Single branch mode, does it work?
    // TODO: make this work to get further into a typical workflow.
    env.but("branch new feat")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: workspace at refs/heads/main is missing a base

"#]]);

    Ok(())
}

#[test]
fn from_workspace() -> anyhow::Result<()> {
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
    setup_metadata(&env, &["A", "B"])?;

    env.but("status")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/status01.stdout.term.svg"]);

    env.but("status -v").assert().success().stdout_eq(file![
        "snapshots/from-workspace/status01-verbose.stdout.term.svg"
    ]);

    // List is the default
    env.but("branch")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // But list is also explicit.
    env.but("branch list")
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // TODO: more operations on the repository!
    Ok(())
}

#[test]
fn json_flag_can_be_placed_before_or_after_subcommand() -> anyhow::Result<()> {
    // TODO: use an actual repository here, but single-branch mode isn't really supported yet
    //       so everything fails anyway.
    let env = Sandbox::empty()?;

    // Test that --json flag works in both positions with help command (doesn't need a valid repo)
    env.but("--json completions --help").assert().success();

    env.but("completions --help --json").assert().success();

    // Test with actual commands that need a repo (they'll fail but should accept the flag)
    // Before subcommand
    env.but("--json status")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    // After subcommand
    env.but("status --json")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    // Test with log command as well
    // Before subcommand
    env.but("--json log")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    // After subcommand
    env.but("log --json")
        .env_remove("BUT_OUTPUT_FORMAT")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    Ok(())
}
