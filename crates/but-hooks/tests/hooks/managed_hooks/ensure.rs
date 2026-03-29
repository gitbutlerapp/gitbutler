use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::time::SystemTime;

use anyhow::Result;
use but_hooks::managed_hooks::{
    HookSetupOutcome, ensure_managed_hooks, install_managed_hooks, install_managed_hooks_enabled,
    set_install_managed_hooks_enabled,
};

use super::{
    create_prek_environment, create_repo_with_hooks_dir, create_user_hook, hook_exists, read_hook,
};

#[test]
fn installs_when_no_manager_and_config_default() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(outcome, HookSetupOutcome::Installed { .. }),
        "Should install hooks on clean repo, got: {outcome:?}"
    );

    super::assert_all_hooks_exist(&hooks_dir);
    Ok(())
}

#[test]
fn returns_disabled_by_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    set_install_managed_hooks_enabled(&repo, false)?;
    // Re-open to pick up the config change.
    let repo = gix::open(_temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(outcome, HookSetupOutcome::DisabledByConfig),
        "Should return DisabledByConfig when config is false, got: {outcome:?}"
    );

    // No hooks should be installed.
    super::assert_no_hooks_exist(&hooks_dir);
    Ok(())
}

#[test]
fn force_bypasses_disabled_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    set_install_managed_hooks_enabled(&repo, false)?;
    // Re-open to pick up the config change.
    let repo = gix::open(_temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, true, SystemTime::now());
    assert!(
        matches!(outcome, HookSetupOutcome::Installed { .. }),
        "force=true should bypass DisabledByConfig, got: {outcome:?}"
    );

    super::assert_all_hooks_exist(&hooks_dir);
    Ok(())
}

#[test]
fn detects_prek_and_persists_config() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    create_prek_environment(&hooks_dir, _temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(
            outcome,
            HookSetupOutcome::ExternalManagerDetected {
                ref manager_name,
                ..
            } if manager_name == "prek"
        ),
        "Should detect prek, got: {outcome:?}"
    );

    // Config should be persisted as false.
    let repo = gix::open(_temp.path())?;
    assert!(
        !install_managed_hooks_enabled(&repo),
        "installHooks should be persisted as false after detection"
    );
    Ok(())
}

#[test]
fn subsequent_call_after_detection_returns_external_manager_detected() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    create_prek_environment(&hooks_dir, _temp.path())?;

    // First call: detect manager, persist installHooks=false.
    let outcome1 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(matches!(
        outcome1,
        HookSetupOutcome::ExternalManagerDetected { .. }
    ));

    // Re-open repo to see persisted config.
    let repo = gix::open(_temp.path())?;

    // Second call: installHooks=false is already set, but prek hooks are still present.
    // Should re-detect prek and return ExternalManagerDetected (not the generic
    // DisabledByConfig), so callers can show the informative "Detected prek" message.
    let outcome2 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(outcome2, HookSetupOutcome::ExternalManagerDetected { .. }),
        "Subsequent call with prek hooks still present should return ExternalManagerDetected, got: {outcome2:?}"
    );
    Ok(())
}

#[test]
fn subsequent_call_after_detection_returns_disabled_when_manager_gone() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    create_prek_environment(&hooks_dir, _temp.path())?;

    // First call: detect manager, persist installHooks=false.
    let outcome1 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(matches!(
        outcome1,
        HookSetupOutcome::ExternalManagerDetected { .. }
    ));

    // Remove the prek hook file and config so detection no longer finds prek.
    fs::remove_file(hooks_dir.join("pre-commit"))?;
    fs::remove_file(_temp.path().join("prek.toml"))?;

    // Re-open repo to see persisted config.
    let repo = gix::open(_temp.path())?;

    // Second call: installHooks=false but no external manager detected → DisabledByConfig.
    let outcome2 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(outcome2, HookSetupOutcome::DisabledByConfig),
        "Subsequent call with manager gone should return DisabledByConfig, got: {outcome2:?}"
    );
    Ok(())
}

#[test]
fn force_overrides_external_manager() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    create_prek_environment(&hooks_dir, _temp.path())?;

    // Force install should skip detection and install hooks.
    let outcome = ensure_managed_hooks(&repo, &hooks_dir, true, SystemTime::now());
    assert!(
        matches!(outcome, HookSetupOutcome::Installed { .. }),
        "force=true should override external manager, got: {outcome:?}"
    );

    // Prek hook should be backed up, GB hook installed.
    assert!(hook_exists(&hooks_dir, "pre-commit-user"));
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert!(
        content.contains("GITBUTLER_MANAGED_HOOK_V1"),
        "GitButler hook should be installed after force"
    );
    Ok(())
}

#[test]
fn detection_cleans_up_existing_gb_hooks() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Pre-install GitButler hooks (simulates prior app-triggered install).
    install_managed_hooks(&hooks_dir, false, SystemTime::now())?;
    super::assert_all_hooks_exist(&hooks_dir);

    // Now introduce a prek-managed hook (overwriting GB's pre-commit).
    create_prek_environment(&hooks_dir, _temp.path())?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(matches!(
        outcome,
        HookSetupOutcome::ExternalManagerDetected { .. }
    ));

    // GB-managed post-checkout and pre-push should be cleaned up.
    assert!(
        !hook_exists(&hooks_dir, "post-checkout"),
        "post-checkout should be removed after external manager detection"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-push"),
        "pre-push should be removed after external manager detection"
    );

    // The prek hook should still be there (uninstall only removes GB-managed hooks).
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert!(
        content.contains("File generated by prek"),
        "Prek hook should not be removed"
    );
    Ok(())
}

#[test]
fn skips_unknown_external_hook() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Create an unknown external hook (not a known manager, no config files).
    let unknown_hook = "#!/bin/sh\n# Some unknown hook manager\necho 'unknown'\n";
    create_user_hook(&hooks_dir, "pre-commit", unknown_hook)?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(
            outcome,
            HookSetupOutcome::HookSkipped { ref hook_names } if hook_names.contains(&"pre-commit".to_string())
        ) || matches!(outcome, HookSetupOutcome::Installed { .. })
            || matches!(outcome, HookSetupOutcome::PartialSuccess { .. }),
        "Should either skip unknown hook or install remaining, got: {outcome:?}"
    );

    // The unknown hook should be preserved.
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        content, unknown_hook,
        "Unknown external hook should be preserved"
    );
    Ok(())
}

#[test]
fn idempotent_on_installed_hooks() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // First install.
    let outcome1 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(matches!(outcome1, HookSetupOutcome::Installed { .. }));

    // Second install — should still return Installed (already configured).
    let outcome2 = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());
    assert!(
        matches!(outcome2, HookSetupOutcome::AlreadyInstalled),
        "Idempotent call should return AlreadyInstalled, got: {outcome2:?}"
    );
    Ok(())
}

/// Verify that `HookSetupOutcome::PartialSuccess` is returned (not `Installed`) when
/// hook writes fail due to an IO error, and that warnings are surfaced to the caller.
///
/// Uses a read-only hooks directory so every `fs::write` inside `install_hook` fails,
/// causing all three hooks to collect errors rather than returning success or skipped.
#[cfg(unix)]
#[test]
fn partial_success_surfaces_warnings() -> Result<()> {
    let (_temp, repo, hooks_dir) = create_repo_with_hooks_dir()?;

    // Make the hooks directory read-only so all hook file writes fail with EACCES.
    fs::set_permissions(&hooks_dir, fs::Permissions::from_mode(0o555))?;

    let outcome = ensure_managed_hooks(&repo, &hooks_dir, false, SystemTime::now());

    // Restore so TempDir cleanup can remove the directory.
    fs::set_permissions(&hooks_dir, fs::Permissions::from_mode(0o755))?;

    match outcome {
        HookSetupOutcome::PartialSuccess { ref warnings, .. } => {
            assert!(
                !warnings.is_empty(),
                "PartialSuccess should carry at least one warning, got none"
            );
        }
        other => panic!("Expected PartialSuccess, got: {other:?}"),
    }

    Ok(())
}
