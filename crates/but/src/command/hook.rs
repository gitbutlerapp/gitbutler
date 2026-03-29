//! Implementation of `but hook` subcommands.
//!
//! These commands provide GitButler's workspace guard and cleanup logic
//! as standalone CLI commands that any external hook manager can invoke.

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::utils::OutputChannel;

/// Open a git repository for hook subcommands.
///
/// Uses [`gix::discover_with_environment_overrides()`] so that the repository
/// is found via a directory walk when `GIT_DIR` is not set (e.g. direct
/// invocation by prek or in tests), and the git-provided `GIT_DIR` /
/// `GIT_WORK_TREE` environment variables are respected when git itself
/// invokes this command as a hook subprocess.
fn open_repo(current_dir: &std::path::Path) -> anyhow::Result<gix::Repository> {
    Ok(gix::discover_with_environment_overrides(current_dir)?)
}

/// Return the short name of the currently checked-out branch, or `None`
/// on detached HEAD / error.
fn current_branch_name(repo: &gix::Repository) -> Option<String> {
    repo.head()
        .ok()?
        .referent_name()
        .map(|n| n.shorten().to_string())
}

/// Block an action when on the `gitbutler/workspace` branch.
///
/// Opens the repository, resolves the current branch, and if it is
/// `gitbutler/workspace` prints a coloured error with the supplied
/// `headline` and `help_lines`, then bails with `bail_msg`.
/// Returns `Ok(())` on any other branch, detached HEAD, or if the
/// repository cannot be opened.
fn workspace_guard(
    out: &mut OutputChannel,
    current_dir: &std::path::Path,
    headline: &str,
    help_lines: &[&str],
    bail_msg: &str,
) -> Result<()> {
    let repo = match open_repo(current_dir) {
        Ok(repo) => repo,
        Err(_) => return Ok(()),
    };

    let Some(branch_name) = current_branch_name(&repo) else {
        return Ok(());
    };

    if branch_name == "gitbutler/workspace" {
        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(out, "{}", headline.red().bold())?;
            writeln!(out)?;
            for line in help_lines {
                writeln!(out, "  {}", line.dimmed())?;
            }
            writeln!(out)?;
        }
        anyhow::bail!("{bail_msg}");
    }

    Ok(())
}

/// Return the branch name from which the most recent checkout originated.
///
/// Walks the `HEAD` reflog in reverse (most-recent-first) looking for the
/// first `"checkout: moving from <src> to <dst>"` entry, and returns `<src>`.
///
/// # Why the reflog and not the hook arguments?
///
/// Git's `post-checkout` hook receives `$1` (previous HEAD commit SHA), `$2`
/// (new HEAD commit SHA), and `$3` (branch-checkout flag) — it does **not**
/// receive branch names. Recovering the previous *branch name* from the commit
/// SHA alone would require a reverse ref-lookup that is unreliable when the
/// SHA is reachable from multiple branches. The reflog entry written by Git at
/// checkout time is the authoritative, stable source for this information; its
/// `"checkout: moving from <src> to <dst>"` message format has been stable
/// since Git 1.8 and is relied upon by many tools.
///
/// When reflogs are disabled (`log.keepBackupRefs=false`, `--no-log`, or a
/// freshly-cloned repo before any checkout) `previous_checkout_branch_name`
/// returns `None` and the caller silently skips the workspace-guard check.
/// This is the safe default: better to miss a notification than to fire
/// incorrectly.
///
/// Returns `None` when:
/// - the reflog is absent or empty (e.g. freshly-cloned repo with reflogs disabled),
/// - no checkout entry exists yet, or
/// - the reflog entry cannot be parsed.
fn previous_checkout_branch_name(repo: &gix::Repository) -> Option<String> {
    use gix::bstr::ByteSlice;

    let head_ref = repo.find_reference("HEAD").ok()?;
    let mut log_iter = head_ref.log_iter();
    let mut reverse = log_iter.rev().ok()??;

    for entry in reverse.by_ref() {
        let Some(line) = entry.ok() else { continue };
        let Some(msg) = line.message.to_str().ok() else {
            continue;
        };
        if let Some(rest) = msg.strip_prefix("checkout: moving from ") {
            // Use rsplit_once so branch names containing " to " are parsed
            // correctly — the *last* " to " is always the separator.
            let (from_branch, _to) = rest.rsplit_once(" to ")?;
            return (!from_branch.is_empty()).then(|| from_branch.to_owned());
        }
    }
    None
}

/// Run the pre-commit workspace guard.
///
/// Blocks direct `git commit` on the `gitbutler/workspace` branch with a
/// helpful error message. Exits 0 (allow) on any other branch or if the
/// repository cannot be opened.
///
/// Respects git-provided environment variables (`GIT_DIR`, etc.) when
/// invoked as a hook subprocess.
pub fn pre_commit(out: &mut OutputChannel, current_dir: &std::path::Path) -> Result<()> {
    workspace_guard(
        out,
        current_dir,
        "GITBUTLER_ERROR: Cannot commit directly to gitbutler/workspace branch.",
        &[
            "GitButler manages commits on this branch. Please use GitButler to commit your changes:",
            "- Use the GitButler app to create commits",
            "- Or run 'but commit' from the command line",
            "",
            "If you want to exit GitButler mode and use normal git:",
            "- Run 'but teardown' to switch to a regular branch",
            "- Or directly checkout another branch: git checkout <branch>",
        ],
        "Cannot commit directly to gitbutler/workspace branch",
    )
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
/// Respects git-provided environment variables (`GIT_DIR`, etc.) when
/// invoked as a hook subprocess.
pub fn post_checkout(
    out: &mut OutputChannel,
    current_dir: &std::path::Path,
    _prev_head: &str,
    _new_head: &str,
    is_branch_checkout: &str,
) -> Result<()> {
    // Only act on branch checkouts (not file checkouts)
    if is_branch_checkout != "1" {
        return Ok(());
    }

    let repo = match open_repo(current_dir) {
        Ok(repo) => repo,
        Err(_) => return Ok(()),
    };

    let prev_branch = previous_checkout_branch_name(&repo);
    if prev_branch.as_deref() != Some("gitbutler/workspace") {
        return Ok(());
    }

    let new_branch = current_branch_name(&repo).unwrap_or_default();

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
/// Respects git-provided environment variables (`GIT_DIR`, etc.) when
/// invoked as a hook subprocess.
pub fn pre_push(
    out: &mut OutputChannel,
    current_dir: &std::path::Path,
    _remote_name: &str,
    _remote_url: &str,
) -> Result<()> {
    workspace_guard(
        out,
        current_dir,
        "GITBUTLER_ERROR: Cannot push the gitbutler/workspace branch.",
        &[
            "The workspace branch is a synthetic branch managed by GitButler.",
            "Pushing it to a remote would publish GitButler's internal state.",
            "",
            "To push your branches, use:",
            "- The GitButler app to push branches",
            "- Or run 'but push' from the command line",
            "",
            "If you want to exit GitButler mode and push normally:",
            "- Run 'but teardown' to switch to a regular branch",
        ],
        "Cannot push the gitbutler/workspace branch",
    )
}

/// The integration mode for GitButler hooks in this repository.
#[derive(Debug, PartialEq, Serialize)]
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
    External,
    /// The hook file exists but is not recognized as GitButler or a known manager.
    User,
    /// No hook file exists at this path.
    Missing,
}

impl HookOwner {
    /// Human-readable label, optionally annotated with the manager name.
    fn display(&self, manager: Option<&str>) -> String {
        match self {
            Self::Gitbutler => "GitButler-managed".to_owned(),
            Self::External => format!("external ({})", manager.unwrap_or("unknown")),
            Self::User => "user hook".to_owned(),
            Self::Missing => "not installed".to_owned(),
        }
    }
}

impl std::fmt::Display for HookOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display(None))
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
    /// Name of the external hook manager, when `owner` is `external`.
    #[serde(skip_serializing_if = "Option::is_none")]
    manager: Option<String>,
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

/// Human-readable description of the consequence when a GitButler hook is missing or displaced.
fn hook_missing_description(hook_name: &str) -> &'static str {
    match hook_name {
        "pre-commit" => {
            "workspace guard won't run — commits to the workspace branch won't be blocked"
        }
        "post-checkout" => {
            "workspace cleanup won't run — stale state may persist when switching branches"
        }
        "pre-push" => "push guard won't run — pushes from the workspace branch won't be blocked",
        _ => "GitButler hook won't run",
    }
}

/// Show hook ownership and integration state for the current repository.
///
/// Inspects the hooks directory, `gitbutler.installHooks` config, and
/// hook file contents to report a diagnostic summary. Produces human,
/// JSON, and shell output.
///
/// Respects git-provided environment variables (`GIT_DIR`, etc.) when
/// invoked as a hook subprocess.
pub fn status(out: &mut OutputChannel, current_dir: &std::path::Path) -> Result<()> {
    let repo = open_repo(current_dir)?;

    let hooks_dir = but_hooks::managed_hooks::get_hooks_dir_gix(&repo);
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

    // Detect config-only manager once (no hook files present yet, e.g. prek.toml exists
    // and binary is on PATH, but `prek install` hasn't run). Hoisted outside the loop to
    // avoid redundant disk I/O (config stat + which call) for each of the three hooks.
    let config_only_manager = but_hooks::hook_manager::detect_hook_manager("", &workdir);

    for hook_name in but_hooks::hook_manager::MANAGED_HOOK_NAMES {
        let hook_path = hooks_dir.join(hook_name);
        let (exists, owner, manager) = if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
            if but_hooks::managed_hooks::is_gitbutler_managed_hook(&hook_path) {
                has_gitbutler_hook = true;
                (true, HookOwner::Gitbutler, None)
            } else if let Some(manager) =
                but_hooks::hook_manager::detect_hook_manager(&content, &workdir)
            {
                let name = manager.name().to_owned();
                external_manager_name = Some(name.clone());
                (true, HookOwner::External, Some(name))
            } else {
                (true, HookOwner::User, None)
            }
        } else if let Some(manager) = config_only_manager {
            // Hook file doesn't exist, but a manager is configured + available
            // (e.g. prek.toml present, binary on PATH, hooks not yet installed).
            let name = manager.name().to_owned();
            external_manager_name = Some(name.clone());
            (false, HookOwner::External, Some(name))
        } else {
            (false, HookOwner::Missing, None)
        };

        hooks.push(SingleHookStatus {
            name: (*hook_name).to_owned(),
            exists,
            owner,
            manager,
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
    // Warn about each GitButler-required hook that the external manager hasn't installed.
    // A missing hook means the corresponding GitButler guard won't fire, which can lead
    // to silent data-integrity issues (e.g. committing to the workspace branch unguarded).
    if mode == HookMode::External {
        for hook in &hooks {
            if !hook.exists {
                let description = hook_missing_description(&hook.name);
                let mgr = hook.manager.as_deref().unwrap_or("external manager");
                warnings.push(format!(
                    "{} is not installed by {mgr} — {description}",
                    hook.name
                ));
            }
        }
    }
    // In Managed mode, warn about any slot occupied by a user hook instead of a GB hook.
    // The mode label says "GitButler-managed" but the guard for that slot won't fire,
    // which can silently break workspace protection (e.g. the pre-commit workspace guard).
    if mode == HookMode::Managed {
        for hook in &hooks {
            if matches!(hook.owner, HookOwner::User) {
                let description = hook_missing_description(&hook.name);
                warnings.push(format!("{}: user hook present — {description}", hook.name));
            }
        }
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
    // Add a single setup hint if any external-manager hooks are missing.
    let has_missing_external_hooks = mode == HookMode::External && hooks.iter().any(|h| !h.exists);
    if has_missing_external_hooks {
        recommendations.push("Run `but setup` to see integration instructions.".into());
    }
    if custom_hooks_path && but_hooks::managed_hooks::has_managed_hooks_in(&default_hooks_dir) {
        recommendations.push(format!(
            "Remove orphaned hooks: {}",
            but_hooks::hook_manager::orphaned_hooks_remove_command()
        ));
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
                    HookOwner::External => {
                        format!("● {}", hook.owner.display(hook.manager.as_deref()))
                            .cyan()
                            .to_string()
                    }
                    HookOwner::User => format!("○ {}", hook.owner).yellow().to_string(),
                    HookOwner::Missing => unreachable!(),
                }
            } else {
                let label = match &hook.owner {
                    HookOwner::External => format!(
                        "not configured ({})",
                        hook.manager.as_deref().unwrap_or("unknown")
                    ),
                    other => other.display(hook.manager.as_deref()),
                };
                format!("✗ {label}").dimmed().to_string()
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
            writeln!(
                out,
                "hook_{}='{}'",
                hook.name.replace('-', "_"),
                hook.owner.display(hook.manager.as_deref())
            )?;
        }
    }

    // JSON output
    if let Some(json_out) = out.for_json() {
        json_out.write_value(&result)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_missing_description_covers_all_managed_hooks() {
        assert!(hook_missing_description("pre-commit").contains("workspace guard"));
        assert!(hook_missing_description("post-checkout").contains("workspace cleanup"));
        assert!(hook_missing_description("pre-push").contains("push guard"));
        assert_eq!(
            hook_missing_description("unknown-hook"),
            "GitButler hook won't run"
        );
    }
}
