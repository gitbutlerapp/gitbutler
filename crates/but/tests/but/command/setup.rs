use serde_json::json;

use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn not_a_git_repository() {
    let env = Sandbox::empty();

    env.but("setup")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No git repository found - run `but setup --init` to initialize a new repository.

"#]]);
}

#[test]
fn no_remote_creates_gb_local() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

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
}

#[test]
fn no_remote_with_non_standard_branch() {
    let env = Sandbox::open_with_default_settings("repo-no-remote-no-main-or-master");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

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
}

#[test]
fn remote_exists_but_no_remote_head() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

#[test]
fn remote_exists_with_head() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

#[test]
fn already_setup() {
    let env = Sandbox::open_with_default_settings("repo-already-setup");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

#[test]
fn json_output_new_setup() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");

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
}

#[test]
fn json_output_already_setup() {
    let env = Sandbox::open_with_default_settings("repo-already-setup");

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
}

#[test]
fn json_output_gb_local() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

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
}

#[test]
fn json_output_non_standard_branch() {
    let env = Sandbox::open_with_default_settings("repo-no-remote-no-main-or-master");

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
}

#[test]
fn json_output_remote_no_head_fallback() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head");

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
}

#[test]
fn json_output_not_a_git_repo() {
    let env = Sandbox::empty();

    env.but("--json setup")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No git repository found - run `but setup --init` to initialize a new repository.

"#]])
        .stdout_eq(snapbox::str![]);
}

#[test]
fn init_flag_creates_repo() {
    let env = Sandbox::empty();

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

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
    let commit_count: u32 = env.invoke_git("rev-list --count HEAD").parse().unwrap();
    assert!(
        commit_count >= 1,
        "Expected at least 1 commit, found {commit_count}"
    );
}

#[test]
fn init_flag_json_output() {
    let env = Sandbox::empty();

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
}

#[test]
fn init_flag_with_existing_repo() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

#[test]
fn setup_called_on_unmigrated_projects_json() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);

    let projects_file = env.app_data_dir().join("com.gitbutler.app/projects.json");
    let mut file: serde_json::Value = std::fs::read_to_string(&projects_file)
        .unwrap()
        .parse()
        .unwrap();

    file.as_array_mut().unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("git_dir".into(), json!(""));

    std::fs::write(projects_file, serde_json::to_string_pretty(&file).unwrap()).unwrap();

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



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

/// When the user opted out of hooks via `gitbutler.installHooks=false`, setup
/// must say hook installation was skipped instead of claiming hooks are being
/// installed while direct commits to the workspace branch go unguarded.
#[test]
fn setup_with_hooks_opted_out_says_so() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
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

  Note: Skipped Git hook installation (gitbutler.installHooks=false). Commits made directly to the workspace branch will not be blocked.

Setting up your project for GitButler tooling. Some things to note:

- Switching you to a special `gitbutler/workspace` branch to enable parallel branches
- Skipping Git hooks (disabled via gitbutler.installHooks)

To undo these changes and return to normal Git mode, either:

    - Directly checkout a branch (`git checkout main`)
    - Run `but teardown`

More info: https://docs.gitbutler.com/workspace-branch



██▄      ▄██  ▀██▀▀█▄ ▀██▀ ▀██▀ █▀▀██▀▀█
████▄  ▄████   ██  ██  ██   ██  ▀  ██  ▀
████████████   ██▀▀█▄  ██   ██     ██
████▀  ▀████   ██  ██  ██   ██     ██
██▀      ▀██  ▄██▄▄█▀  ▀█▄▄▄█▀   ▄▄██▄▄

The command-line interface for GitButler ⋈

$ but branch new <name>                       Create a new branch
$ but status                                  View workspace status
$ but commit -m <message>                     Commit changes to current branch
$ but push                                    Push all branches
$ but teardown                                Return to normal Git mode

Learn more at https://docs.gitbutler.com/cli-overview


"#]]);
}

/// The opt-out note above only reaches humans. A JSON consumer that cannot see
/// it would proceed as if the workspace were guarded, so the skip has to be
/// visible in the JSON output too.
#[test]
fn json_output_reports_hooks_skipped() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    env.invoke_git("config --local gitbutler.installHooks false");

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
  "hooksSkipped": true
}

"#]]);
}

/// Hooks install cleanly here, so neither new field should appear -- existing
/// JSON consumers must see exactly the payload they saw before.
#[test]
fn json_output_omits_hook_fields_when_install_succeeds() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");

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
}
