//! Implementation of `but hook` subcommands.
//!
//! These commands provide GitButler's workspace guard and cleanup logic
//! as standalone CLI commands that any external hook manager can invoke.

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::utils::OutputChannel;

/// Run the pre-commit workspace guard.
///
/// Blocks direct `git commit` on the `gitbutler/workspace` branch with a
/// helpful error message. Exits 0 (allow) on any other branch or if the
/// repository cannot be opened.
///
/// Uses `gix::discover()` directly because this command is invoked by git as a
/// hook subprocess — there is no `Context` available in this execution path.
pub fn pre_commit(out: &mut OutputChannel, current_dir: &std::path::Path) -> Result<()> {
    let repo = match gix::discover(current_dir) {
        Ok(repo) => repo,
        Err(_) => {
            // Not in a git repo — allow the commit (shouldn't happen from a hook, but be safe)
            return Ok(());
        }
    };

    let branch_name = match repo.head() {
        Ok(head) => head
            .referent_name()
            .map(|n| n.shorten().to_string())
            .unwrap_or_default(),
        Err(_) => return Ok(()), // Detached HEAD or error — allow
    };

    if branch_name == "gitbutler/workspace" {
        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GITBUTLER_ERROR: Cannot commit directly to gitbutler/workspace branch."
                    .red()
                    .bold()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GitButler manages commits on this branch. Please use GitButler to commit your changes:"
                    .dimmed()
            )?;
            writeln!(
                out,
                "  {}",
                "- Use the GitButler app to create commits".dimmed()
            )?;
            writeln!(
                out,
                "  {}",
                "- Or run 'but commit' from the command line".dimmed()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "If you want to exit GitButler mode and use normal git:".dimmed()
            )?;
            writeln!(
                out,
                "  {}",
                "- Run 'but teardown' to switch to a regular branch".dimmed()
            )?;
            writeln!(
                out,
                "  {}",
                "- Or directly checkout another branch: git checkout <branch>".dimmed()
            )?;
            writeln!(out)?;
        }
        // Exit with error to block the commit
        anyhow::bail!("Cannot commit directly to gitbutler/workspace branch");
    }

    Ok(())
}

/// Run the post-checkout notification logic for hook-manager users.
///
/// When leaving the `gitbutler/workspace` branch (branch checkout only),
/// prints an informational message directing the user to `but setup`.
/// On file checkouts or when staying on workspace, does nothing.
///
/// # Difference from the shell-based post-checkout hook
///
/// The shell hook installed by `install_managed_hooks` also **uninstalls**
/// GitButler's managed hooks when leaving workspace (since GitButler owns
/// them). This command intentionally does **not** uninstall anything because
/// it runs inside a hook manager (prek, husky, etc.) that owns the hook
/// files — there are no GitButler-managed hooks to remove.
///
/// Uses `gix::discover()` directly because this command is invoked by git as a
/// hook subprocess — there is no `Context` available in this execution path.
pub fn post_checkout(
    out: &mut OutputChannel,
    current_dir: &std::path::Path,
    prev_head: &str,
    _new_head: &str,
    is_branch_checkout: &str,
) -> Result<()> {
    // Only act on branch checkouts (not file checkouts)
    if is_branch_checkout != "1" {
        return Ok(());
    }

    let repo = match gix::discover(current_dir) {
        Ok(repo) => repo,
        Err(_) => return Ok(()),
    };

    // Check if we actually left gitbutler/workspace by comparing prev_head
    // against the workspace ref. This mirrors the shell script logic and
    // avoids printing spurious messages on unrelated branch checkouts.
    let workspace_head = repo
        .find_reference("refs/heads/gitbutler/workspace")
        .ok()
        .and_then(|r| r.into_fully_peeled_id().ok())
        .map(|id| id.to_hex().to_string());

    let left_workspace = workspace_head.as_deref().is_some_and(|ws| ws == prev_head);

    if !left_workspace {
        return Ok(());
    }

    let new_branch = match repo.head() {
        Ok(head) => head
            .referent_name()
            .map(|n| n.shorten().to_string())
            .unwrap_or_default(),
        Err(_) => return Ok(()),
    };

    // If we're still on gitbutler/workspace (e.g. same-branch checkout), nothing to report
    if new_branch == "gitbutler/workspace" {
        return Ok(());
    }

    // When called via `but hook post-checkout`, the hook manager owns the
    // hooks — GitButler didn't install managed hooks, so there's nothing
    // to uninstall. Just inform the user they left workspace mode.
    if let Some(out) = out.for_human() {
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            "NOTE: You have left GitButler's managed workspace branch.".cyan()
        )?;
        writeln!(
            out,
            "{}",
            "To return to GitButler mode, run: but setup".blue()
        )?;
        writeln!(out)?;
    }

    Ok(())
}

/// Run the pre-push workspace guard.
///
/// Blocks `git push` when on the `gitbutler/workspace` branch with a
/// helpful error message. Exits 0 (allow) on any other branch or if the
/// repository cannot be opened.
///
/// The `_remote_name` and `_remote_url` parameters match git's pre-push
/// hook signature but are not inspected — the guard decision is based
/// solely on the current branch name.
///
/// Uses `gix::discover()` directly because this command is invoked by git as a
/// hook subprocess — there is no `Context` available in this execution path.
pub fn pre_push(
    out: &mut OutputChannel,
    current_dir: &std::path::Path,
    _remote_name: &str,
    _remote_url: &str,
) -> Result<()> {
    let repo = match gix::discover(current_dir) {
        Ok(repo) => repo,
        Err(_) => {
            // Not in a git repo — allow the push (shouldn't happen from a hook, but be safe)
            return Ok(());
        }
    };

    let branch_name = match repo.head() {
        Ok(head) => head
            .referent_name()
            .map(|n| n.shorten().to_string())
            .unwrap_or_default(),
        Err(_) => return Ok(()), // Detached HEAD or error — allow
    };

    if branch_name == "gitbutler/workspace" {
        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "GITBUTLER_ERROR: Cannot push the gitbutler/workspace branch."
                    .red()
                    .bold()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "The workspace branch is a synthetic branch managed by GitButler.".dimmed()
            )?;
            writeln!(
                out,
                "{}",
                "Pushing it to a remote would publish GitButler's internal state.".dimmed()
            )?;
            writeln!(out)?;
            writeln!(out, "{}", "To push your branches, use:".dimmed())?;
            writeln!(out, "  {}", "- The GitButler app to push branches".dimmed())?;
            writeln!(
                out,
                "  {}",
                "- Or run 'but push' from the command line".dimmed()
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                "If you want to exit GitButler mode and push normally:".dimmed()
            )?;
            writeln!(
                out,
                "  {}",
                "- Run 'but teardown' to switch to a regular branch".dimmed()
            )?;
            writeln!(out)?;
        }
        // Exit with error to block the push
        anyhow::bail!("Cannot push the gitbutler/workspace branch");
    }

    Ok(())
}

/// The integration mode for GitButler hooks in this repository.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum HookMode {
    /// GitButler manages hooks directly in the hooks directory.
    Managed,
    /// An external hook manager (e.g. prek) owns the hook files.
    External,
    /// Hook installation is disabled via `gitbutler.installHooks = false`.
    Disabled,
    /// No hooks are installed and no configuration has been set.
    Unconfigured,
}

impl std::fmt::Display for HookMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Managed => write!(f, "GitButler-managed"),
            Self::External => write!(f, "external hook manager"),
            Self::Disabled => write!(f, "disabled"),
            Self::Unconfigured => write!(f, "unconfigured"),
        }
    }
}

/// Ownership classification for a single hook file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum HookOwner {
    /// The hook file is managed by GitButler (contains the V1 signature).
    Gitbutler,
    /// The hook file is owned by a known external hook manager.
    External {
        /// Name of the detected manager.
        manager: String,
    },
    /// The hook file exists but is not recognized as GitButler or a known manager.
    User,
    /// No hook file exists at this path.
    Missing,
}

impl std::fmt::Display for HookOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gitbutler => write!(f, "GitButler-managed"),
            Self::External { manager } => write!(f, "external ({manager})"),
            Self::User => write!(f, "user hook"),
            Self::Missing => write!(f, "not installed"),
        }
    }
}

/// Status of a single hook file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SingleHookStatus {
    /// The hook name (e.g. "pre-commit").
    name: String,
    /// Whether the hook file exists on disk.
    exists: bool,
    /// Who owns this hook file.
    owner: HookOwner,
}

/// Full result of the `but hook status` diagnostic command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookStatusResult {
    /// The resolved hooks directory path.
    hooks_path: String,
    /// Whether `core.hooksPath` is set (non-default).
    custom_hooks_path: bool,
    /// Value of `gitbutler.installHooks` config key (`true` when not set).
    config_enabled: bool,
    /// The detected hook integration mode.
    mode: HookMode,
    /// Name of the detected external hook manager, if any.
    external_manager: Option<String>,
    /// Per-hook status for each managed hook type.
    hooks: Vec<SingleHookStatus>,
    /// Warning messages (e.g. orphaned hooks).
    warnings: Vec<String>,
    /// Recommended next actions.
    recommendations: Vec<String>,
}

/// Show hook ownership and integration state for the current repository.
///
/// Inspects the hooks directory, `gitbutler.installHooks` config, and
/// hook file contents to report a diagnostic summary. Produces human,
/// JSON, and shell output.
///
/// Uses `gix::discover()` directly because this runs as a standalone
/// CLI diagnostic — there is no `Context` available.
pub fn status(out: &mut OutputChannel, current_dir: &std::path::Path) -> Result<()> {
    let repo = gix::discover(current_dir)?;

    let hooks_dir = but_hooks::managed_hooks::resolve_hooks_dir(&repo);
    let default_hooks_dir = repo.git_dir().join("hooks");
    let custom_hooks_path = hooks_dir != default_hooks_dir;
    let config_enabled = but_hooks::managed_hooks::install_managed_hooks_enabled(&repo);

    let workdir = repo
        .workdir()
        .unwrap_or_else(|| repo.git_dir())
        .to_path_buf();

    // Per-hook status
    let mut hooks = Vec::new();
    let mut has_gitbutler_hook = false;
    let mut external_manager_name: Option<String> = None;

    for hook_name in but_hooks::hook_manager::MANAGED_HOOK_NAMES {
        let hook_path = hooks_dir.join(hook_name);
        let (exists, owner) = if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
            if content.contains("GITBUTLER_MANAGED_HOOK_V1") {
                has_gitbutler_hook = true;
                (true, HookOwner::Gitbutler)
            } else if let Some(manager) =
                but_hooks::hook_manager::detect_hook_manager(&content, &workdir)
            {
                external_manager_name = Some(manager.name().to_owned());
                (
                    true,
                    HookOwner::External {
                        manager: manager.name().to_owned(),
                    },
                )
            } else {
                (true, HookOwner::User)
            }
        } else {
            (false, HookOwner::Missing)
        };

        hooks.push(SingleHookStatus {
            name: (*hook_name).to_owned(),
            exists,
            owner,
        });
    }

    // Determine mode
    // External check comes first: `but setup` sets `installHooks = false` when it
    // detects an external manager, so `!config_enabled` alone would mask the real reason.
    let mode = if external_manager_name.is_some() {
        HookMode::External
    } else if !config_enabled {
        HookMode::Disabled
    } else if has_gitbutler_hook {
        HookMode::Managed
    } else {
        HookMode::Unconfigured
    };

    // Warnings
    let mut warnings = Vec::new();
    if custom_hooks_path && but_hooks::managed_hooks::has_managed_hooks_in(&default_hooks_dir) {
        warnings.push(format!(
            "Orphaned GitButler-managed hooks found in {} (core.hooksPath points elsewhere)",
            default_hooks_dir.display()
        ));
    }

    // Recommendations
    let mut recommendations = Vec::new();
    match &mode {
        HookMode::Disabled => {
            recommendations
                .push("Run `but setup --force-hooks` to re-enable GitButler-managed hooks.".into());
        }
        HookMode::External => {
            if let Some(ref mgr) = external_manager_name {
                recommendations.push(format!(
                    "Hooks are managed by {mgr}. Use `but hook pre-commit` etc. in your {mgr} config."
                ));
            }
        }
        HookMode::Unconfigured => {
            recommendations.push("Run `but setup` to install GitButler hooks.".into());
        }
        HookMode::Managed => {}
    }
    if !warnings.is_empty() {
        recommendations.push(
            "Remove orphaned hooks: rm .git/hooks/pre-commit .git/hooks/post-checkout .git/hooks/pre-push".into(),
        );
    }

    let result = HookStatusResult {
        hooks_path: hooks_dir.display().to_string(),
        custom_hooks_path,
        config_enabled,
        mode,
        external_manager: external_manager_name,
        hooks,
        warnings,
        recommendations,
    };

    // Human output
    if let Some(out) = out.for_human() {
        writeln!(out)?;
        writeln!(out, "{}", "Hook status".cyan().bold())?;
        writeln!(out)?;
        writeln!(
            out,
            "  {:<20} {}",
            "Hooks path:".dimmed(),
            result.hooks_path
        )?;
        if result.custom_hooks_path {
            writeln!(
                out,
                "  {:<20} {}",
                "".dimmed(),
                "(set via core.hooksPath)".dimmed()
            )?;
        }
        writeln!(
            out,
            "  {:<20} gitbutler.installHooks = {}",
            "Config:".dimmed(),
            if result.config_enabled {
                "true"
            } else {
                "false"
            }
        )?;
        writeln!(out, "  {:<20} {}", "Mode:".dimmed(), result.mode)?;
        if let Some(ref mgr) = result.external_manager {
            writeln!(out, "  {:<20} {}", "Hook manager:".dimmed(), mgr)?;
        }
        writeln!(out)?;

        for hook in &result.hooks {
            let status_str = if hook.exists {
                match &hook.owner {
                    HookOwner::Gitbutler => format!("✓ {}", hook.owner).green().to_string(),
                    HookOwner::External { .. } => format!("● {}", hook.owner).cyan().to_string(),
                    HookOwner::User => format!("○ {}", hook.owner).yellow().to_string(),
                    HookOwner::Missing => unreachable!(),
                }
            } else {
                format!("✗ {}", hook.owner).dimmed().to_string()
            };
            writeln!(
                out,
                "  {:<20} {}",
                format!("{}:", hook.name).dimmed(),
                status_str
            )?;
        }
        writeln!(out)?;

        for warning in &result.warnings {
            writeln!(out, "  {}", format!("⚠ {warning}").yellow())?;
        }
        for rec in &result.recommendations {
            writeln!(out, "  {}", format!("→ {rec}").dimmed())?;
        }
        if !result.warnings.is_empty() || !result.recommendations.is_empty() {
            writeln!(out)?;
        }
    }

    // Shell output
    if let Some(out) = out.for_shell() {
        writeln!(out, "hooks_path='{}'", result.hooks_path)?;
        writeln!(out, "custom_hooks_path={}", result.custom_hooks_path)?;
        writeln!(out, "config_enabled={}", result.config_enabled)?;
        writeln!(out, "mode='{}'", result.mode)?;
        if let Some(ref mgr) = result.external_manager {
            writeln!(out, "external_manager='{mgr}'")?;
        }
        for hook in &result.hooks {
            writeln!(out, "hook_{}='{}'", hook.name.replace('-', "_"), hook.owner)?;
        }
    }

    // JSON output
    if let Some(json_out) = out.for_json() {
        json_out.write_value(&result)?;
    }

    Ok(())
}
