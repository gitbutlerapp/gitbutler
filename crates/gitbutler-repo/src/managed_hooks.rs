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
    /// Installation was skipped because the user opted out via `gitbutler.installHooks=false`
    SkippedByConfig,
    /// Installation partially succeeded with warnings
    PartialSuccess { warnings: Vec<String> },
}

/// Subdirectory of the common git directory where GitButler stores its hook scripts
/// when hooks are registered through git config (git 2.54+). Git never reads this
/// location on its own, so nothing here can shadow or displace user hooks.
const CONFIG_HOOK_SCRIPTS_SUBDIR: &str = "gitbutler/hooks";

/// Local git config key that lets users opt out of hook installation entirely.
const INSTALL_HOOKS_CONFIG_KEY: &str = "gitbutler.installHooks";

/// Pre-commit hook script content for config-based hooks.
///
/// Much simpler than [`PRE_COMMIT_HOOK_SCRIPT`]: since the script never occupies a
/// slot in the hooks directory, there is no user hook to back up, chain, or restore.
const CONFIG_PRE_COMMIT_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook is managed by GitButler and registered via git config (hook.gitbutler-pre-commit).
# It does not occupy .git/hooks, so your own hooks and hook managers are left untouched.

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
    echo "If you no longer have the GitButler CLI installed, you can remove this hook with:"
    echo "  git config --remove-section hook.gitbutler-pre-commit"
    echo ""
    exit 1
fi

exit 0
"#;

/// Post-checkout hook script content for config-based hooks.
///
/// Cleans up GitButler's config entries and scripts when the user checks out away
/// from `gitbutler/workspace`. No user hook restoration is needed because nothing
/// was displaced in the first place. Also sweeps any legacy file-based GitButler
/// hooks that a prior migration to config-based hooks failed to clean up, so a
/// partially-migrated repo still ends up fully torn down.
const CONFIG_POST_CHECKOUT_HOOK_SCRIPT: &str = r#"#!/bin/sh
# GITBUTLER_MANAGED_HOOK_V1
# This hook is managed by GitButler and registered via git config (hook.gitbutler-post-checkout).
# It auto-removes GitButler's hooks when you checkout away from gitbutler/workspace.

# post-checkout receives: $1 previous HEAD, $2 new HEAD, $3 branch-checkout flag.
PREV_HEAD=$1
BRANCH_CHECKOUT=$3

# Only act on branch checkouts (not file checkouts)
if [ "$BRANCH_CHECKOUT" != "1" ]; then
    exit 0
fi

NEW_BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null)

# If we just left gitbutler/workspace (and aren't coming back to it). @{-1}
# is the branch that was checked out before this checkout, from git's own
# record (the HEAD reflog, on by default in non-bare repos). Comparing
# commit ids instead would misfire when another ref happens to point at the
# same commit, or when a concurrent GitButler process moves the workspace
# ref between checkout start and hook execution.
PREV_BRANCH=$(git rev-parse --symbolic-full-name @{-1} 2>/dev/null)
case "$PREV_BRANCH" in
    refs/*)
        # Resolved to a real ref: trust it.
        ;;
    *)
        # @{-1} gave no branch. Without a reflog
        # (core.logAllRefUpdates=false or logs pruned) rev-parse fails and
        # echoes the literal "@{-1}"; after a detached checkout it prints
        # nothing. Fall back to comparing the previous HEAD commit against
        # the workspace ref. Weaker (a coinciding ref or a concurrent ref
        # move can confuse it), but far better than never cleaning up.
        WORKSPACE_HEAD=$(git rev-parse --verify -q refs/heads/gitbutler/workspace 2>/dev/null)
        if [ -n "$WORKSPACE_HEAD" ] && [ "$WORKSPACE_HEAD" = "$PREV_HEAD" ]; then
            PREV_BRANCH="refs/heads/gitbutler/workspace"
        fi
        ;;
esac
if [ "$PREV_BRANCH" = "refs/heads/gitbutler/workspace" ]; then
    if [ "$NEW_BRANCH" != "gitbutler/workspace" ]; then
        echo ""
        echo "NOTE: You have left GitButler's managed workspace branch."
        echo "Cleaning up GitButler hooks..."

        SCRIPTS_DIR=$(dirname "$0")

        # Only delete a script once its config registration is confirmed
        # gone: if `--remove-section` fails (lock contention, permissions),
        # leaving the script in place keeps the hook consistent (still
        # working) instead of a config entry pointing at a deleted script.
        git config --remove-section hook.gitbutler-pre-commit 2>/dev/null
        if ! git config --get hook.gitbutler-pre-commit.command >/dev/null 2>&1; then
            rm -f "$SCRIPTS_DIR/pre-commit"
        fi

        git config --remove-section hook.gitbutler-post-checkout 2>/dev/null
        if ! git config --get hook.gitbutler-post-checkout.command >/dev/null 2>&1; then
            rm -f "$SCRIPTS_DIR/post-checkout"
        fi

        rmdir "$SCRIPTS_DIR" 2>/dev/null

        # A prior install may have failed to fully clean up a legacy
        # file-based installation while migrating to config-based hooks
        # (e.g. a filesystem error). Sweep any stranded GitButler-signed
        # hooks left in the hooks directory.
        LEGACY_HOOKS_DIR=$(git rev-parse --path-format=absolute --git-path hooks 2>/dev/null)

        # Legacy pre-commit is a different event: safe to remove outright.
        legacy_pre_commit="$LEGACY_HOOKS_DIR/pre-commit"
        if [ -f "$legacy_pre_commit" ] && grep -q "GITBUTLER_MANAGED_HOOK_V1" "$legacy_pre_commit" 2>/dev/null; then
            rm -f "$legacy_pre_commit"
        fi
        # Put the user's original back if the old file-based installer left
        # its backup behind and nothing else took the hook's place.
        if [ -f "$LEGACY_HOOKS_DIR/pre-commit-user" ] && [ ! -e "$legacy_pre_commit" ]; then
            mv "$LEGACY_HOOKS_DIR/pre-commit-user" "$legacy_pre_commit"
        fi

        # Legacy post-checkout is *this same event's* own hooks-directory
        # hook: git already resolved to run it right after this config hook
        # returns, so deleting the file out from under that exec would make
        # git fail with "cannot exec: No such file or directory". Neuter its
        # content in place instead: the path still exists for git to exec,
        # it just does nothing now, and it keeps the signature so the next
        # `but setup` still recognizes and removes it for good.
        legacy_post_checkout="$LEGACY_HOOKS_DIR/post-checkout"
        if [ -f "$legacy_post_checkout" ] && grep -q "GITBUTLER_MANAGED_HOOK_V1" "$legacy_post_checkout" 2>/dev/null; then
            if [ -f "$LEGACY_HOOKS_DIR/post-checkout-user" ]; then
                # Swap the user's original back in atomically: the path git
                # resolved for its upcoming hooks-directory exec stays valid,
                # and the user's own post-checkout runs for this checkout --
                # just as it would have without GitButler in the way.
                mv "$LEGACY_HOOKS_DIR/post-checkout-user" "$legacy_post_checkout"
            else
                printf '#!/bin/sh\n# GITBUTLER_MANAGED_HOOK_V1\nexit 0\n' > "$legacy_post_checkout"
            fi
        elif [ -f "$LEGACY_HOOKS_DIR/post-checkout-user" ] && [ ! -e "$legacy_post_checkout" ]; then
            mv "$LEGACY_HOOKS_DIR/post-checkout-user" "$legacy_post_checkout"
        fi

        echo ""
        echo "To return to GitButler mode, run: but setup"
        echo ""
    fi
fi

exit 0
"#;

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

    /// Subsection name under which this hook is registered in git config (git 2.54+).
    fn config_section(&self) -> &'static str {
        match self {
            Self::PreCommit => "gitbutler-pre-commit",
            Self::PostCheckout => "gitbutler-post-checkout",
        }
    }

    /// Script content used when this hook is registered through git config.
    fn config_script_content(&self) -> &'static str {
        match self {
            Self::PreCommit => CONFIG_PRE_COMMIT_HOOK_SCRIPT,
            Self::PostCheckout => CONFIG_POST_CHECKOUT_HOOK_SCRIPT,
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

/// Whether two paths refer to the same location, resolving symlinks where
/// possible and falling back to a literal comparison otherwise.
fn is_same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
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
    #[cfg(not(unix))]
    let _ = path;
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

/// Install all GitButler managed hooks
///
/// Called after switching HEAD to gitbutler/workspace.
///
/// With git 2.54+ on `PATH`, hooks are registered through git config
/// (`hook.<name>.event` / `hook.<name>.command`) and their scripts live in a
/// GitButler-owned directory — the hooks directory and any hooks in it (whether
/// the user's own or a hook manager's) are never touched. Older gits fall back
/// to the previous file-based installation.
pub fn install_managed_hooks(repo: &gix::Repository) -> Result<HookInstallationResult> {
    install_managed_hooks_with(repo, git_on_path_supports_config_hooks())
}

/// Like [`install_managed_hooks`], but with explicit control over whether
/// config-based hooks (git 2.54+) are used instead of the hooks directory.
pub fn install_managed_hooks_with(
    repo: &gix::Repository,
    use_config_hooks: bool,
) -> Result<HookInstallationResult> {
    if hooks_opted_out(repo) {
        // Opting out must actually remove any hooks a previous install left
        // behind, not just skip installing new ones -- otherwise a user who
        // opts out after the fact keeps being blocked by stale hooks.
        return match uninstall_managed_hooks(repo) {
            partial @ Ok(HookInstallationResult::PartialSuccess { .. }) => partial,
            Ok(_) => Ok(HookInstallationResult::SkippedByConfig),
            Err(e) => {
                Err(e.context("Failed to remove hooks after opting out of hook installation"))
            }
        };
    }
    if use_config_hooks {
        install_config_based_hooks(repo)
    } else {
        install_file_based_hooks(repo)
    }
}

/// Install the hooks into the hooks directory, dropping any config-based
/// registration a previous install left behind.
///
/// Every path that ends up here can be reached on a repository that was
/// previously installed through config — the git on `PATH` losing 2.54 support
/// is enough — and leaving the registrations in place would run each hook
/// twice: once via config, once via the hooks directory.
fn install_file_based_hooks(repo: &gix::Repository) -> Result<HookInstallationResult> {
    uninstall_config_based_hooks(repo)?;
    install_managed_hooks_at(&get_hooks_dir(repo))
}

/// Register GitButler's hooks through git config (git 2.54+).
///
/// Scripts are written to a GitButler-owned directory inside the common git dir
/// and referenced from local config. The hooks directory is left alone, so user
/// hooks and hook-manager hooks (prek, pre-commit, husky, ...) keep working —
/// git runs config-based hooks first and the hooks-directory hook afterwards.
fn install_config_based_hooks(repo: &gix::Repository) -> Result<HookInstallationResult> {
    // Registering through config means serializing the script path into
    // `hook.<section>.command`. On Unix `sh_quote` passes raw bytes through
    // losslessly, but elsewhere (Windows) a path that isn't valid Unicode has
    // no lossless byte encoding, so the config value would be corrupted and
    // the hook would silently never run. Fall back to file-based hooks, which
    // never embed a path anywhere.
    #[cfg(not(unix))]
    if repo.common_dir().to_str().is_none() {
        return install_file_based_hooks(repo);
    }

    let scripts_dir = config_scripts_dir(repo);
    fs::create_dir_all(&scripts_dir)
        .context("Failed to create GitButler hook scripts directory")?;

    // If `core.hooksPath` points at our scripts directory, writing scripts
    // there would overwrite the user's hooks in place (without a backup), and
    // the legacy-hook sweep below would then remove our own signed scripts
    // again. Fall back to the file-based mechanism, which backs up and
    // restores whatever lives in the hooks directory.
    if is_same_path(&get_hooks_dir(repo), &scripts_dir) {
        return install_file_based_hooks(repo);
    }

    let mut changed = false;
    let mut warnings = Vec::new();

    for hook_type in ManagedHookType::ALL {
        let script_path = scripts_dir.join(hook_type.hook_name());
        let content = hook_type.config_script_content();
        if fs::read_to_string(&script_path).ok().as_deref() != Some(content) {
            fs::write(&script_path, content).context("Failed to write hook script")?;
            changed = true;
        }
        set_hook_executable(&script_path)?;
    }

    // `edit_repo_config` reads the config file fresh from disk and only writes it
    // back if the edit changed anything, so this is naturally idempotent.
    let wrote_config =
        but_core::git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
            for hook_type in ManagedHookType::ALL {
                let section = hook_type.config_section();
                let script_path = scripts_dir.join(hook_type.hook_name());
                config.set_raw_value(
                    format!("hook.{section}.event").as_str(),
                    hook_type.hook_name(),
                )?;
                let quoted_path = sh_quote(&script_path);
                config.set_raw_value(
                    format!("hook.{section}.command").as_str(),
                    quoted_path.as_slice(),
                )?;
            }
            Ok(())
        })?;
    changed |= wrote_config;

    // Clean up hooks that an older GitButler version installed into the hooks
    // directory, restoring any displaced user hook from its backup.
    match uninstall_managed_hooks_at(&get_hooks_dir(repo)) {
        Ok(HookInstallationResult::PartialSuccess { warnings: w }) => warnings.extend(w),
        Ok(_) => {}
        Err(e) => warnings.push(format!("Failed to clean up legacy GitButler hooks: {e}")),
    }

    if !warnings.is_empty() {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    } else if changed {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::AlreadyConfigured)
    }
}

/// `true` if the user has opted out of hook installation via
/// `gitbutler.installHooks=false`.
fn hooks_opted_out(repo: &gix::Repository) -> bool {
    repo.config_snapshot()
        .boolean(INSTALL_HOOKS_CONFIG_KEY)
        .is_some_and(|enabled| !enabled)
}

/// `true` if the `git` binary on `PATH` supports config-based hooks (git 2.54+).
///
/// The version that actually matters is that of whatever `git` binary the user
/// invokes later -- hooks run under it, not under the gitoxide library
/// GitButler links. That binary is unknowable at install time, so probing
/// GitButler's own `PATH` is the best available proxy. The accepted residual
/// risk: a user whose shell resolves an older `git` than GitButler's will have
/// config-based hooks registered that their git silently ignores, leaving the
/// workspace branch unguarded -- the same exposure as opting out of hooks, and
/// one that disappears as soon as their git reaches 2.54.
fn git_on_path_supports_config_hooks() -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| parse_git_version(&String::from_utf8_lossy(&out.stdout)))
            .is_some_and(|version| version >= (2, 54))
    })
}

/// Parse output like `git version 2.54.0` or `git version 2.39.5 (Apple Git-154)`
/// into `(major, minor)`.
fn parse_git_version(output: &str) -> Option<(u32, u32)> {
    let rest = output.trim().strip_prefix("git version ")?;
    let mut numbers = rest.split(|c: char| !c.is_ascii_digit());
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    Some((major, minor))
}

/// Single-quote a path so git can run it as a shell command from config.
///
/// Works at the byte level rather than through `Path::display()`, which
/// lossily replaces invalid UTF-8 with `U+FFFD` -- that would silently
/// corrupt the `hook.<section>.command` value for a repository living at a
/// non-UTF-8 path on Unix.
fn sh_quote(path: &Path) -> bstr::BString {
    #[cfg(unix)]
    let bytes: Vec<u8> = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    // Lossy is acceptable here: `install_config_based_hooks` falls back to
    // file-based hooks for non-Unicode paths on non-Unix, so this branch only
    // ever sees paths where `to_string_lossy` is in fact lossless.
    #[cfg(not(unix))]
    let bytes: Vec<u8> = path.to_string_lossy().into_owned().into_bytes();

    let mut quoted = Vec::with_capacity(bytes.len() + 2);
    quoted.push(b'\'');
    for byte in bytes {
        if byte == b'\'' {
            quoted.extend_from_slice(b"'\\''");
        } else {
            quoted.push(byte);
        }
    }
    quoted.push(b'\'');
    bstr::BString::from(quoted)
}

fn install_managed_hooks_at(hooks_dir: &Path) -> Result<HookInstallationResult> {
    let mut warnings = Vec::new();
    let mut already_configured_count = 0;

    for hook_type in ManagedHookType::ALL {
        match install_hook(hooks_dir, hook_type) {
            // Never produced by install_hook; needed for exhaustiveness.
            Ok(HookInstallationResult::SkippedByConfig) => {}
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
                warnings.push(format!(
                    "Failed to install {}: {}",
                    hook_type.hook_name(),
                    e
                ));
            }
        }
    }

    // If all hooks were already configured, return AlreadyConfigured
    if already_configured_count == ManagedHookType::ALL.len() && warnings.is_empty() {
        Ok(HookInstallationResult::AlreadyConfigured)
    } else if warnings.is_empty() {
        Ok(HookInstallationResult::Success)
    } else {
        Ok(HookInstallationResult::PartialSuccess { warnings })
    }
}

/// Uninstall all GitButler managed hooks and restore user's originals
///
/// Called during teardown. Cleans up both installation mechanisms regardless of
/// the current git version: config-based registrations (git 2.54+) and hooks in
/// the hooks directory from older installs.
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
            // Never produced by uninstall_hook; needed for exhaustiveness.
            Ok(HookInstallationResult::SkippedByConfig) => {}
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

#[cfg(test)]
mod tests {
    use super::{parse_git_version, sh_quote};

    #[test]
    fn parses_plain_git_version() {
        assert_eq!(parse_git_version("git version 2.54.0\n"), Some((2, 54)));
    }

    #[test]
    fn parses_apple_git_version() {
        assert_eq!(
            parse_git_version("git version 2.39.5 (Apple Git-154)\n"),
            Some((2, 39))
        );
    }

    #[test]
    fn parses_windows_git_version() {
        assert_eq!(
            parse_git_version("git version 2.54.0.windows.1\n"),
            Some((2, 54))
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_git_version("not git"), None);
        assert_eq!(parse_git_version(""), None);
        assert_eq!(parse_git_version("git version x.y"), None);
    }

    /// `Path::display()` (used internally by the old implementation of
    /// `sh_quote`) lossily replaces invalid UTF-8 bytes with `U+FFFD`. On Unix,
    /// a repository living at a non-UTF-8 path would therefore get a mangled
    /// `hook.<section>.command` config value that git can't execute. The
    /// quoted value must preserve the exact original bytes.
    #[cfg(unix)]
    #[test]
    fn sh_quote_preserves_non_utf8_path_bytes() {
        use bstr::ByteSlice;
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // 0xFF is never valid UTF-8 on its own.
        let raw: &[u8] = b"/tmp/repo-\xFF/gitbutler/hooks/pre-commit";
        let path = std::path::Path::new(OsStr::from_bytes(raw));

        let quoted = sh_quote(path);
        assert!(
            quoted
                .as_bytes()
                .windows(raw.len())
                .any(|window| window == raw),
            "quoted path should preserve the exact non-UTF-8 byte sequence, got {quoted:?}"
        );
    }
}
