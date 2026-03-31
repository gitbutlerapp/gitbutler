use crate::utils::{CommandExt as _, Sandbox};

/// Simulate a prek environment with config file and project-local binary.
fn simulate_prek_config_and_binary(env: &Sandbox) {
    env.invoke_bash("echo '# prek config' > prek.toml");
    env.invoke_bash(
        "mkdir -p .venv/bin && echo '#!/bin/sh' > .venv/bin/prek && chmod +x .venv/bin/prek",
    );
}

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
fn no_hooks_flag_leaves_no_managed_hook_files_on_disk() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // `but setup --no-hooks` writes installHooks=false then switches to workspace.
    // The workspace switch internally calls ensure_managed_hooks(force=false) which
    // may read a stale config snapshot. This test confirms that the --no-hooks flag
    // results in NO GitButler-managed hook files on disk regardless of config staleness.
    env.but("setup --no-hooks")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    let hooks_dir = env.projects_root().join(".git/hooks");
    for hook_name in &["pre-commit", "post-checkout", "pre-push"] {
        let hook_path = hooks_dir.join(hook_name);
        if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path)?;
            assert!(
                !content.contains("GITBUTLER_MANAGED_HOOK_V1"),
                "{hook_name} should NOT contain GitButler signature after --no-hooks, got: {content}"
            );
        }
    }

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
fn status_detects_config_only_external_manager() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Simulate prek configured + available locally, but hooks not yet installed.
    // This mirrors the state after `uv add --dev prek` + creating prek.toml,
    // before running `prek install`.
    simulate_prek_config_and_binary(&env);

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                external hook manager
  Hook manager:        prek

  pre-commit:          ✗ not configured (prek)
  post-checkout:       ✗ not configured (prek)
  pre-push:            ✗ not configured (prek)

  ⚠ pre-commit is not installed by prek — workspace guard won't run — commits to the workspace branch won't be blocked
  ⚠ post-checkout is not installed by prek — workspace cleanup won't run — stale state may persist when switching branches
  ⚠ pre-push is not installed by prek — push guard won't run — pushes from the workspace branch won't be blocked
  → Hooks are managed by prek. Use `but hook pre-commit` etc. in your prek config.
  → Run `but setup` to see integration instructions.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_warns_when_all_prek_hooks_missing() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Simulate prek configured + available locally but no hooks installed yet
    // (e.g. prek.toml present + binary on PATH, but `prek install` not run).
    simulate_prek_config_and_binary(&env);

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                external hook manager
  Hook manager:        prek

  pre-commit:          ✗ not configured (prek)
  post-checkout:       ✗ not configured (prek)
  pre-push:            ✗ not configured (prek)

  ⚠ pre-commit is not installed by prek — workspace guard won't run — commits to the workspace branch won't be blocked
  ⚠ post-checkout is not installed by prek — workspace cleanup won't run — stale state may persist when switching branches
  ⚠ pre-push is not installed by prek — push guard won't run — pushes from the workspace branch won't be blocked
  → Hooks are managed by prek. Use `but hook pre-commit` etc. in your prek config.
  → Run `but setup` to see integration instructions.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shows_mixed_prek_installed_and_not() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    simulate_prek_config_and_binary(&env);

    // Install a fake prek-owned hook for pre-commit only.
    // post-checkout and pre-push have no hook files (not configured by prek).
    env.invoke_bash(
        "mkdir -p .git/hooks && printf '#!/bin/sh\n# File generated by prek\n' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit",
    );

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                external hook manager
  Hook manager:        prek

  pre-commit:          ● external (prek)
  post-checkout:       ✗ not configured (prek)
  pre-push:            ✗ not configured (prek)

  ⚠ post-checkout is not installed by prek — workspace cleanup won't run — stale state may persist when switching branches
  ⚠ pre-push is not installed by prek — push guard won't run — pushes from the workspace branch won't be blocked
  → Hooks are managed by prek. Use `but hook pre-commit` etc. in your prek config.
  → Run `but setup` to see integration instructions.


"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

#[test]
fn status_shell_output_includes_manager_name_per_hook() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Simulate prek configured + available locally (same setup as config-only test).
    simulate_prek_config_and_binary(&env);

    // Shell output must include the manager name in per-hook fields, not just "external".
    env.but("--format shell hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
hooks_path='[..]'
custom_hooks_path=false
config_enabled=true
mode='external hook manager'
external_manager='prek'
hook_pre_commit='external (prek)'
hook_post_checkout='external (prek)'
hook_pre_push='external (prek)'

"#]])
        .stderr_eq(snapbox::str![]);

    Ok(())
}

/// When mode is `Managed` but a user-owned hook occupies a GB-managed slot,
/// `but hook status` must emit a warning for that slot so the user knows the
/// corresponding guard (e.g. workspace commit block) won't fire.
///
/// Scenario: `but setup` installs all three hooks; then a user hook overwrites
/// `pre-commit` (no GB signature). Mode stays `Managed` (two GB hooks remain),
/// but a warning must appear for the unprotected `pre-commit` slot.
#[test]
fn managed_mode_warns_when_user_hook_occupies_gb_slot() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Install all three GB-managed hooks.
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![]);

    // Overwrite pre-commit with a plain user hook (no GB signature).
    env.invoke_bash(
        "printf '#!/bin/sh\\necho my hook\\n' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit",
    );

    env.but("hook status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"

Hook status

  Hooks path:          ./.git/hooks
  Config:              gitbutler.installHooks = true
  Mode:                GitButler-managed

  pre-commit:          ○ user hook
  post-checkout:       ✓ GitButler-managed
  pre-push:            ✓ GitButler-managed

  ⚠ pre-commit: user hook present — workspace guard won't run — commits to the workspace branch won't be blocked


"#]])
        .stderr_eq(snapbox::str![]);

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
