use std::fmt::Write;

use but_core::sync::LockScope;
use but_ctx::Context;
use colored::Colorize;
use command_group::AsyncCommandGroup;

use crate::{args::Args, utils::OutputChannel};

pub(crate) enum BackgroundSync {
    Enabled,
    Disabled,
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
/// * `background_sync` - Controls whether to perform automatic background synchronization:
///   - `BackgroundSync::Enabled` - Enable background sync (fetch, PR data, CI status)
///   - `BackgroundSync::Disabled` - Disable background sync completely
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
pub fn init_ctx(
    args: &Args,
    background_sync: BackgroundSync,
    out: &mut OutputChannel,
) -> anyhow::Result<Context> {
    let repo = gix::discover(&args.current_dir)?;
    let (ctx, fetch_interval_minutes, last_fetch) = {
        let Some(workdir) = repo.workdir() else {
            anyhow::bail!("Bare repositories are not supported.");
        };
        #[cfg(feature = "legacy")]
        {
            use but_ctx::LegacyProject;
            let project = match LegacyProject::find_by_worktree_dir(workdir) {
                Ok(p) => Ok(p),
                Err(_e) => {
                    crate::command::legacy::init::repo(
                        workdir,
                        &mut OutputChannel::new_without_pager_non_json(args.format),
                        false,
                    )?;
                    LegacyProject::find_by_worktree_dir(workdir)
                }
            }?;
            let ctx = Context::new_from_legacy_project(project)?;
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

    match background_sync {
        BackgroundSync::Disabled => {
            return Ok(ctx);
        }
        BackgroundSync::Enabled => {
            // Background sync only done for human output
            if !matches!(out.format(), crate::args::OutputFormat::Human) {
                return Ok(ctx);
            }

            // Determine what needs to be synced based on intervals and lock availability
            let sync_operations =
                determine_sync_operations(&ctx, fetch_interval_minutes, last_fetch);

            // Spawn background sync if there's anything to do
            if sync_operations.has_work() {
                spawn_background_sync(args, out, last_fetch, sync_operations);
            }
        }
    }

    Ok(ctx)
}

/// Tracks which background sync operations should be performed.
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
        but_core::sync::try_exclusive_inter_process_access(
            &ctx.gitdir,
            LockScope::BackgroundRefreshOperations,
        )
        .ok()
    } else {
        None
    };

    // Determine if updates should be checked based on configured interval
    let update_check_interval_sec = ctx.settings.app_updates_check_interval_sec;
    let should_check_updates = if update_check_interval_sec == 0 {
        // Update checks disabled
        false
    } else {
        match but_update::last_checked() {
            Ok(Some(last_check)) => {
                // Cache exists, check if interval has elapsed
                let now = chrono::Utc::now();
                let elapsed = now.signed_duration_since(last_check);
                elapsed.num_seconds() >= update_check_interval_sec as i64
            }
            Ok(None) => {
                // No cache exists - this is first check or cache was deleted, should check
                true
            }
            Err(_) => {
                // Error determining cache state - fail-safe for tests and error cases
                false
            }
        }
    };

    // Try to acquire lock for update check
    let update_lock = if should_check_updates {
        but_update::try_update_check_lock().ok()
    } else {
        None
    };

    // Check if we successfully acquired the locks
    let can_sync_remote_data = remote_data_lock.is_some();
    let can_check_updates = update_lock.is_some();

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
    cmd.arg("-C")
        .arg(&args.current_dir)
        .arg("refresh-remote-data");

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
                    std::time::SystemTime::now()
                        .duration_since(t)
                        .ok()
                        .map(|elapsed| {
                            let secs = elapsed.as_secs();
                            if secs < 60 {
                                format!("Last fetch was {}s ago. ", secs)
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

        writeln!(
            out,
            "{}",
            format!("{}Initiated a background sync...", msg).dimmed()
        )
        .ok();
    }
}
