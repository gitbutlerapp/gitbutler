use std::fmt::Write;

use but_core::sync::LockScope;

pub fn handle(
    ctx: &mut but_ctx::Context,
    out: &mut crate::utils::OutputChannel,
    fetch: bool,
    prs: bool,
    ci: bool,
    updates: bool,
    app_settings: &but_settings::AppSettings,
) -> anyhow::Result<()> {
    // Obtain a lock to prevent concurrent background refreshes of remote data
    // Only acquire this lock if we're actually performing remote data operations
    let _exclusive_access = if fetch || prs || ci {
        Some(but_core::sync::try_exclusive_inter_process_access(
            &ctx.gitdir,
            LockScope::BackgroundRefreshOperations,
        )?)
    } else {
        None
    };

    if fetch {
        out.write_str("\nFetching from remotes...")?;
        let fetch_result =
            but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("auto".to_string()));
        if fetch_result.is_err() {
            out.write_str("Failed to fetch from the remote repository.")?;
        }
    }
    if prs {
        out.write_str("\nGetting PR data...")?;
        let pr_result =
            but_api::legacy::forge::list_reviews(ctx, Some(but_forge::CacheConfig::NoCache));
        if pr_result.is_err() {
            out.write_str("Failed to refresh pull request data.")?;
        }
    }
    if ci {
        out.write_str("\nGetting CI checks...")?;
        let ci_result = but_api::legacy::forge::warm_ci_checks_cache(ctx);
        if ci_result.is_err() {
            out.write_str("Failed to refresh CI data.")?;
        }
    }
    if updates {
        out.write_str("\nChecking for updates...")?;

        let mut cache = ctx.app_cache.get_cache_mut()?;
        let update_result =
            but_update::check_status(but_update::AppName::Cli, app_settings, &mut cache);

        match update_result {
            Ok(None) => {
                out.write_str("Another process is checking for updates.")?;
            }
            Ok(Some(status)) => {
                out.write_str(&format!("Latest: {:?}", status.latest_version))?;
            }
            Err(e) => {
                out.write_str(&format!("Failed to check for updates: {e}"))?;
            }
        }
    }
    out.write_str("\nDone.")?;
    Ok(())
}
