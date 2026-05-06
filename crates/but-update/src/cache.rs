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
///
/// This function reads from the cache and returns when the most recent
/// update check was performed. This can be used to:
/// - Determine if enough time has passed to trigger another check
/// - Display information to the user about update check freshness
///
/// # Returns
///
/// * `Ok(Some(timestamp))` - The UTC timestamp of the last update check
/// * `Ok(None)` - No cached update check exists or the cache is invalid
/// * `Err(_)` - Failed to access the cache
pub fn last_checked(cache: &but_db::AppCacheHandle) -> anyhow::Result<Option<DateTime<Utc>>> {
    Ok(cache.update_check().last_checked())
}

/// Information about an available application update.
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    /// The current version of the application.
    pub current_version: String,
    /// The latest available version.
    pub available_version: String,
    /// Markdown-formatted release notes for the new version.
    pub release_notes: Option<String>,
    /// Direct download URL for the update package.
    pub url: Option<String>,
}

impl AvailableUpdate {
    /// Checks if the available update is a no-op (i.e., the available version is the same as the current version).
    /// This can happen if the update check cache is stale.
    pub fn is_noop(&self) -> bool {
        self.available_version == self.current_version
    }
}

/// Returns information about an available application update, if one exists and is not suppressed.
///
/// This function checks the cache for a previously performed update check and returns
/// update information if:
/// - A cached update check exists
/// - The current build has a real compile-time `VERSION`
/// - The cached status indicates an update is available (`up_to_date == false`)
/// - The update is not currently suppressed
/// - The available version differs from the current version (not a no-op)
///
/// No-op updates (where the available version matches the current version) can occur when
/// the cache becomes stale. This function automatically filters them out.
///
/// # Returns
///
/// * `Ok(Some(AvailableUpdate))` - An update is available and not suppressed
/// * `Ok(None)` - No update is available, no cache exists, cache is invalid, update is suppressed, or update is a no-op
/// * `Err(_)` - Failed to access the cache
pub fn available_update(cache: &but_db::AppCacheHandle) -> anyhow::Result<Option<AvailableUpdate>> {
    available_update_for_version(cache, crate::current_version())
}

fn available_update_for_version(
    cache: &but_db::AppCacheHandle,
    current_version: Option<&str>,
) -> anyhow::Result<Option<AvailableUpdate>> {
    let Some(current_version) = current_version else {
        return Ok(None);
    };

    let cached = match cache.update_check().get() {
        Some(cached) => cached,
        None => return Ok(None),
    };

    // If already up to date, no update available
    if cached.status.up_to_date {
        return Ok(None);
    }

    // Check if suppression is active
    if let (Some(suppressed_at), Some(duration_hours)) =
        (cached.suppressed_at, cached.suppress_duration_hours)
    {
        let now = Utc::now();
        let suppress_until = suppressed_at + chrono::Duration::hours(duration_hours as i64);

        // If suppression is still active, return None
        if now <= suppress_until {
            return Ok(None);
        }
    }

    let update = AvailableUpdate {
        current_version: current_version.to_string(),
        available_version: cached.status.latest_version,
        release_notes: cached.status.release_notes,
        url: cached.status.url,
    };

    // Filter out no-op updates (stale cache entries where versions are identical)
    if update.is_noop() {
        return Ok(None);
    }

    Ok(Some(update))
}

/// Suppress an available update for a specified duration.
///
/// This function sets the suppression fields in the cache to temporarily hide an available update.
/// The suppression will automatically expire after the specified number of hours.
///
/// # Arguments
///
/// * `cache` - The application cache handle
/// * `hours` - The number of hours to suppress the update (must be between 1 and 720)
///
/// # Returns
///
/// * `Ok(())` - Suppression was successfully set
/// * `Err(_)` - An error occurred:
///   - Invalid hours value (must be 1-720)
///   - No cached update check exists
///   - The cached status shows the app is already up to date
///   - Failed to write the updated cache
///
/// # Errors
///
/// Returns an error if:
/// - The hours parameter is 0 or greater than 720 (30 days)
/// - No update check has been performed yet (no cache exists)
/// - The current version is already up to date (nothing to suppress)
/// - The cache cannot be written
pub fn suppress_update(cache: &mut but_db::AppCacheHandle, hours: u32) -> anyhow::Result<()> {
    // Validate input
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

#[cfg(test)]
mod tests {
    use but_db::{
        AppCacheHandle,
        cache::{CachedCheckResult, CheckUpdateStatus},
    };
    use chrono::DateTime;

    use super::available_update_for_version;

    #[test]
    fn returns_update_when_versions_differ() -> anyhow::Result<()> {
        let mut cache = in_memory_cache();
        cache.update_check_mut()?.save(&cached_update("1.2.3"))?;

        let update = available_update_for_version(&cache, Some("1.2.2"))?
            .expect("update should be available");

        assert_eq!(update.current_version, "1.2.2");
        assert_eq!(update.available_version, "1.2.3");
        assert_eq!(
            update.release_notes,
            Some("Bug fixes and improvements".to_string())
        );
        assert_eq!(update.url, Some("https://example.com/download".to_string()));
        Ok(())
    }

    #[test]
    fn ignores_no_op_update_when_versions_match() -> anyhow::Result<()> {
        let mut cache = in_memory_cache();
        cache.update_check_mut()?.save(&cached_update("1.2.3"))?;

        let update = available_update_for_version(&cache, Some("1.2.3"))?;

        assert!(update.is_none());
        Ok(())
    }

    fn cached_update(version: &str) -> CachedCheckResult {
        CachedCheckResult {
            checked_at: DateTime::from_timestamp(1000000, 0).unwrap(),
            status: CheckUpdateStatus {
                up_to_date: false,
                latest_version: version.to_string(),
                release_notes: Some("Bug fixes and improvements".to_string()),
                url: Some("https://example.com/download".to_string()),
                signature: Some("signature123".to_string()),
            },
            suppressed_at: None,
            suppress_duration_hours: None,
        }
    }

    fn in_memory_cache() -> AppCacheHandle {
        AppCacheHandle::new_at_path(":memory:")
    }
}
