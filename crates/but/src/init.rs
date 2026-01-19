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
            // Negative or zero fetch interval disables background sync
            // Background sync done only for human output
            if fetch_interval_minutes <= 0
                || !matches!(out.format(), crate::args::OutputFormat::Human)
            {
                return Ok(ctx);
            }

            let should_fetch = if let Some(last_fetch) = last_fetch {
                match std::time::SystemTime::now().duration_since(last_fetch) {
                    Ok(elapsed) => elapsed.as_secs() / 60 >= fetch_interval_minutes as u64,
                    Err(_) => true, // System time went backwards, force fetch
                }
            } else {
                true // Never fetched before, force fetch
            };

            // Check if there is a process still doing background refreshes
            let exclusive_access = but_core::sync::try_exclusive_inter_process_access(
                &ctx.gitdir,
                LockScope::BackgroundRefreshOperations,
            )
            .ok();

            if should_fetch && exclusive_access.is_some() {
                drop(exclusive_access); // Release the lock immediately so that the new child process can acquire it
                let binary_path = std::env::current_exe().unwrap_or_default();
                let proc = tokio::process::Command::new(binary_path.clone())
                    .arg("-C")
                    .arg(&args.current_dir)
                    .arg("refresh-remote-data")
                    .arg("--fetch")
                    .arg("--pr")
                    .arg("--ci")
                    .stderr(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .group()
                    .kill_on_drop(false)
                    .spawn();
                if proc.is_ok() {
                    let msg = last_fetch
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
                        .unwrap_or_default();
                    writeln!(
                        out,
                        "{}",
                        format!("{}Initiated a background fetch...", msg).dimmed()
                    )
                    .ok();
                }
            }
        }
    }

    Ok(ctx)
}
