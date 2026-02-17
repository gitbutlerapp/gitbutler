use std::fmt::Write;

use but_core::sync::LockScope;
use but_ctx::Context;
use colored::Colorize;
use command_group::AsyncCommandGroup;

use crate::{args::Args, utils::OutputChannel};

#[derive(Default)]
pub(crate) enum BackgroundSync {
    Enabled,
    #[default]
    Disabled,
}

#[derive(Default)]
pub(crate) enum WorkspaceCheck {
    #[default]
    Enabled,
    Disabled,
}

/// Options for initializing the context via [`init_ctx`].
#[derive(Default)]
pub(crate) struct InitCtxOptions {
    /// Controls whether to perform automatic background synchronization.
    /// Defaults to `BackgroundSync::Disabled`.
    pub background_sync: BackgroundSync,
    /// Controls whether to check for workspace commit integrity.
    /// Defaults to `WorkspaceCheck::Enabled`.
    pub workspace_check: WorkspaceCheck,
}

/// Gets or initializes a non-bare repository context.
///
/// This function discovers the git repository from the current directory, finds or initializes
/// the GitButler project for that repository, and optionally triggers a background sync if
/// the configured interval has elapsed.
///
/// # Arguments
///
/// * `args` - Command-line arguments containing the current directory
/// * `options` - Configuration options for context initialization:
///   - `background_sync` - Controls automatic background synchronization:
///     - `BackgroundSync::Enabled` - Enable background sync (fetch, PR data, CI status)
///     - `BackgroundSync::Disabled` - Disable background sync completely
///   - `workspace_check` - Controls workspace commit integrity checking:
///     - `WorkspaceCheck::Enabled` - Check for non-workspace commits on gitbutler/workspace
///     - `WorkspaceCheck::Disabled` - Skip workspace commit checking
///
/// # Returns
///
/// Returns a `Context` containing the repository and project information.
///
/// # Errors
///
/// Returns an error if:
/// - The repository cannot be discovered
/// - The repository is bare (not supported)
/// - The project cannot be found or initialized
///
/// # Background sync behavior
///
/// Background sync is skipped entirely when the `NO_BG_TASKS` environment variable is set,
/// regardless of any other conditions.
///
/// When `background_sync` is `BackgroundSync::Enabled`, a background sync is initiated if:
/// - The fetch interval is positive (negative or zero disables background sync)
/// - The output format is for human consumption (not JSON or shell)
/// - Either no previous sync exists, or the elapsed time since the last sync
///   exceeds the configured interval
///
/// When a background sync is initiated, a human-readable message is written to the output
/// channel showing how long ago the last sync occurred (e.g., "Last fetch was 15m ago.
/// Initiated a background fetch..."). The time is formatted as seconds (s), minutes (m),
/// hours (h), or days (d) depending on the elapsed duration.
///
/// When `background_sync` is `BackgroundSync::Disabled`, no background sync is performed
/// regardless of the configured interval.
pub fn init_ctx(args: &Args, options: InitCtxOptions, out: &mut OutputChannel) -> anyhow::Result<Context> {
    // lets try to get the repo from the current directory
    let repo = match gix::discover(&args.current_dir) {
        Ok(repo) => repo,
        Err(_) => anyhow::bail!(
            "No git repository found at {}\nPlease run 'but setup' to initialize the project.",
            &args.current_dir.display()
        ),
    };

    // Check if we're on gitbutler/workspace with non-workspace commits on top
    // before creating the context
    if matches!(options.workspace_check, WorkspaceCheck::Enabled) {
        check_workspace_commits_before_init(&repo, out)?;
    }

    let (ctx, fetch_interval_minutes, last_fetch) = {
        let Some(workdir) = repo.workdir() else {
            anyhow::bail!("Bare repositories are not supported.");
        };
        #[cfg(feature = "legacy")]
        {
            use but_ctx::LegacyProject;

            use crate::command::legacy::setup::check_project_setup;

            // Try to find an existing project, or prompt for setup if not found
            let project = match LegacyProject::find_by_worktree_dir(workdir) {
                Ok(project) => project,
                Err(_) => {
                    let message = format!("No GitButler project found at {}", workdir.display());
                    match prompt_for_setup(out, &message) {
                        SetupPromptResult::RunSetup => {
                            // Run setup
                            let mut ctx = Context::from_repo(repo.clone())?;
                            let mut guard = ctx.exclusive_worktree_access();
                            crate::command::legacy::setup::repo(
                                &mut ctx,
                                &args.current_dir,
                                out,
                                guard.write_permission(),
                            )?;
                            // Retry finding the project after setup
                            LegacyProject::find_by_worktree_dir(workdir).map_err(|_| {
                                anyhow::anyhow!("Setup completed but project still not found at {}", workdir.display())
                            })?
                        }
                        SetupPromptResult::Declined => {
                            anyhow::bail!("Setup required: {message}");
                        }
                    }
                }
            };

            // Check project setup, prompt for setup if needed
            let mut ctx = Context::new_from_legacy_project(project)?;
            {
                let mut guard = ctx.exclusive_worktree_access();
                if let Err(e) = check_project_setup(&ctx, guard.read_permission()) {
                    let message = e.to_string();
                    match prompt_for_setup(out, &message) {
                        SetupPromptResult::RunSetup => {
                            // Run setup to fix the project configuration
                            crate::command::legacy::setup::repo(
                                &mut ctx,
                                &args.current_dir,
                                out,
                                guard.write_permission(),
                            )?;
                            // Re-find and re-check the project after setup
                            let _project = LegacyProject::find_by_worktree_dir(workdir).map_err(|_| {
                                anyhow::anyhow!("Setup completed but project still not found at {}", workdir.display())
                            })?;
                            check_project_setup(&ctx, guard.read_permission())?;
                        }
                        SetupPromptResult::Declined => {
                            anyhow::bail!("Setup required: {message}");
                        }
                    }
                }
            }

            let fetch_interval_minutes = ctx.settings.fetch.auto_fetch_interval_minutes;
            let last_fetch = ctx
                .legacy_project
                .project_data_last_fetch
                .as_ref()
                .map(|f| f.timestamp());
            (ctx, fetch_interval_minutes, last_fetch)
        }
        #[cfg(not(feature = "legacy"))]
        {
            let ctx = but_ctx::Context::from_repo(repo)?;
            // TODO: this can be implemented once project metadata is available from the project location itself,
            //       i.e. once it was migrated out of `projects.json` into `.git/gitbutler/…`
            let fetch_interval_disabled = 0;
            (ctx, fetch_interval_disabled, None::<std::time::SystemTime>)
        }
    };

    // If this is the first time running GitButler, show a metrics info message and update onboarding status
    if !ctx.settings.onboarding_complete && out.for_human().is_some() {
        crate::command::onboarding::handle(out)?;
    }

    match options.background_sync {
        BackgroundSync::Disabled => {
            return Ok(ctx);
        }
        BackgroundSync::Enabled => {
            // Check if background tasks are disabled via environment variable
            if std::env::var("NO_BG_TASKS").is_ok() {
                return Ok(ctx);
            }

            // Background sync only done for human output
            if !matches!(out.format(), crate::args::OutputFormat::Human) {
                return Ok(ctx);
            }

            // Determine what needs to be synced based on intervals and lock availability
            let sync_operations = determine_sync_operations(&ctx, fetch_interval_minutes, last_fetch);

            // Spawn background sync if there's anything to do
            if sync_operations.has_work() {
                spawn_background_sync(args, out, last_fetch, sync_operations);
            }
        }
    }

    Ok(ctx)
}

/// Tracks which background sync operations should be performed.
#[derive(Debug)]
struct SyncOperations {
    fetch: bool,
    pr: bool,
    ci: bool,
    updates: bool,
}

impl SyncOperations {
    fn has_work(&self) -> bool {
        self.fetch || self.pr || self.ci || self.updates
    }
}

/// Determines which sync operations should be performed based on intervals and lock availability.
fn determine_sync_operations(
    ctx: &Context,
    fetch_interval_minutes: isize,
    last_fetch: Option<std::time::SystemTime>,
) -> SyncOperations {
    // Check if fetch/pr/ci should run based on fetch interval
    let should_sync_remote_data = if fetch_interval_minutes > 0 {
        if let Some(last_fetch) = last_fetch {
            match std::time::SystemTime::now().duration_since(last_fetch) {
                Ok(elapsed) => elapsed.as_secs() / 60 >= fetch_interval_minutes as u64,
                Err(_) => true, // System time went backwards, force sync
            }
        } else {
            true // Never synced before, force sync
        }
    } else {
        false // Negative or zero interval disables sync
    };

    // Try to acquire lock for remote data operations (fetch/pr/ci)
    let remote_data_lock = if should_sync_remote_data {
        but_core::sync::try_exclusive_inter_process_access(&ctx.gitdir, LockScope::BackgroundRefreshOperations).ok()
    } else {
        None
    };

    // Determine if updates should be checked based on configured interval
    let update_check_interval_sec = ctx.settings.app_updates_check_interval_sec;
    let should_check_updates = if update_check_interval_sec == 0 {
        // Update checks disabled
        None
    } else {
        // Try to access cache to determine last check time
        let cache = but_db::AppCacheHandle::new_in_directory(but_path::app_cache_dir().ok());
        let should_check = but_update::last_checked(&cache)
            .ok()
            .flatten()
            .map(|last_check| {
                // Cache exists, check if interval has elapsed
                let now = chrono::Utc::now();
                let elapsed = now.signed_duration_since(last_check);
                elapsed.num_seconds() >= update_check_interval_sec as i64
            })
            .unwrap_or(true); // No cache exists - this is first check, should check
        should_check.then_some(cache)
    };

    // Try to acquire lock for update check by attempting to get a non-blocking transaction
    let can_check_updates = if let Some(mut cache) = should_check_updates {
        !but_update::is_probably_still_running(&mut cache)
    } else {
        false
    };

    // Check if we successfully acquired the locks
    let can_sync_remote_data = remote_data_lock.is_some();

    // Locks are dropped here, allowing the spawned child process to acquire them
    SyncOperations {
        fetch: can_sync_remote_data,
        pr: can_sync_remote_data,
        ci: can_sync_remote_data,
        updates: can_check_updates,
    }
}

/// Spawns a background process to perform the specified sync operations.
fn spawn_background_sync(
    args: &Args,
    out: &mut OutputChannel,
    last_fetch: Option<std::time::SystemTime>,
    operations: SyncOperations,
) {
    let binary_path = std::env::current_exe().unwrap_or_default();
    let mut cmd = tokio::process::Command::new(binary_path);
    cmd.arg("-C").arg(&args.current_dir).arg("refresh-remote-data");

    if operations.fetch {
        cmd.arg("--fetch");
    }
    if operations.pr {
        cmd.arg("--pr");
    }
    if operations.ci {
        cmd.arg("--ci");
    }
    if operations.updates {
        cmd.arg("--updates");
    }

    cmd.stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .group()
        .kill_on_drop(false);

    if cmd.spawn().is_ok() {
        // Show user feedback about what's happening
        // Only show "last fetch" message if we're actually fetching
        let msg = if operations.fetch {
            last_fetch
                .and_then(|t| {
                    std::time::SystemTime::now().duration_since(t).ok().map(|elapsed| {
                        let secs = elapsed.as_secs();
                        if secs < 60 {
                            format!("Last fetch was {secs}s ago. ")
                        } else if secs < 3600 {
                            format!("Last fetch was {}m ago. ", secs / 60)
                        } else if secs < 86400 {
                            format!("Last fetch was {}h ago. ", secs / 3600)
                        } else {
                            format!("Last fetch was {}d ago. ", secs / 86400)
                        }
                    })
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        writeln!(out, "{}", format!("{msg}Initiated a background sync...").dimmed()).ok();
    }
}

/// Represents the result of prompting the user for setup
enum SetupPromptResult {
    /// User agreed to run setup
    RunSetup,
    /// User declined or non-interactive terminal
    Declined,
}

fn prompt_for_setup(out: &mut OutputChannel, message: &str) -> SetupPromptResult {
    use std::fmt::Write;
    let mut progress = out.progress_channel();

    // Progress channel only writes when output is for humans, so we can write unconditionally
    _ = writeln!(
        progress,
        "The current project is not configured to be managed by GitButler.\n"
    );
    writeln!(progress, "{}\n", message.red()).ok();

    // Check if we have an interactive terminal and prompt the user
    let user_declined_in_interactive_mode = if let Some(mut inout) = out.prepare_for_terminal_input() {
        _ = writeln!(
            progress,
            "In order to manage projects with GitButler, we need to do some changes:\n\n - Switch you to a special `gitbutler/workspace` branch to enable parallel branches\n - Install Git hooks to help manage the tooling\n\nYou can go back to normal git workflows at any time by either:\n\n - Running `but teardown`\n - Manually checking out a normal branch with `git checkout <branch>`\n",
        );

        if let Ok(Some(response)) = inout.prompt("Would you like to run setup now? [Y/n]: ") {
            let response = response.trim().to_lowercase();
            if response.is_empty() || response == "y" || response == "yes" {
                return SetupPromptResult::RunSetup;
            }
        }
        // User declined
        true // indicates interactive terminal was available
    } else {
        false // non-interactive
    };

    // Now handle the declined/non-interactive cases with a fresh borrow
    if user_declined_in_interactive_mode {
        // User declined in interactive mode
        _ = writeln!(
            progress,
            "{}",
            "Please run `but setup` to switch to GitButler management.\n".yellow()
        );
    }

    if let Some(json_out) = out.for_json() {
        _ = json_out.write_value(serde_json::json!({
            "error": "setup_required",
            "message": message.to_string(),
            "hint": "run `but setup` to configure the project"
        }));
    } else if out.for_human().is_some() {
        // Non-interactive terminal, just show the hint
        _ = writeln!(
            progress,
            "{}",
            "Please run `but setup` to switch to GitButler management.\n".yellow()
        );
    };
    SetupPromptResult::Declined
}

/// Check if we're on gitbutler/workspace with non-workspace commits on top.
/// If so, inform the user and suggest running teardown to preserve their work.
fn check_workspace_commits_before_init(repo: &gix::Repository, out: &mut OutputChannel) -> anyhow::Result<()> {
    let head = repo.head()?;
    let head_name = head
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_default();

    // Only check if we're on gitbutler/workspace
    if head_name != "gitbutler/workspace" {
        return Ok(());
    }

    // Check the HEAD commit message
    let mut workspace_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
    let workspace_commit = workspace_ref.peel_to_commit()?;
    let message = workspace_commit.message_raw()?;
    let message_str = String::from_utf8_lossy(message);
    let first_line = message_str.lines().next().unwrap_or("");

    // If the first line is NOT a workspace commit, someone has committed on top
    if !first_line.starts_with("GitButler Workspace Commit") {
        if let Some(writer) = out.for_human() {
            writeln!(writer)?;
            writeln!(
                writer,
                "{}",
                "⚠ Detected commits on top of gitbutler/workspace".yellow().bold()
            )?;
            writeln!(writer)?;
            writeln!(
                writer,
                "{}",
                "GitButler detected that you have committed directly on the\ngitbutler/workspace branch. To preserve your work and\nfix up things, please run `but teardown`.".dimmed()
            )?;
            writeln!(writer)?;
        }

        // After teardown, we should not continue with the original command
        anyhow::bail!("GitButler mode exit required: please run `but teardown` to preserve your work.");
    }

    Ok(())
}
