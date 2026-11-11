//! Caching layer for update check results to avoid redundant network requests.
//!
//! This module provides functionality to persist update check results to the database,
//! allowing the CLI to:
//! - Avoid checking for updates too frequently
//! - Display cached update information without making network requests
//! - Track when the last successful update check occurred
//! - Temporarily suppress update notifications for a configurable duration

use chrono::{DateTime, Utc};

/// Returns the timestamp of the last successful update check, if available.
pub fn last_checked(cache: &but_db::AppCacheHandle) -> anyhow::Result<Option<DateTime<Utc>>> {
    Ok(cache.update_check().last_checked())
}

#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    /// The current version of the application.
    pub current_version: String,
    pub available_version: String,
    /// Markdown-formatted release notes for the new version.
    pub release_notes: Option<String>,
    /// Direct download URL for the update package.
    pub url: Option<String>,
}

/// Returns information about an available application update, if one exists and is not suppressed.
///
/// This function checks the cache for a previously performed update check and returns
/// update information if:
/// - A cached update check exists
/// - The cached status indicates an update is available (`up_to_date == false`)
/// - The cached check is valid for the current version of the app
/// - The update is not currently suppressed
pub fn available_update(cache: &but_db::AppCacheHandle) -> anyhow::Result<Option<AvailableUpdate>> {
    let cached = match cache.update_check().get() {
        Some(cached) => cached,
        None => return Ok(None),
    };

    let current_version = option_env!("VERSION").unwrap_or("0.0.0").to_string();
    let valid_for_version = cached
        .status
        .valid_for_version
        .unwrap_or_else(|| current_version.clone());

    if cached.status.up_to_date || current_version != valid_for_version {
        return Ok(None);
    }

    if let (Some(suppressed_at), Some(duration_hours)) =
        (cached.suppressed_at, cached.suppress_duration_hours)
    {
        let now = Utc::now();
        let suppress_until = suppressed_at + chrono::Duration::hours(duration_hours as i64);

        if now <= suppress_until {
            return Ok(None);
        }
    }

    let update = AvailableUpdate {
        current_version,
        available_version: cached.status.latest_version,
        release_notes: cached.status.release_notes,
        url: cached.status.url,
    };

    Ok(Some(update))
}

/// This function sets the suppression fields in the cache to temporarily hide an available update.
/// The suppression will automatically expire after the specified number of hours.
///
/// # Errors
///
/// Returns an error if:
/// - The hours parameter is 0 or greater than 720 (30 days)
/// - No update check has been performed yet (no cache exists)
/// - The current version is already up to date (nothing to suppress)
/// - The cache cannot be written
pub fn suppress_update(cache: &mut but_db::AppCacheHandle, hours: u32) -> anyhow::Result<()> {
    const MAX_SUPPRESSION_HOURS: u32 = 720; // 30 days

    if hours == 0 {
        anyhow::bail!("Suppression duration must be at least 1 hour");
    }

    if hours > MAX_SUPPRESSION_HOURS {
        anyhow::bail!("Suppression duration cannot exceed {MAX_SUPPRESSION_HOURS} hours (30 days)");
    }

    cache
        .update_check_mut()?
        .suppress(hours)
        .map_err(Into::into)
}
