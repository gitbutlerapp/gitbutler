//! Management of GitButler-installed Git hooks
//!
//! This module handles installation and cleanup of Git hooks that prevent
//! accidental `git commit` usage on the `gitbutler/workspace` branch and
//! provide auto-cleanup when users checkout away from GitButler mode.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use but_core::RepositoryExt as _;

/// Marker comment to identify GitButler-managed hooks
const GITBUTLER_HOOK_SIGNATURE: &str = "# GITBUTLER_MANAGED_HOOK_V1";

/// Repo-local Git config key controlling whether GitButler should install managed hooks.
const GITBUTLER_INSTALL_HOOKS_CONFIG_KEY: &str = "gitbutler.installHooks";

/// Pre-commit hook script content
const PRE_COMMIT_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook is managed by GitButler to prevent accidental commits on the workspace branch.
# Your original pre-commit hook has been preserved as 'pre-commit-user'.

HOOKS_DIR=$(dirname "$0")

# Run user's hook first if it exists - if it fails, stop here
if [ -x "$HOOKS_DIR/pre-commit-user" ]; then
    "$HOOKS_DIR/pre-commit-user" "$@" || exit $?
fi

# Get the current branch name
BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null)

if [ "$BRANCH" = "gitbutler/workspace" ]; then
    echo ""
    echo "GITBUTLER_ERROR: Cannot commit directly to gitbutler/workspace branch."
    echo ""
    echo "GitButler manages commits on this branch. Please use GitButler to commit your changes:"
    echo "  - Use the GitButler app to create commits"
    echo "  - Or run 'but commit' from the command line"
    echo ""
    echo "If you want to exit GitButler mode and use normal git:"
    echo "  - Run 'but teardown' to switch to a regular branch"
    echo "  - Or directly checkout another branch: git checkout <branch>"
    echo ""
    echo "If you no longer have the GitButler CLI installed, you can simply remove this hook and checkout another branch:"
    printf '  rm "%s/pre-commit"\n' "$HOOKS_DIR"
    echo ""
    exit 1
fi

# Not on workspace branch - run user's original hook if it exists
if [ -x "$HOOKS_DIR/pre-commit-user" ]; then
    echo ""
    echo "WARNING: GitButler's pre-commit hook is still installed but you're not on gitbutler/workspace."
    echo "If you're no longer using GitButler, you can restore your original hook:"
    printf '  mv "%s/pre-commit-user" "%s/pre-commit"\n' "$HOOKS_DIR" "$HOOKS_DIR"
    echo ""
fi

exit 0
"#;

/// Post-checkout hook script content
const POST_CHECKOUT_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook auto-cleans GitButler hooks when you checkout away from gitbutler/workspace.

PREV_HEAD=$1
NEW_HEAD=$2
BRANCH_CHECKOUT=$3

# Only act on branch checkouts (not file checkouts)
if [ "$BRANCH_CHECKOUT" != "1" ]; then
    # Run user's hook if it exists
    if [ -x "$(dirname "$0")/post-checkout-user" ]; then
        exec "$(dirname "$0")/post-checkout-user" "$@"
    fi
    exit 0
fi

# Get the new branch name
NEW_BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null)

# If we just left gitbutler/workspace (and aren't coming back to it)
PREV_BRANCH=$(git name-rev --name-only "$PREV_HEAD" 2>/dev/null | sed 's|^remotes/||')
if echo "$PREV_BRANCH" | grep -q "gitbutler/workspace"; then
    if [ "$NEW_BRANCH" != "gitbutler/workspace" ]; then
        echo ""
        echo "NOTE: You have left GitButler's managed workspace branch."
        echo "Cleaning up GitButler hooks..."

        HOOKS_DIR=$(dirname "$0")

        # Restore pre-commit - but only if it's GitButler-managed
        if [ -f "$HOOKS_DIR/pre-commit-user" ]; then
            mv "$HOOKS_DIR/pre-commit-user" "$HOOKS_DIR/pre-commit"
            echo "  Restored: pre-commit"
        elif [ -f "$HOOKS_DIR/pre-commit" ]; then
            # Only remove if it's GitButler-managed (has our signature)
            if grep -q "GITBUTLER_MANAGED_HOOK_V1" "$HOOKS_DIR/pre-commit"; then
                rm "$HOOKS_DIR/pre-commit"
                echo "  Removed: pre-commit (GitButler managed)"
            else
                echo "  Warning: pre-commit hook is not GitButler-managed, leaving it untouched"
            fi
        fi

        # Clean up pre-push hook
        if [ -f "$HOOKS_DIR/pre-push-user" ]; then
            mv "$HOOKS_DIR/pre-push-user" "$HOOKS_DIR/pre-push"
            echo "  Restored: pre-push"
        elif [ -f "$HOOKS_DIR/pre-push" ]; then
            if grep -q "GITBUTLER_MANAGED_HOOK_V1" "$HOOKS_DIR/pre-push"; then
                rm "$HOOKS_DIR/pre-push"
                echo "  Removed: pre-push (GitButler managed)"
            else
                echo "  Warning: pre-push hook is not GitButler-managed, leaving it untouched"
            fi
        fi

        # Run user's post-checkout if it exists, then clean up
        if [ -x "$HOOKS_DIR/post-checkout-user" ]; then
            "$HOOKS_DIR/post-checkout-user" "$@"
            mv "$HOOKS_DIR/post-checkout-user" "$HOOKS_DIR/post-checkout"
            echo "  Restored: post-checkout"
        else
            # Only remove self if we're GitButler-managed (we should be, but check anyway)
            if grep -q "GITBUTLER_MANAGED_HOOK_V1" "$HOOKS_DIR/post-checkout"; then
                rm "$HOOKS_DIR/post-checkout"
                echo "  Removed: post-checkout (GitButler managed)"
            else
                echo "  Warning: post-checkout hook is not GitButler-managed, leaving it untouched"
            fi
        fi

        echo ""
        echo "To return to GitButler mode, run: but setup"
        echo ""
        exit 0
    fi
fi

# Run user's hook if it exists
if [ -x "$(dirname "$0")/post-checkout-user" ]; then
    exec "$(dirname "$0")/post-checkout-user" "$@"
fi

exit 0
"#;

/// Pre-push hook script content
const PRE_PUSH_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook is managed by GitButler to prevent accidental pushes of the workspace branch.
# Your original pre-push hook has been preserved as 'pre-push-user'.

HOOKS_DIR=$(dirname "$0")

# Get the current branch name
BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null)

if [ "$BRANCH" = "gitbutler/workspace" ]; then
    echo ""
    echo "GITBUTLER_ERROR: Cannot push the gitbutler/workspace branch."
    echo ""
    echo "The workspace branch is a synthetic branch managed by GitButler."
    echo "Pushing it to a remote would publish GitButler's internal state."
    echo ""
    echo "To push your branches, use:"
    echo "  - The GitButler app to push branches"
    echo "  - Or run 'but push' from the command line"
    echo ""
    echo "If you want to exit GitButler mode and push normally:"
    echo "  - Run 'but teardown' to switch to a regular branch"
    echo ""
    exit 1
fi

# Not on workspace branch - run user's original hook if it exists
if [ -x "$HOOKS_DIR/pre-push-user" ]; then
    exec "$HOOKS_DIR/pre-push-user" "$@"
fi

exit 0
"#;

/// Result of hook installation/uninstallation
#[derive(Debug, Clone)]
pub enum HookInstallationResult {
    /// Hook was successfully installed/uninstalled
    Success,
    /// Hook was already in the desired state
    AlreadyConfigured,
    /// Hook was skipped because it is owned by an external hook manager
    Skipped {
        /// The name of the hook that was skipped (e.g. "pre-commit")
        hook_name: String,
    },
    /// Installation partially succeeded with warnings
    PartialSuccess { warnings: Vec<String> },
}

/// Types of hooks we manage
#[derive(Debug, Clone, Copy)]
enum ManagedHookType {
    PreCommit,
    PostCheckout,
    PrePush,
}

impl ManagedHookType {
    fn hook_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::PostCheckout => "post-checkout",
            Self::PrePush => "pre-push",
        }
    }

    fn user_backup_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit-user",
            Self::PostCheckout => "post-checkout-user",
            Self::PrePush => "pre-push-user",
        }
    }

    fn script_content(&self) -> &'static str {
        match self {
            Self::PreCommit => PRE_COMMIT_HOOK_SCRIPT,
            Self::PostCheckout => POST_CHECKOUT_HOOK_SCRIPT,
            Self::PrePush => PRE_PUSH_HOOK_SCRIPT,
        }
    }
}

/// Check if the given directory contains any GitButler-managed hook files.
///
/// This is useful for detecting orphaned hooks when `core.hooksPath` has been
/// changed to a different directory.
pub fn has_managed_hooks_in(dir: &Path) -> bool {
    for hook_type in [
        ManagedHookType::PreCommit,
        ManagedHookType::PostCheckout,
        ManagedHookType::PrePush,
    ] {
        if is_gitbutler_managed_hook(&dir.join(hook_type.hook_name())) {
            return true;
        }
    }
    false
}

/// Check if a hook file contains our signature
fn is_gitbutler_managed_hook(path: &Path) -> bool {
    if let Ok(content) = fs::read_to_string(path) {
        content.contains(GITBUTLER_HOOK_SIGNATURE)
    } else {
        false
    }
}

/// Set executable permissions on Unix systems
fn set_hook_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .context("Failed to set hook as executable")?;
    }
    Ok(())
}

/// Install a single managed hook.
///
/// When `force` is true, existing non-GitButler hooks are overwritten
/// (backed up to `<hook>-user`) instead of being skipped.
fn install_hook(
    hooks_dir: &Path,
    hook_type: ManagedHookType,
    force: bool,
) -> Result<HookInstallationResult> {
    let hook_path = hooks_dir.join(hook_type.hook_name());
    let user_backup_path = hooks_dir.join(hook_type.user_backup_name());

    // Create hooks directory if it doesn't exist
    if !hooks_dir.exists() {
        fs::create_dir_all(hooks_dir).context("Failed to create hooks directory")?;
    }

    // Check if our hook is already installed
    if hook_path.exists() && is_gitbutler_managed_hook(&hook_path) {
        // Content-based staleness detection: if our marker is present but
        // the content differs from the current template, update in place.
        let current_content = fs::read_to_string(&hook_path).unwrap_or_default();
        if current_content == hook_type.script_content() {
            return Ok(HookInstallationResult::AlreadyConfigured);
        }
        tracing::info!(
            "Updating stale {} hook (content differs from current template)",
            hook_type.hook_name()
        );
        fs::write(&hook_path, hook_type.script_content())
            .context("Failed to update managed hook")?;
        set_hook_executable(&hook_path)?;
        return Ok(HookInstallationResult::Success);
    }

    // If a hook exists that we didn't create, it's likely owned by an external
    // hook manager — don't overwrite it unless `force` is true.
    if hook_path.exists() && !is_gitbutler_managed_hook(&hook_path) {
        if force {
            if !user_backup_path.exists() {
                fs::rename(&hook_path, &user_backup_path).with_context(|| {
                    format!(
                        "Failed to back up existing {} hook to {}",
                        hook_type.hook_name(),
                        hook_type.user_backup_name()
                    )
                })?;
                tracing::info!(
                    "Force-installing {} — backed up existing hook to {}",
                    hook_type.hook_name(),
                    hook_type.user_backup_name()
                );
            } else {
                tracing::info!(
                    "Force-installing {} — backup already exists at {}, overwriting hook in place",
                    hook_type.hook_name(),
                    hook_type.user_backup_name()
                );
            }
        } else {
            tracing::info!(
                "Skipping {} — hook exists and is not GitButler-managed",
                hook_type.hook_name()
            );
            return Ok(HookInstallationResult::Skipped {
                hook_name: hook_type.hook_name().to_owned(),
            });
        }
    }

    // Write our managed hook
    let hook_content = hook_type.script_content();

    fs::write(&hook_path, hook_content).context("Failed to write managed hook")?;

    set_hook_executable(&hook_path)?;

    Ok(HookInstallationResult::Success)
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
            return Ok(HookInstallationResult::Skipped {
                hook_name: hook_type.hook_name().to_owned(),
            });
        }
    }

    // Restore user's backup if it exists
    if user_backup_path.exists() {
        fs::rename(&user_backup_path, &hook_path).context("Failed to restore user hook")?;
    }

    Ok(HookInstallationResult::Success)
}

/// Return the repo-local Git config key used to persist managed hook installation preference.
pub fn install_hooks_config_key() -> &'static str {
    GITBUTLER_INSTALL_HOOKS_CONFIG_KEY
}

/// Return whether GitButler-managed hook installation is enabled for this repository.
///
/// Defaults to `true` when the repository-local config key is not present.
pub fn install_managed_hooks_enabled(repo: &gix::Repository) -> bool {
    repo.config_snapshot()
        .boolean(GITBUTLER_INSTALL_HOOKS_CONFIG_KEY)
        .unwrap_or(true)
}

/// Persist whether GitButler should install managed hooks for this repository.
pub fn set_install_managed_hooks_enabled(repo: &gix::Repository, enabled: bool) -> Result<()> {
    let mut config = repo.local_common_config_for_editing()?;
    config.set_raw_value(
        &GITBUTLER_INSTALL_HOOKS_CONFIG_KEY,
        if enabled { "true" } else { "false" },
    )?;
    repo.write_local_common_config(&config)?;
    Ok(())
}

/// Resolve the hooks directory for the given repository.
///
/// Checks `core.hooksPath` in the git config first. If not set, falls back
/// to the `hooks` subdirectory of the git directory (e.g. `.git/hooks`).
pub fn resolve_hooks_dir(repo: &gix::Repository) -> PathBuf {
    repo.config_snapshot()
        .string("core.hooksPath")
        .map(|p| {
            let path = PathBuf::from(p.to_string());
            if path.is_relative() {
                let base = repo.workdir().unwrap_or(repo.git_dir());
                base.join(&path)
            } else {
                path
            }
        })
        .unwrap_or_else(|| repo.git_dir().join("hooks"))
}

/// Install all GitButler managed hooks into the given hooks directory.
///
/// Called after switching HEAD to gitbutler/workspace.
/// The caller is responsible for resolving the hooks directory
/// (respecting `core.hooksPath` if set), e.g. via [`resolve_hooks_dir`].
///
/// When `force` is true, existing non-GitButler hooks are backed up and
/// overwritten instead of being skipped.
pub fn install_managed_hooks(hooks_dir: &Path, force: bool) -> Result<HookInstallationResult> {
    let mut warnings = Vec::new();
    let mut already_configured_count = 0;
    let mut skipped_hooks = Vec::new();

    for hook_type in [
        ManagedHookType::PreCommit,
        ManagedHookType::PostCheckout,
        ManagedHookType::PrePush,
    ] {
        match install_hook(hooks_dir, hook_type, force) {
            Ok(HookInstallationResult::Success) => {
                tracing::debug!("Installed {} hook", hook_type.hook_name());
            }
            Ok(HookInstallationResult::AlreadyConfigured) => {
                tracing::trace!("{} hook already configured", hook_type.hook_name());
                already_configured_count += 1;
            }
            Ok(HookInstallationResult::Skipped { hook_name }) => {
                tracing::info!("{hook_name} hook skipped — owned by external hook manager");
                skipped_hooks.push(hook_name);
            }
            Ok(HookInstallationResult::PartialSuccess { warnings: w }) => {
                warnings.extend(w);
            }
            Err(e) => {
                warnings.push(format!(
                    "Failed to install {}: {}",
                    hook_type.hook_name(),
                    e
                ));
            }
        }
    }

    // If any hooks were skipped due to external ownership, report that
    if let Some(hook_name) = skipped_hooks.into_iter().next() {
        return Ok(HookInstallationResult::Skipped { hook_name });
    }

    // If all hooks were already configured, return AlreadyConfigured
    if already_configured_count == 3 && warnings.is_empty() {
        Ok(HookInstallationResult::AlreadyConfigured)
    } else if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}

/// Outcome of the high-level hook setup flow performed by [`ensure_managed_hooks`].
///
/// This enum captures every possible resolution so callers (CLI, app, background
/// operations) can render appropriate user-facing guidance without duplicating
/// detection / persistence logic.
#[derive(Debug, Clone)]
pub enum HookSetupOutcome {
    /// GitButler managed hooks were installed (or were already up to date).
    Installed,
    /// Hook installation is disabled via the `gitbutler.installHooks` repo-local
    /// config key. No detection or installation was attempted.
    DisabledByConfig,
    /// An external hook manager was detected. The repo-local config has been
    /// persisted (`gitbutler.installHooks=false`) and any previously-installed
    /// GitButler managed hooks have been removed.
    ExternalManagerDetected {
        /// Human-readable name of the detected manager (e.g. `"prek"`).
        manager_name: String,
        /// Integration instructions for wiring GitButler through the manager.
        instructions: String,
    },
    /// An individual hook file was skipped because it exists and is not
    /// GitButler-managed, but no known external hook manager was detected.
    HookSkipped {
        /// The name of the skipped hook (e.g. `"pre-commit"`).
        hook_name: String,
    },
    /// Hook installation failed (non-fatal).
    Failed {
        /// A human-readable description of the error.
        error: String,
    },
}

/// High-level hook management entry point used by both CLI and app flows.
///
/// Encapsulates the full detect → persist → cleanup → install sequence so
/// every caller gets consistent behavior:
///
/// 1. If `gitbutler.installHooks` is `false`, returns [`HookSetupOutcome::DisabledByConfig`].
/// 2. If `force` is `false` and an external hook manager is detected, persists
///    `gitbutler.installHooks=false`, removes any GitButler-managed hooks, and
///    returns [`HookSetupOutcome::ExternalManagerDetected`].
/// 3. Otherwise installs (or updates) all managed hooks via [`install_managed_hooks`].
pub fn ensure_managed_hooks(
    repo: &gix::Repository,
    hooks_dir: &Path,
    force: bool,
) -> HookSetupOutcome {
    // Fast path: installation disabled by repo-local config.
    // When `force` is true the caller has explicitly requested installation,
    // so we skip the config check entirely.
    if !force && !install_managed_hooks_enabled(repo) {
        tracing::info!(
            config_key = GITBUTLER_INSTALL_HOOKS_CONFIG_KEY,
            "Managed hook installation disabled via repo-local config, skipping."
        );
        return HookSetupOutcome::DisabledByConfig;
    }

    // Detect external hook manager (skip detection when forcing).
    if !force {
        let workdir = repo
            .workdir()
            .unwrap_or_else(|| repo.git_dir())
            .to_path_buf();

        if let Some((manager_name, instructions)) =
            crate::hook_manager::detect_hook_manager_in_hooks_dir(hooks_dir, &workdir)
        {
            // Persist opt-out so subsequent calls take the fast path.
            if let Err(e) = set_install_managed_hooks_enabled(repo, false) {
                tracing::warn!(
                    error = %e,
                    "Failed to persist hook installation opt-out after detecting {manager_name}"
                );
            }

            // Clean up any previously-installed GitButler hooks.
            if let Err(e) = uninstall_managed_hooks(hooks_dir) {
                tracing::warn!(
                    error = %e,
                    "Failed to remove GitButler managed hooks while switching to externally-managed mode"
                );
            }

            tracing::info!(
                manager = manager_name,
                "External hook manager detected; persisted config and cleaned up managed hooks. \
                 Run `but setup` for integration instructions."
            );
            return HookSetupOutcome::ExternalManagerDetected {
                manager_name: manager_name.to_owned(),
                instructions: instructions.to_owned(),
            };
        }
    }

    // No external manager (or force mode) — install managed hooks.
    match install_managed_hooks(hooks_dir, force) {
        Ok(
            HookInstallationResult::Success
            | HookInstallationResult::AlreadyConfigured
            | HookInstallationResult::PartialSuccess { .. },
        ) => HookSetupOutcome::Installed,
        Ok(HookInstallationResult::Skipped { hook_name }) => {
            HookSetupOutcome::HookSkipped { hook_name }
        }
        Err(e) => HookSetupOutcome::Failed {
            error: format!("{e:#}"),
        },
    }
}

/// Uninstall all GitButler managed hooks and restore user's originals.
///
/// Called during teardown.
/// The caller is responsible for resolving the hooks directory
/// (respecting `core.hooksPath` if set).
pub fn uninstall_managed_hooks(hooks_dir: &Path) -> Result<HookInstallationResult> {
    let mut warnings = Vec::new();

    for hook_type in [
        ManagedHookType::PreCommit,
        ManagedHookType::PostCheckout,
        ManagedHookType::PrePush,
    ] {
        match uninstall_hook(hooks_dir, hook_type) {
            Ok(HookInstallationResult::Success) => {
                tracing::debug!("Uninstalled {} hook", hook_type.hook_name());
            }
            Ok(
                HookInstallationResult::AlreadyConfigured | HookInstallationResult::Skipped { .. },
            ) => {}
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
