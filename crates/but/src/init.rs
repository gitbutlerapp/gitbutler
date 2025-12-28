use crate::{args::Args, utils::OutputChannel};
use but_ctx::Context;
use command_group::AsyncCommandGroup;
use std::fmt::Write;

pub(crate) enum Fetch {
    Auto,
    None,
}

/// Gets or initializes a non-bare repository context.
///
/// This function discovers the git repository from the current directory, finds or initializes
/// the GitButler project for that repository, and optionally triggers a background fetch if
/// the configured fetch interval has elapsed.
///
/// # Arguments
///
/// * `args` - Command-line arguments containing the current directory
/// * `fetch_mode` - Controls whether and how to perform automatic fetching:
///   - `Fetch::Auto(out)` - Enable auto-fetch with output to the provided channel
///   - `Fetch::None` - Disable auto-fetch completely
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
/// # Auto-fetch behavior
///
/// When `fetch_mode` is `Fetch::Auto(out)`, a background fetch is initiated if:
/// - The fetch interval is positive (negative or zero disables auto-fetch)
/// - The output format is for human consumption (not JSON or shell)
/// - Either no previous fetch exists, or the elapsed time since the last fetch
///   exceeds the configured interval
///
/// When a background fetch is initiated, a human-readable message is written to the output
/// channel showing how long ago the last fetch occurred (e.g., "Last fetch was 15m ago.
/// Initiated a background fetch..."). The time is formatted as seconds (s), minutes (m),
/// hours (h), or days (d) depending on the elapsed duration.
///
/// When `fetch_mode` is `Fetch::None`, no auto-fetch is performed regardless of the
/// configured interval.
pub fn init_ctx(
    args: &Args,
    fetch_mode: Fetch,
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

    match fetch_mode {
        Fetch::None => {
            return Ok(ctx);
        }
        Fetch::Auto => {
            // Negative or zero fetch interval disables auto-fetch
            // Auto fetch done only for human output
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

            if should_fetch {
                let binary_path = std::env::current_exe().unwrap_or_default();
                let proc = tokio::process::Command::new(binary_path)
                    .arg("-C")
                    .arg(&args.current_dir)
                    .arg("base")
                    .arg("fetch")
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
                    writeln!(out, "{}Initiated a background fetch...", msg).ok();
                }
            }
        }
    }

    Ok(ctx)
}
