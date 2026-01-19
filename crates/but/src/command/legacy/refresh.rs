use but_core::sync::LockScope;
use std::fmt::Write;

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
        let fetch_result = but_api::legacy::virtual_branches::fetch_from_remotes(
            ctx.legacy_project.id,
            Some("auto".to_string()),
        );
        if fetch_result.is_err() {
            out.write_str("Failed to fetch from the remote repository.")?;
        }
    }
    if prs {
        out.write_str("\nGetting PR data...")?;
        let pr_result = but_api::legacy::forge::list_reviews(
            ctx.legacy_project.id,
            Some(but_forge::CacheConfig::NoCache),
        );
        if pr_result.is_err() {
            out.write_str("Failed to refresh pull request data.")?;
        }
    }
    if ci {
        out.write_str("\nGetting CI checks...")?;
        let ci_result = but_api::legacy::forge::warm_ci_checks_cache(ctx.legacy_project.id);
        if ci_result.is_err() {
            out.write_str("Failed to refresh CI data.")?;
        }
    }
    if updates {
        out.write_str("\nChecking for updates...")?;

        // Try to acquire update check lock to prevent concurrent checks
        if let Ok(_update_lock) = but_update::try_update_check_lock() {
            // Lock is held for the duration of this block
            let update_result = but_update::check_status(but_update::AppName::Cli, app_settings);
            if update_result.is_err() {
                out.write_str("Failed to check for updates.")?;
            }
            // Lock is automatically released when _update_lock is dropped
        }
        // If lock acquisition fails, another process is already checking - skip silently
    }
    out.write_str("\nDone.")?;
    Ok(())
}
