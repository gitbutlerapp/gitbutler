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

/// Marker comment to identify GitButler-managed hooks
const GITBUTLER_HOOK_SIGNATURE: &str = "# GITBUTLER_MANAGED_HOOK_V1";

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

/// Result of hook installation/uninstallation
#[derive(Debug, Clone)]
pub enum HookInstallationResult {
    /// Hook was successfully installed/uninstalled
    Success,
    /// Hook was already in the desired state
    AlreadyConfigured,
    /// Installation partially succeeded with warnings
    PartialSuccess { warnings: Vec<String> },
}

/// Types of hooks we manage
#[derive(Debug, Clone, Copy)]
enum ManagedHookType {
    PreCommit,
    PostCheckout,
}

impl ManagedHookType {
    fn hook_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::PostCheckout => "post-checkout",
        }
    }

    fn user_backup_name(&self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit-user",
            Self::PostCheckout => "post-checkout-user",
        }
    }

    fn script_content(&self) -> &'static str {
        match self {
            Self::PreCommit => PRE_COMMIT_HOOK_SCRIPT,
            Self::PostCheckout => POST_CHECKOUT_HOOK_SCRIPT,
        }
    }
}

/// Get the hooks directory, respecting core.hooksPath configuration
fn get_hooks_dir(repo: &git2::Repository) -> PathBuf {
    repo.config()
        .and_then(|config| config.get_path("core.hooksPath"))
        .unwrap_or_else(|_| repo.path().join("hooks"))
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
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).context("Failed to set hook as executable")?;
    }
    Ok(())
}

/// Install a single managed hook
fn install_hook(hooks_dir: &Path, hook_type: ManagedHookType) -> Result<HookInstallationResult> {
    let hook_path = hooks_dir.join(hook_type.hook_name());
    let user_backup_path = hooks_dir.join(hook_type.user_backup_name());

    // Create hooks directory if it doesn't exist
    if !hooks_dir.exists() {
        fs::create_dir_all(hooks_dir).context("Failed to create hooks directory")?;
    }

    // Check if our hook is already installed
    if hook_path.exists() && is_gitbutler_managed_hook(&hook_path) {
        return Ok(HookInstallationResult::AlreadyConfigured);
    }

    // Backup existing hook if it exists and backup doesn't already exist
    if hook_path.exists() && !user_backup_path.exists() {
        fs::rename(&hook_path, &user_backup_path).context("Failed to backup existing hook")?;
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
            tracing::debug!("{} is not GitButler-managed, skipping", hook_type.hook_name());
            return Ok(HookInstallationResult::AlreadyConfigured);
        }
    }

    // Restore user's backup if it exists
    if user_backup_path.exists() {
        fs::rename(&user_backup_path, &hook_path).context("Failed to restore user hook")?;
    }

    Ok(HookInstallationResult::Success)
}

/// Install all GitButler managed hooks
///
/// Called after switching HEAD to gitbutler/workspace
pub fn install_managed_hooks(repo: &git2::Repository) -> Result<HookInstallationResult> {
    let hooks_dir = get_hooks_dir(repo);
    let mut warnings = Vec::new();
    let mut already_configured_count = 0;

    for hook_type in [ManagedHookType::PreCommit, ManagedHookType::PostCheckout] {
        match install_hook(&hooks_dir, hook_type) {
            Ok(HookInstallationResult::Success) => {
                tracing::debug!("Installed {} hook", hook_type.hook_name());
            }
            Ok(HookInstallationResult::AlreadyConfigured) => {
                tracing::trace!("{} hook already configured", hook_type.hook_name());
                already_configured_count += 1;
            }
            Ok(HookInstallationResult::PartialSuccess { warnings: w }) => {
                warnings.extend(w);
            }
            Err(e) => {
                warnings.push(format!("Failed to install {}: {}", hook_type.hook_name(), e));
            }
        }
    }

    // If all hooks were already configured, return AlreadyConfigured
    if already_configured_count == 2 && warnings.is_empty() {
        Ok(HookInstallationResult::AlreadyConfigured)
    } else if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}

/// Uninstall all GitButler managed hooks and restore user's originals
///
/// Called during teardown
pub fn uninstall_managed_hooks(repo: &git2::Repository) -> Result<HookInstallationResult> {
    let hooks_dir = get_hooks_dir(repo);
    let mut warnings = Vec::new();

    for hook_type in [ManagedHookType::PreCommit, ManagedHookType::PostCheckout] {
        match uninstall_hook(&hooks_dir, hook_type) {
            Ok(HookInstallationResult::Success) => {
                tracing::debug!("Uninstalled {} hook", hook_type.hook_name());
            }
            Ok(HookInstallationResult::AlreadyConfigured) => {}
            Ok(HookInstallationResult::PartialSuccess { warnings: w }) => {
                warnings.extend(w);
            }
            Err(e) => {
                warnings.push(format!("Failed to uninstall {}: {}", hook_type.hook_name(), e));
            }
        }
    }

    if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}
