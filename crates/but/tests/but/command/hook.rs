use crate::utils::Sandbox;

#[test]
fn pre_commit_help_shows_usage() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("hook pre-commit --help")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Workspace guard for pre-commit hooks.

Blocks direct `git commit` on the `gitbutler/workspace` branch with a
helpful error message directing the user to use `but commit` instead.
Exits 0 (allow) on any other branch.

This command is designed to be called from a hook manager configuration:

## Examples

prek.toml:

```text
[[repos]]
repo = "local"
hooks = [{ id = "gitbutler-workspace-guard", language = "system", entry = "but hook pre-commit" }]
```

lefthook.yml:

```text
pre-commit:
  commands:
    gitbutler-guard:
      run: but hook pre-commit
```

Usage: but hook pre-commit [OPTIONS]

Options:
  -j, --json
          Whether to use JSON output format

      --status-after
          After a mutation command completes, also output workspace status.
          
          In human mode, prints status after the command output. In JSON mode, wraps both in
          {"result": ..., "status": ...} on success, or {"result": ..., "status_error": ...} if the
          status query fails (in which case "status" is absent).

  -h, --help
          Print help (see a summary with '-h')

"#]]);

    Ok(())
}

#[test]
fn post_checkout_help_shows_usage() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("hook post-checkout --help")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Informational hook for post-checkout events.

When leaving the `gitbutler/workspace` branch, prints an informational
message noting you have left GitButler mode and directing you to run
`but setup` to return.

Accepts the standard post-checkout arguments (prev_head, new_head, is_branch_checkout)
as positional parameters, matching git's post-checkout hook signature.

## Examples

prek.toml:

```text
[[repos]]
repo = "local"
hooks = [{ id = "gitbutler-post-checkout", language = "system", entry = "but hook post-checkout" }]
```

Usage: but hook post-checkout [OPTIONS] [PREV_HEAD] [NEW_HEAD] [IS_BRANCH_CHECKOUT]

Arguments:
  [PREV_HEAD]
          The ref of the previous HEAD (provided by git)
          
          [default: ""]

  [NEW_HEAD]
          The ref of the new HEAD (provided by git)
          
          [default: ""]

  [IS_BRANCH_CHECKOUT]
          Whether this is a branch checkout (1) or a file checkout (0)
          
          [default: 1]

Options:
  -j, --json
          Whether to use JSON output format

      --status-after
          After a mutation command completes, also output workspace status.
          
          In human mode, prints status after the command output. In JSON mode, wraps both in
          {"result": ..., "status": ...} on success, or {"result": ..., "status_error": ...} if the
          status query fails (in which case "status" is absent).

  -h, --help
          Print help (see a summary with '-h')

"#]]);

    Ok(())
}

#[test]
fn pre_commit_allows_on_non_workspace_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // On main branch (not gitbutler/workspace), should exit 0
    env.but("hook pre-commit")
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn post_checkout_file_checkout_is_noop() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // File checkout (is_branch_checkout=0) should be a no-op
    env.but("hook post-checkout abc123 def456 0")
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn post_checkout_shows_message_when_leaving_workspace() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Create a workspace branch and record its HEAD SHA, then checkout main
    env.invoke_bash(
        "git checkout -b gitbutler/workspace && \
         git commit --allow-empty -m 'GitButler Workspace Commit' && \
         git checkout main",
    );
    let workspace_sha = env.invoke_git("rev-parse refs/heads/gitbutler/workspace");
    let main_sha = env.invoke_git("rev-parse refs/heads/main");

    // Simulate leaving workspace: prev_head is workspace SHA, now on main
    env.but(format!("hook post-checkout {workspace_sha} {main_sha} 1"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
NOTE: You have left GitButler's managed workspace branch.
...
"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn post_checkout_is_silent_when_previous_state_was_detached_at_workspace_tip() -> anyhow::Result<()>
{
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    env.invoke_bash(
        "git checkout -b gitbutler/workspace && \
         git commit --allow-empty -m 'GitButler Workspace Commit' && \
         WORKSPACE_SHA=$(git rev-parse HEAD) && \
         git checkout --detach \"$WORKSPACE_SHA\" && \
         git checkout main",
    );
    let workspace_sha = env.invoke_git("rev-parse refs/heads/gitbutler/workspace");
    let main_sha = env.invoke_git("rev-parse refs/heads/main");

    env.but(format!("hook post-checkout {workspace_sha} {main_sha} 1"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn pre_commit_blocks_on_workspace_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Create and checkout the gitbutler/workspace branch
    env.invoke_bash(
        "git checkout -b gitbutler/workspace && \
         git commit --allow-empty -m 'GitButler Workspace Commit'",
    );

    // pre-commit should block with an error on gitbutler/workspace
    env.but("hook pre-commit")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![[r#"

GITBUTLER_ERROR: Cannot commit directly to gitbutler/workspace branch.

  GitButler manages commits on this branch. Please use GitButler to commit your changes:
  - Use the GitButler app to create commits
  - Or run 'but commit' from the command line
  
  If you want to exit GitButler mode and use normal git:
  - Run 'but teardown' to switch to a regular branch
  - Or directly checkout another branch: git checkout <branch>


"#]]);

    Ok(())
}

#[test]
fn post_checkout_silent_when_not_leaving_workspace() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Create a workspace branch and a feature branch, then checkout feature
    env.invoke_bash(
        "git checkout -b gitbutler/workspace && \
         git commit --allow-empty -m 'GitButler Workspace Commit' && \
         git checkout main && \
         git checkout -b feature",
    );
    let main_sha = env.invoke_git("rev-parse refs/heads/main");
    let feature_sha = env.invoke_git("rev-parse refs/heads/feature");

    // Simulate checkout from main to feature (not from workspace) — should be silent
    env.but(format!("hook post-checkout {main_sha} {feature_sha} 1"))
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn pre_push_help_shows_usage() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("hook pre-push --help")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Push guard for pre-push hooks.

Blocks `git push` when on the `gitbutler/workspace` branch with a
helpful error message directing the user to use `but push` instead.
Exits 0 (allow) on any other branch.

Accepts the standard pre-push arguments (remote_name, remote_url)
as positional parameters, matching git's pre-push hook signature.
Stdin refspec lines from git are not inspected.

## Examples

prek.toml:

```text
[[repos]]
repo = "local"
hooks = [{ id = "gitbutler-push-guard", language = "system", entry = "but hook pre-push" }]
```

lefthook.yml:

```text
pre-push:
  commands:
    gitbutler-push-guard:
      run: but hook pre-push
```

Usage: but hook pre-push [OPTIONS] [REMOTE_NAME] [REMOTE_URL]

Arguments:
  [REMOTE_NAME]
          The name of the remote being pushed to (provided by git)
          
          [default: ""]

  [REMOTE_URL]
          The URL of the remote being pushed to (provided by git)
          
          [default: ""]

Options:
  -j, --json
          Whether to use JSON output format

      --status-after
          After a mutation command completes, also output workspace status.
          
          In human mode, prints status after the command output. In JSON mode, wraps both in
          {"result": ..., "status": ...} on success, or {"result": ..., "status_error": ...} if the
          status query fails (in which case "status" is absent).

  -h, --help
          Print help (see a summary with '-h')

"#]]);

    Ok(())
}

#[test]
fn pre_push_allows_on_non_workspace_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // On main branch (not gitbutler/workspace), should exit 0
    env.but("hook pre-push origin https://example.com/repo.git")
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn pre_push_ignores_stdin_refspecs() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Git pipes refspec lines on stdin to pre-push hooks.
    // Our guard only checks the branch name and must not choke on stdin data.
    // Pipe refspec-like data via stdin — should still exit 0 on a non-workspace branch.
    env.but("hook pre-push origin https://example.com/repo.git")
        .stdin("refs/heads/main abc123 refs/heads/main def456\n")
        .assert()
        .success()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn pre_push_blocks_on_workspace_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Create and checkout the gitbutler/workspace branch
    env.invoke_bash(
        "git checkout -b gitbutler/workspace && \
         git commit --allow-empty -m 'GitButler Workspace Commit'",
    );

    // pre-push should block with an error on gitbutler/workspace
    env.but("hook pre-push origin https://example.com/repo.git")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![[r#"

GITBUTLER_ERROR: Cannot push the gitbutler/workspace branch.

  The workspace branch is a synthetic branch managed by GitButler.
  Pushing it to a remote would publish GitButler's internal state.
  
  To push your branches, use:
  - The GitButler app to push branches
  - Or run 'but push' from the command line
  
  If you want to exit GitButler mode and push normally:
  - Run 'but teardown' to switch to a regular branch


"#]]);

    Ok(())
}
