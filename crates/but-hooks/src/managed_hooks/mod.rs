//! Management of GitButler-installed Git hooks
//!
//! This module handles installation and cleanup of Git hooks that prevent
//! accidental `git commit` usage on the `gitbutler/workspace` branch and
//! provide auto-cleanup when users checkout away from GitButler mode.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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

# Check the workspace branch guard first: if we are on gitbutler/workspace, block
# immediately without running the user hook. Running the user hook first would cause
# unnecessary side effects (formatters, notifications, etc.) for a commit that is
# guaranteed to fail anyway.
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

# Not on workspace branch — delegate to the user's original hook if present.
if [ -x "$HOOKS_DIR/pre-commit-user" ]; then
    "$HOOKS_DIR/pre-commit-user" "$@" || exit $?
fi

exit 0
"#;

/// Post-checkout hook script content
const POST_CHECKOUT_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook auto-cleans GitButler hooks when you checkout away from gitbutler/workspace.

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
PREV_BRANCH=$(git rev-parse --abbrev-ref @{-1} 2>/dev/null)
if [ "$PREV_BRANCH" = "gitbutler/workspace" ]; then
    if [ "$NEW_BRANCH" != "gitbutler/workspace" ]; then
        echo ""
        echo "NOTE: You have left GitButler's managed workspace branch."
        echo "Cleaning up GitButler hooks..."

        HOOKS_DIR=$(dirname "$0")

        # Restore or remove a single GitButler-managed hook.
        # Usage: cleanup_hook <hook-name> [extra-args-for-user-hook...]
        cleanup_hook() {
            _hook="$1"; shift
            if [ -f "$HOOKS_DIR/${_hook}-user" ]; then
                # Run the user hook first if it's executable and we have args
                if [ $# -gt 0 ] && [ -x "$HOOKS_DIR/${_hook}-user" ]; then
                    "$HOOKS_DIR/${_hook}-user" "$@"
                fi
                mv "$HOOKS_DIR/${_hook}-user" "$HOOKS_DIR/${_hook}"
                echo "  Restored: ${_hook}"
            elif [ -f "$HOOKS_DIR/${_hook}" ]; then
                if grep -q "GITBUTLER_MANAGED_HOOK_V1" "$HOOKS_DIR/${_hook}"; then
                    rm "$HOOKS_DIR/${_hook}"
                    echo "  Removed: ${_hook} (GitButler managed)"
                else
                    echo "  Warning: ${_hook} hook is not GitButler-managed, leaving it untouched"
                fi
            fi
        }

        cleanup_hook pre-commit
        cleanup_hook pre-push
        # post-checkout MUST be last: it deletes the currently-executing script.
        # Safe on Unix/macOS — the kernel keeps the inode alive via the open fd
        # until this process exits. On Windows, .git/hooks/ is rarely locked and
        # Git uses .bat/.cmd wrappers that behave similarly.
        cleanup_hook post-checkout "$@"

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
    /// All hooks were successfully installed/updated with per-hook backup status.
    Success {
        /// Per-hook results containing the hook name and backup status.
        hook_results: Vec<(String, HookBackupStatus)>,
    },
    /// Hook was already in the desired state
    AlreadyConfigured,
    /// Hook was skipped because it is owned by an external hook manager
    Skipped {
        /// The names of the hooks that were skipped (e.g. `["pre-commit", "pre-push"]`)
        hook_names: Vec<String>,
    },
    /// Installation partially succeeded: some hooks were installed, others were skipped.
    PartialSuccess {
        /// Names of hooks that were installed or were already up to date.
        installed_hooks: Vec<String>,
        /// Per-hook results for hooks that were installed, including backup status.
        hook_results: Vec<(String, HookBackupStatus)>,
        /// Human-readable descriptions of each hook that was skipped and why.
        warnings: Vec<String>,
    },
}

/// Describes what happened to an existing hook file during installation.
#[derive(Debug, Clone, PartialEq)]
pub enum HookBackupStatus {
    /// No existing hook was present — nothing was backed up.
    None,
    /// An existing hook was backed up to the indicated path.
    Created(String),
    /// A primary backup already existed; the current hook was saved to a secondary
    /// timestamped path (e.g. `pre-commit-user.bak.1234567890`) to avoid data loss.
    SecondaryBackup {
        /// Path of the pre-existing primary backup (e.g. `pre-commit-user`).
        primary_backup: String,
        /// Path of the newly-created secondary backup for the hook that was about to be lost.
        secondary_backup: String,
    },
}

impl HookBackupStatus {
    /// Returns the most-relevant backup path associated with this status, or `None`.
    ///
    /// For [`HookBackupStatus::SecondaryBackup`] this returns the secondary backup path,
    /// since that is the newly-created file the caller most likely wants to surface.
    pub fn to_backup_path(&self) -> Option<&str> {
        match self {
            Self::None => Option::None,
            Self::Created(p) => Some(p.as_str()),
            Self::SecondaryBackup {
                secondary_backup, ..
            } => Some(secondary_backup.as_str()),
        }
    }

    /// Format a human-readable installation summary line for this hook.
    ///
    /// Examples:
    /// - `"✓ Installed pre-commit"`
    /// - `"✓ Installed pre-commit (backed up existing → pre-commit-user)"`
    /// - `"✓ Installed pre-commit (primary backup existed; saved current hook → pre-commit-user.bak.1234567890)"`
    pub fn format_install_line(&self, name: &str) -> String {
        match self {
            Self::None => format!("✓ Installed {name}"),
            Self::Created(backup) => format!("✓ Installed {name} (backed up existing → {backup})"),
            Self::SecondaryBackup {
                secondary_backup, ..
            } => {
                format!(
                    "✓ Installed {name} (primary backup existed; saved current hook → {secondary_backup})"
                )
            }
        }
    }
}

/// Per-hook outcome returned by the private [`uninstall_hook`] helper.
#[derive(Debug)]
enum SingleUninstallOutcome {
    /// The GitButler-managed hook was removed; no user backup existed.
    Removed,
    /// The GitButler-managed hook was removed and the user's backup hook was restored.
    Restored,
    /// Hook was not GitButler-managed; left untouched.
    Skipped,
    /// Hook file did not exist and there was no backup to restore.
    NotPresent,
}

/// Summary of a bulk uninstall of all GitButler-managed hooks.
#[derive(Debug, Default)]
pub struct HookUninstallSummary {
    /// Names of hooks that were removed (GitButler-managed, no user backup present).
    pub removed: Vec<String>,
    /// Names of hooks that were removed and the user's original hook was restored.
    pub restored: Vec<String>,
    /// Non-fatal error messages encountered during uninstall.
    pub warnings: Vec<String>,
}

/// Types of hooks we manage
#[derive(Debug, Clone, Copy)]
enum ManagedHookType {
    PreCommit,
    PostCheckout,
    PrePush,
}

/// All managed hook types in installation order.
const MANAGED_HOOK_TYPES: [ManagedHookType; 3] = [
    ManagedHookType::PreCommit,
    ManagedHookType::PostCheckout,
    ManagedHookType::PrePush,
];

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
    for hook_type in MANAGED_HOOK_TYPES {
        if is_gitbutler_managed_hook(&dir.join(hook_type.hook_name())) {
            return true;
        }
    }
    false
}

/// Return `true` if the hook file at `path` contains the GitButler managed-hook signature.
///
/// This is the canonical check used everywhere to determine whether a hook
/// file was written by GitButler. Prefer this over inlining
/// `content.contains("GITBUTLER_MANAGED_HOOK_V1")` at call sites.
pub fn is_gitbutler_managed_hook(path: &Path) -> bool {
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
    now: SystemTime,
) -> Result<HookInstallationResult> {
    let hook_path = hooks_dir.join(hook_type.hook_name());
    let user_backup_path = hooks_dir.join(hook_type.user_backup_name());

    if !hooks_dir.exists() {
        fs::create_dir_all(hooks_dir).context("Failed to create hooks directory")?;
    }

    let mut backup_status = HookBackupStatus::None;

    if hook_path.exists() && is_gitbutler_managed_hook(&hook_path) {
        let current_content = fs::read_to_string(&hook_path).unwrap_or_default();
        if current_content == hook_type.script_content() {
            return Ok(HookInstallationResult::AlreadyConfigured);
        }
        tracing::info!(
            "Updating stale {} hook (content differs from current template)",
            hook_type.hook_name()
        );
    } else if hook_path.exists() && !is_gitbutler_managed_hook(&hook_path) {
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
                backup_status = HookBackupStatus::Created(hook_type.user_backup_name().to_string());
            } else {
                // Primary backup already exists. To avoid losing the current hook,
                // save it to a timestamped secondary backup before overwriting.
                let ts = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                let secondary_name = format!("{}.bak.{ts}", hook_type.user_backup_name());
                let secondary_path = hooks_dir.join(&secondary_name);
                fs::copy(&hook_path, &secondary_path).with_context(|| {
                    format!(
                        "Failed to save current {} hook to secondary backup {}",
                        hook_type.hook_name(),
                        secondary_name
                    )
                })?;
                tracing::warn!(
                    "Force-installing {} — primary backup {} already exists; \
                     saved current hook to secondary backup {}",
                    hook_type.hook_name(),
                    hook_type.user_backup_name(),
                    secondary_name,
                );
                backup_status = HookBackupStatus::SecondaryBackup {
                    primary_backup: hook_type.user_backup_name().to_string(),
                    secondary_backup: secondary_name,
                };
            }
        } else {
            tracing::info!(
                "Skipping {} — hook exists and is not GitButler-managed",
                hook_type.hook_name()
            );
            return Ok(HookInstallationResult::Skipped {
                hook_names: vec![hook_type.hook_name().to_owned()],
            });
        }
    }

    let hook_content = hook_type.script_content();

    fs::write(&hook_path, hook_content).context("Failed to write managed hook")?;

    set_hook_executable(&hook_path)?;

    Ok(HookInstallationResult::Success {
        hook_results: vec![(hook_type.hook_name().to_string(), backup_status)],
    })
}

/// Uninstall a single managed hook and restore user's original.
///
/// Returns a [`SingleUninstallOutcome`] describing what happened, or an error
/// if a file-system operation failed.
fn uninstall_hook(hooks_dir: &Path, hook_type: ManagedHookType) -> Result<SingleUninstallOutcome> {
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
            return Ok(SingleUninstallOutcome::Skipped);
        }
    } else if !user_backup_path.exists() {
        return Ok(SingleUninstallOutcome::NotPresent);
    }

    // Restore user's backup if it exists
    if user_backup_path.exists() {
        fs::rename(&user_backup_path, &hook_path).context("Failed to restore user hook")?;
        return Ok(SingleUninstallOutcome::Restored);
    }

    Ok(SingleUninstallOutcome::Removed)
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
        GITBUTLER_INSTALL_HOOKS_CONFIG_KEY,
        if enabled { "true" } else { "false" },
    )?;
    repo.write_local_common_config(&config)?;
    Ok(())
}

/// Resolve the hooks directory for the given repository.
///
/// Checks `core.hooksPath` in the git config first (using `trusted_path` for
/// tilde expansion and config trust validation). If not set, falls back
/// to the `hooks` subdirectory of the git directory (e.g. `.git/hooks`).
pub fn get_hooks_dir_gix(repo: &gix::Repository) -> PathBuf {
    let hooks_path = repo
        .config_snapshot()
        .trusted_path("core.hooksPath")
        .and_then(|path| path.ok().map(std::borrow::Cow::into_owned))
        .map(|path| {
            if path.is_relative() {
                let base = repo.workdir().unwrap_or(repo.git_dir());
                base.join(path)
            } else {
                path
            }
        });
    hooks_path.unwrap_or_else(|| repo.git_dir().join("hooks"))
}

/// Accumulates per-hook outcomes during a bulk install and reduces them to a
/// single [`HookInstallationResult`].
///
/// Encapsulates the five counters/vecs that `install_managed_hooks` previously
/// managed inline, making the reduction logic explicit and the loop body minimal.
#[derive(Default)]
struct HookAccumulator {
    /// Hook names that were newly installed or updated.
    installed: Vec<String>,
    /// Per-hook backup status for hooks that were installed or already configured.
    hook_results: Vec<(String, HookBackupStatus)>,
    /// Number of hooks that were already up to date (no write needed).
    already_configured_count: usize,
    /// Number of hooks that were newly written or updated.
    success_count: usize,
    /// Hook names that were skipped because they are owned by another manager.
    skipped: Vec<String>,
    /// Non-fatal error messages from hooks that failed to install.
    warnings: Vec<String>,
}

impl HookAccumulator {
    /// Record a successful hook installation.
    fn record_success(&mut self, name: String, results: Vec<(String, HookBackupStatus)>) {
        self.success_count += 1;
        self.installed.push(name);
        self.hook_results.extend(results);
    }

    /// Record a hook that was already at the correct version (no write needed).
    fn record_already_configured(&mut self, name: String) {
        self.already_configured_count += 1;
        self.installed.push(name.clone());
        self.hook_results.push((name, HookBackupStatus::None));
    }

    /// Record hooks that were skipped because they are owned by another manager.
    fn record_skipped(&mut self, names: Vec<String>) {
        self.skipped.extend(names);
    }

    /// Record a non-fatal installation error.
    fn record_warning(&mut self, msg: String) {
        self.warnings.push(msg);
    }

    /// Reduce the accumulated state into the final [`HookInstallationResult`].
    fn into_result(mut self) -> HookInstallationResult {
        // If any hooks were skipped by an external manager:
        if !self.skipped.is_empty() {
            if self.success_count > 0 || self.already_configured_count > 0 {
                // Mixed result: some installed, some owned by another manager.
                self.warnings.extend(self.skipped.iter().map(|name| {
                    format!(
                        "Skipped {name} — hook exists and is not GitButler-managed \
                         — use --force-hooks to override"
                    )
                }));
                return HookInstallationResult::PartialSuccess {
                    installed_hooks: self.installed,
                    hook_results: self.hook_results,
                    warnings: self.warnings,
                };
            }
            // All hooks were skipped. If there were also errors (e.g. one hook
            // was user-owned and another failed with an IO error), surface the
            // warnings via PartialSuccess rather than silently dropping them.
            if !self.warnings.is_empty() {
                self.warnings.extend(self.skipped.iter().map(|name| {
                    format!(
                        "Skipped {name} — hook exists and is not GitButler-managed \
                         — use --force-hooks to override"
                    )
                }));
                return HookInstallationResult::PartialSuccess {
                    installed_hooks: self.installed,
                    hook_results: self.hook_results,
                    warnings: self.warnings,
                };
            }
            return HookInstallationResult::Skipped {
                hook_names: self.skipped,
            };
        }

        if self.already_configured_count == MANAGED_HOOK_TYPES.len() && self.warnings.is_empty() {
            HookInstallationResult::AlreadyConfigured
        } else if self.warnings.is_empty() {
            HookInstallationResult::Success {
                hook_results: self.hook_results,
            }
        } else {
            HookInstallationResult::PartialSuccess {
                installed_hooks: self.installed,
                hook_results: self.hook_results,
                warnings: self.warnings,
            }
        }
    }
}

/// Install all GitButler managed hooks into the given hooks directory.
///
/// Called after switching HEAD to gitbutler/workspace.
/// The caller is responsible for resolving the hooks directory
/// (respecting `core.hooksPath` if set), e.g. via [`get_hooks_dir_gix`].
///
/// When `force` is true, existing non-GitButler hooks are backed up and
/// overwritten instead of being skipped.
pub fn install_managed_hooks(
    hooks_dir: &Path,
    force: bool,
    now: SystemTime,
) -> Result<HookInstallationResult> {
    let mut acc = HookAccumulator::default();

    for hook_type in MANAGED_HOOK_TYPES {
        match install_hook(hooks_dir, hook_type, force, now) {
            Ok(HookInstallationResult::Success { hook_results }) => {
                tracing::debug!("Installed {} hook", hook_type.hook_name());
                acc.record_success(hook_type.hook_name().to_owned(), hook_results);
            }
            Ok(HookInstallationResult::AlreadyConfigured) => {
                tracing::trace!("{} hook already configured", hook_type.hook_name());
                acc.record_already_configured(hook_type.hook_name().to_owned());
            }
            Ok(HookInstallationResult::Skipped { hook_names }) => {
                for name in &hook_names {
                    tracing::info!("{name} hook skipped — owned by external hook manager");
                }
                acc.record_skipped(hook_names);
            }
            Ok(HookInstallationResult::PartialSuccess {
                installed_hooks,
                hook_results,
                warnings,
            }) => {
                acc.installed.extend(installed_hooks);
                acc.hook_results.extend(hook_results);
                acc.warnings.extend(warnings);
            }
            Err(e) => {
                acc.record_warning(format!(
                    "Failed to install {}: {}",
                    hook_type.hook_name(),
                    e
                ));
            }
        }
    }

    Ok(acc.into_result())
}

/// Outcome of the high-level hook setup flow performed by [`ensure_managed_hooks`].
///
/// This enum captures every possible resolution so callers (CLI, app, background
/// operations) can render appropriate user-facing guidance without duplicating
/// detection / persistence logic.
#[derive(Debug, Clone)]
pub enum HookSetupOutcome {
    /// GitButler managed hooks were freshly installed (at least one new hook written).
    Installed {
        /// Per-hook results containing the hook name and backup status.
        hook_results: Vec<(String, HookBackupStatus)>,
    },
    /// All managed hooks were already installed and up to date — no changes were made.
    AlreadyInstalled,
    /// Some hooks were installed and some were skipped (user hooks already occupied those slots).
    /// Callers should surface the `warnings` and list the `installed_hooks` so users
    /// understand the partial protection state.
    PartialSuccess {
        /// Names of hooks that are now active (installed or already up to date).
        installed_hooks: Vec<String>,
        /// Per-hook results for hooks that were installed, including backup status.
        /// Hooks that were skipped are not included here.
        hook_results: Vec<(String, HookBackupStatus)>,
        /// Human-readable descriptions of each hook that was skipped and why.
        warnings: Vec<String>,
    },
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
        ///
        /// This is always `'static` because instructions come from
        /// [`crate::hook_manager::KnownManager::integration_instructions`], which returns a compile-time
        /// string literal. Using `&'static str` avoids an allocation per detection.
        instructions: &'static str,
    },
    /// One or more hook files were skipped because they exist and are not
    /// GitButler-managed, but no known external hook manager was detected.
    HookSkipped {
        /// The names of the skipped hooks (e.g. `["pre-commit", "pre-push"]`).
        hook_names: Vec<String>,
    },
    /// Hook installation failed (non-fatal).
    ///
    /// # Note on reachability
    ///
    /// This variant is currently unreachable via [`ensure_managed_hooks`] in practice:
    /// [`install_managed_hooks`] catches all per-hook errors as warnings and always
    /// returns `Ok(...)`, so the `Err(e)` arm in `ensure_managed_hooks` that produces
    /// this variant is never hit by the current implementation. The variant is kept for
    /// API completeness and to accommodate future changes where a hard failure (e.g. an
    /// unrecoverable filesystem error) would be appropriate to surface distinctly rather
    /// than as a warning.
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
    now: SystemTime,
) -> HookSetupOutcome {
    // Resolve the working directory once — used by both the config-disabled
    // re-detection path and the first-time detection path below.
    let workdir = repo
        .workdir()
        .unwrap_or_else(|| repo.git_dir())
        .to_path_buf();

    // Fast path: installation disabled by repo-local config.
    // When `force` is true the caller has explicitly requested installation,
    // so we skip the config check entirely.
    if !force && !install_managed_hooks_enabled(repo) {
        // Re-check for an external manager even though the config flag is already set.
        // This ensures that subsequent `but setup` runs after prek was first detected
        // still show the informative "Detected prek" message instead of the generic
        // "--no-hooks is configured" fallback, which doesn't tell the user *why* hooks
        // are skipped.
        if let Some((manager_name, instructions)) =
            crate::hook_manager::detect_hook_manager_in_hooks_dir(hooks_dir, &workdir)
        {
            tracing::info!(
                manager = manager_name,
                "External hook manager re-detected on subsequent setup call; returning ExternalManagerDetected."
            );
            return HookSetupOutcome::ExternalManagerDetected {
                manager_name: manager_name.to_owned(),
                instructions,
            };
        }
        tracing::info!(
            config_key = GITBUTLER_INSTALL_HOOKS_CONFIG_KEY,
            "Managed hook installation disabled via repo-local config, skipping."
        );
        return HookSetupOutcome::DisabledByConfig;
    }

    // Detect external hook manager (skip detection when forcing).
    if !force
        && let Some((manager_name, instructions)) =
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
            instructions,
        };
    }

    // No external manager (or force mode) — install managed hooks.
    match install_managed_hooks(hooks_dir, force, now) {
        Ok(HookInstallationResult::Success { hook_results }) => {
            HookSetupOutcome::Installed { hook_results }
        }
        Ok(HookInstallationResult::AlreadyConfigured) => HookSetupOutcome::AlreadyInstalled,
        Ok(HookInstallationResult::PartialSuccess {
            installed_hooks,
            hook_results,
            warnings,
        }) => HookSetupOutcome::PartialSuccess {
            installed_hooks,
            hook_results,
            warnings,
        },
        Ok(HookInstallationResult::Skipped { hook_names }) => {
            HookSetupOutcome::HookSkipped { hook_names }
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
///
/// Returns a [`HookUninstallSummary`] describing per-hook outcomes so callers
/// can produce informative user-facing output.
pub fn uninstall_managed_hooks(hooks_dir: &Path) -> Result<HookUninstallSummary> {
    let mut summary = HookUninstallSummary::default();

    for hook_type in MANAGED_HOOK_TYPES {
        match uninstall_hook(hooks_dir, hook_type) {
            Ok(SingleUninstallOutcome::Removed) => {
                tracing::debug!("Uninstalled {} hook", hook_type.hook_name());
                summary.removed.push(hook_type.hook_name().to_owned());
            }
            Ok(SingleUninstallOutcome::Restored) => {
                tracing::debug!(
                    "Uninstalled {} hook; restored user backup",
                    hook_type.hook_name()
                );
                summary.restored.push(hook_type.hook_name().to_owned());
            }
            Ok(SingleUninstallOutcome::Skipped | SingleUninstallOutcome::NotPresent) => {}
            Err(e) => {
                summary.warnings.push(format!(
                    "Failed to uninstall {}: {}",
                    hook_type.hook_name(),
                    e
                ));
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests;
