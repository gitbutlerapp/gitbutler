use crate::utils::{CommandExt as _, Sandbox};

#[test]
fn status_shows_managed_hooks_after_setup() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Setup installs managed hooks
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Hook status should show all 3 hooks as GitButler-managed
    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:[..]
  Config:[..]gitbutler.installHooks = true
  Mode:[..]GitButler-managed

  pre-commit:[..]GitButler-managed
  post-checkout:[..]GitButler-managed
  pre-push:[..]GitButler-managed


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shows_disabled_after_no_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Setup with --no-hooks disables hook installation
    env.but("setup --no-hooks")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = false
  Mode:                disabled

  pre-commit:          ✗ not installed
  post-checkout:       ✗ not installed
  pre-push:            ✗ not installed

  → Run `but setup --force-hooks` to re-enable GitButler-managed hooks.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shows_unconfigured_for_fresh_repo() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // No setup — fresh repo with no hooks
    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                unconfigured

  pre-commit:          ✗ not installed
  post-checkout:       ✗ not installed
  pre-push:            ✗ not installed

  → Run `but setup` to install GitButler hooks.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shows_user_hook() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Create a plain user hook (not GitButler-managed)
    env.invoke_bash("mkdir -p .git/hooks && echo '#!/bin/sh\necho hello' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit");

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                unconfigured

  pre-commit:          ○ user hook
  post-checkout:       ✗ not installed
  pre-push:            ✗ not installed

  → Run `but setup` to install GitButler hooks.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shows_custom_hooks_path() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Setup first to install managed hooks in .git/hooks/
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Set core.hooksPath to a custom directory and install hooks there
    env.invoke_bash("mkdir -p .git/custom-hooks");
    env.invoke_git("config --local core.hooksPath .git/custom-hooks");

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:[..]custom-hooks
[..]core.hooksPath[..]
  Config:              gitbutler.installHooks = true
  Mode:                unconfigured

  pre-commit:          ✗ not installed
  post-checkout:       ✗ not installed
  pre-push:            ✗ not installed

  ⚠ Orphaned GitButler-managed hooks found in ./.git/hooks (core.hooksPath points elsewhere)
  → Run `but setup` to install GitButler hooks.
  → Remove orphaned hooks: rm .git/hooks/pre-commit .git/hooks/post-checkout .git/hooks/pre-push


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_warns_about_orphaned_hooks() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Setup installs managed hooks in .git/hooks/
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Redirect core.hooksPath elsewhere — old hooks are now orphaned
    env.invoke_bash("mkdir -p .git/custom-hooks");
    env.invoke_git("config --local core.hooksPath .git/custom-hooks");

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/custom-hooks
                       (set via core.hooksPath)
  Config:              gitbutler.installHooks = true
  Mode:                unconfigured

  pre-commit:          ✗ not installed
  post-checkout:       ✗ not installed
  pre-push:            ✗ not installed

  ⚠ Orphaned GitButler-managed hooks found in ./.git/hooks (core.hooksPath points elsewhere)
  → Run `but setup` to install GitButler hooks.
  → Remove orphaned hooks: rm .git/hooks/pre-commit .git/hooks/post-checkout .git/hooks/pre-push


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_json_output() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Setup installs managed hooks
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    let output = env.but("--json hook status").allow_json().output()?;
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["configEnabled"], true);
    assert_eq!(json["mode"], "managed");
    assert_eq!(json["customHooksPath"], false);
    assert!(json["externalManager"].is_null());

    let hooks = json["hooks"].as_array().expect("hooks should be an array");
    assert_eq!(hooks.len(), 3);
    assert_eq!(hooks[0]["name"], "pre-commit");
    assert_eq!(hooks[0]["exists"], true);
    assert_eq!(hooks[0]["owner"], "gitbutler");

    Ok(())
}

#[test]
fn status_fails_outside_git_repo() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("hook status")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find a git repository in '.' or in any of its parents

"#]]);

    Ok(())
}
