//! Cleanup of Git hooks installed by older GitButler versions
//!
//! GitButler used to install hooks that prevented accidental `git commit` usage
//! on the `gitbutler/workspace` branch and cleaned themselves up when users
//! checked out away from GitButler mode. It no longer installs them; this module
//! removes the ones previous versions left behind.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

/// Marker comment to identify GitButler-managed hooks
const GITBUTLER_HOOK_SIGNATURE: &str = "# GITBUTLER_MANAGED_HOOK_V1";

/// Result of hook uninstallation
#[derive(Debug, Clone)]
pub enum HookInstallationResult {
    /// Hook was successfully uninstalled
    Success,
    /// Hook was already in the desired state
    AlreadyConfigured,
    /// Uninstallation partially succeeded with warnings
    PartialSuccess { warnings: Vec<String> },
}

/// Subdirectory of the common git directory where GitButler stored its hook scripts
/// when hooks were registered through git config (git 2.54+).
const CONFIG_HOOK_SCRIPTS_SUBDIR: &str = "gitbutler/hooks";

/// Types of hooks we manage
#[derive(Debug, Clone, Copy)]
enum ManagedHookType {
    PreCommit,
    PostCheckout,
}

impl ManagedHookType {
    /// Every hook type GitButler manages.
    const ALL: [ManagedHookType; 2] = [ManagedHookType::PreCommit, ManagedHookType::PostCheckout];

    fn hook_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::PostCheckout => "post-checkout",
        }
    }

    /// Subsection name under which this hook was registered in git config (git 2.54+).
    fn config_section(&self) -> &'static str {
        match self {
            Self::PreCommit => "gitbutler-pre-commit",
            Self::PostCheckout => "gitbutler-post-checkout",
        }
    }

    fn user_backup_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit-user",
            Self::PostCheckout => "post-checkout-user",
        }
    }
}

fn hooks_dir_from_git_dir_and_config_path_for_run_dir(
    git_dir: &Path,
    hook_run_dir: &Path,
    hooks_path: Option<PathBuf>,
) -> PathBuf {
    let hooks_path = hooks_path.map(|path| {
        if path.is_relative() {
            hook_run_dir.join(path)
        } else {
            path
        }
    });
    hooks_path.unwrap_or_else(|| git_dir.join("hooks"))
}

/// Get the hooks directory for a `gix` repository, respecting `core.hooksPath`.
pub(crate) fn get_hooks_dir(repo: &gix::Repository) -> PathBuf {
    hooks_dir_from_git_dir_and_config_path_for_run_dir(
        repo.git_dir(),
        repo.workdir().unwrap_or(repo.git_dir()),
        repo.config_snapshot()
            .trusted_path("core.hooksPath")
            .ok()
            .flatten(),
    )
}

/// Directory holding GitButler's config-registered hook scripts.
fn config_scripts_dir(repo: &gix::Repository) -> PathBuf {
    repo.common_dir().join(CONFIG_HOOK_SCRIPTS_SUBDIR)
}

/// Check if a hook file contains our signature
fn is_gitbutler_managed_hook(path: &Path) -> bool {
    if let Ok(content) = fs::read_to_string(path) {
        content.contains(GITBUTLER_HOOK_SIGNATURE)
    } else {
        false
    }
}

/// Uninstall a single managed hook and restore user's original
fn uninstall_hook(hooks_dir: &Path, hook_type: ManagedHookType) -> Result<HookInstallationResult> {
    let hook_path = hooks_dir.join(hook_type.hook_name());
    let user_backup_path = hooks_dir.join(hook_type.user_backup_name());

    // Only remove if it's our managed hook
    if hook_path.exists() {
        if is_gitbutler_managed_hook(&hook_path) {
            fs::remove_file(&hook_path).context("Failed to remove managed hook")?;
        } else {
            // Not our hook, don't touch it
            tracing::debug!(
                "{} is not GitButler-managed, skipping",
                hook_type.hook_name()
            );
            return Ok(HookInstallationResult::AlreadyConfigured);
        }
    }

    // Restore user's backup if it exists
    if user_backup_path.exists() {
        fs::rename(&user_backup_path, &hook_path).context("Failed to restore user hook")?;
    }

    Ok(HookInstallationResult::Success)
}

/// Uninstall all GitButler managed hooks and restore user's originals
///
/// Called during teardown. Cleans up both installation mechanisms older versions
/// used: config-based registrations (git 2.54+) and hooks in the hooks directory.
pub fn uninstall_managed_hooks(repo: &gix::Repository) -> Result<HookInstallationResult> {
    let mut warnings = Vec::new();

    if let Err(e) = uninstall_config_based_hooks(repo) {
        warnings.push(format!("Failed to remove config-based hooks: {e}"));
    }

    match uninstall_managed_hooks_at(&get_hooks_dir(repo)) {
        Ok(HookInstallationResult::PartialSuccess { warnings: w }) => warnings.extend(w),
        Ok(_) => {}
        Err(e) => warnings.push(format!("Failed to remove hooks from hooks directory: {e}")),
    }

    if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}

/// Remove GitButler's config-based hook registrations and their scripts.
fn uninstall_config_based_hooks(repo: &gix::Repository) -> Result<()> {
    // Removing an absent section is a no-op, and the file is only written back
    // if anything actually changed.
    but_core::git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
        for hook_type in ManagedHookType::ALL {
            config.remove_section("hook", Some(hook_type.config_section().into()));
        }
        Ok(())
    })?;

    let scripts_dir = config_scripts_dir(repo);
    for hook_type in ManagedHookType::ALL {
        let script_path = scripts_dir.join(hook_type.hook_name());
        // The signature check matters when `core.hooksPath` points at the
        // scripts directory: a user's own hook may live at this path, and only
        // GitButler's signed scripts may be deleted.
        if script_path.exists() && is_gitbutler_managed_hook(&script_path) {
            fs::remove_file(&script_path).context("Failed to remove hook script")?;
        }
    }
    // Remove the scripts directory if it is now empty; keep it if anything else is inside.
    let _ = fs::remove_dir(&scripts_dir);
    Ok(())
}

fn uninstall_managed_hooks_at(hooks_dir: &Path) -> Result<HookInstallationResult> {
    let mut warnings = Vec::new();

    for hook_type in ManagedHookType::ALL {
        match uninstall_hook(hooks_dir, hook_type) {
            Ok(HookInstallationResult::Success) => {
                tracing::debug!("Uninstalled {} hook", hook_type.hook_name());
            }
            Ok(HookInstallationResult::AlreadyConfigured) => {}
            Ok(HookInstallationResult::PartialSuccess { warnings: w }) => {
                warnings.extend(w);
            }
            Err(e) => {
                warnings.push(format!(
                    "Failed to uninstall {}: {}",
                    hook_type.hook_name(),
                    e
                ));
            }
        }
    }

    if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}
