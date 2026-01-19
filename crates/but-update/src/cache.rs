//! Caching layer for update check results to avoid redundant network requests.
//!
//! This module provides functionality to persist update check results to disk,
//! allowing the CLI to:
//! - Avoid checking for updates too frequently
//! - Display cached update information without making network requests
//! - Track when the last successful update check occurred
//! - Temporarily suppress update notifications for a configurable duration

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::CheckUpdateStatus;

/// The name of the cache file stored in the cache directory.
const CACHE_FILE_NAME: &str = "update-check.json";

/// A cached update check result with timestamp.
///
/// This struct wraps the update check status with metadata about when the check occurred.
/// It's persisted to disk in JSON format and can be loaded to avoid redundant network requests.
#[derive(Debug, Serialize, Deserialize)]
struct CachedCheckResult {
    /// When this update check was performed (UTC timestamp).
    checked_at: DateTime<Utc>,
    /// The update status information from the server.
    status: CheckUpdateStatus,
    /// When the update notification was suppressed (UTC timestamp).
    /// This allows users to temporarily hide update notifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suppressed_at: Option<DateTime<Utc>>,
    /// The number of hours to suppress update notifications.
    /// After this duration expires from `suppressed_at`, notifications will resume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suppress_duration_hours: Option<u32>,
}

/// Returns the path to the update check cache file.
///
/// # Errors
///
/// Returns an error if the cache directory cannot be determined.
fn cache_file_path() -> anyhow::Result<PathBuf> {
    Ok(but_path::app_cache_dir()?.join(CACHE_FILE_NAME))
}

/// Writes a cached result to disk atomically.
///
/// This function handles:
/// - Creating the cache directory if it doesn't exist
/// - Atomic write-and-rename to prevent corruption
/// - Cleanup of temporary files on failure
///
/// # Errors
///
/// Returns an error if the file cannot be written or renamed.
fn write_cache_atomically(cached: &CachedCheckResult) -> anyhow::Result<()> {
    let cache_path = cache_file_path()?;

    // Create cache directory if it doesn't exist
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&cached)?;

    // Atomic write: write to temp file, then rename
    // Use process ID to avoid conflicts if multiple processes run concurrently
    let temp_path = cache_path.with_extension(format!("json.tmp.{}", std::process::id()));
    fs::write(&temp_path, json)?;

    // Rename is atomic
    let rename_result = fs::rename(&temp_path, &cache_path);

    // Clean up temp file if rename failed
    if rename_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    rename_result?;

    Ok(())
}

/// Saves an update check result to the cache.
///
/// This function persists the update status along with the current timestamp
/// to disk. If the cache directory doesn't exist, it will be created.
///
/// Uses atomic write-and-rename to prevent corruption if the process crashes
/// during the write operation. Cache failures are silently ignored, as caching
/// is optional and best-effort.
///
/// If a previous cache exists with suppression settings (`suppressed_at` and `suppress_duration_hours`),
/// these settings are preserved in the new cache. However, if the suppression period has expired
/// (current time is past `suppressed_at` + `suppress_duration_hours`), the suppression fields are cleared.
///
/// # Arguments
///
/// * `status` - The update check result to cache
///
/// # Note
///
/// This function is hidden from documentation but exposed publicly for internal use and testing.
#[doc(hidden)]
pub fn save(status: &CheckUpdateStatus) {
    let result = || -> anyhow::Result<()> {
        // Get current time once to ensure consistency across all uses
        let now = Utc::now();

        // Load existing cache to preserve suppression settings
        let existing_cache = load()?;

        // Determine if we should carry over suppression settings
        let (suppressed_at, suppress_duration_hours) = match existing_cache {
            Some(cached) => match (cached.suppressed_at, cached.suppress_duration_hours) {
                (Some(suppressed_at), Some(duration_hours)) => {
                    // Check if suppression period has expired
                    let suppress_until =
                        suppressed_at + chrono::Duration::hours(duration_hours as i64);

                    if now > suppress_until {
                        // Suppression period has expired, clear the fields
                        (None, None)
                    } else {
                        // Suppression still active, preserve the fields
                        (Some(suppressed_at), Some(duration_hours))
                    }
                }
                _ => {
                    // Incomplete suppression data (missing one of the fields)
                    (None, None)
                }
            },
            None => {
                // No existing cache
                (None, None)
            }
        };

        let cached = CachedCheckResult {
            checked_at: now,
            status: status.clone(),
            suppressed_at,
            suppress_duration_hours,
        };

        write_cache_atomically(&cached)?;

        Ok(())
    };

    // Silently ignore errors - caching is best-effort
    let _ = result();
}

/// Loads the cached update check result if available.
///
/// Returns `None` if the cache file doesn't exist or cannot be parsed.
/// Corrupted cache files are silently ignored and treated as if they don't exist.
///
/// # Errors
///
/// Returns an error only if the cache directory path cannot be determined.
fn load() -> anyhow::Result<Option<CachedCheckResult>> {
    let cache_path = cache_file_path()?;

    if !cache_path.exists() {
        return Ok(None);
    }

    match fs::read_to_string(&cache_path) {
        Ok(contents) => match serde_json::from_str::<CachedCheckResult>(&contents) {
            Ok(cached) => Ok(Some(cached)),
            Err(_) => {
                // Cache is corrupted, treat as if it doesn't exist
                Ok(None)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File was deleted between exists() check and read
            Ok(None)
        }
        Err(_) => {
            // Other IO errors (permissions, etc.) - treat as missing cache
            Ok(None)
        }
    }
}

/// Returns the timestamp of the last successful update check, if available.
///
/// This function reads from the cache file and returns when the most recent
/// update check was performed. This can be used to:
/// - Determine if enough time has passed to trigger another check
/// - Display information to the user about update check freshness
///
/// # Returns
///
/// * `Ok(Some(timestamp))` - The UTC timestamp of the last update check
/// * `Ok(None)` - No cached update check exists or the cache is invalid
/// * `Err(_)` - Failed to determine cache directory path
pub fn last_checked() -> anyhow::Result<Option<DateTime<Utc>>> {
    Ok(load()?.map(|cached| cached.checked_at))
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

/// Returns information about an available application update, if one exists and is not suppressed.
///
/// This function checks the cache for a previously performed update check and returns
/// update information if:
/// - A cached update check exists
/// - The cached status indicates an update is available (`up_to_date == false`)
/// - The update is not currently suppressed
///
/// # Returns
///
/// * `Ok(Some(AvailableUpdate))` - An update is available and not suppressed
/// * `Ok(None)` - No update is available, no cache exists, cache is invalid, or update is suppressed
/// * `Err(_)` - Failed to determine cache directory path
pub fn available_update() -> anyhow::Result<Option<AvailableUpdate>> {
    let cached = match load()? {
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

    // Update is available and not suppressed
    let current_version = option_env!("VERSION").unwrap_or("0.0.0").to_string();

    Ok(Some(AvailableUpdate {
        current_version,
        available_version: cached.status.latest_version,
        release_notes: cached.status.release_notes,
        url: cached.status.url,
    }))
}

/// Suppress an available update for a specified duration.
///
/// This function sets the suppression fields in the cache to temporarily hide an available update.
/// The suppression will automatically expire after the specified number of hours.
///
/// # Arguments
///
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
/// - The cache file cannot be written
pub fn suppress_update(hours: u32) -> anyhow::Result<()> {
    // Validate input: must be between 1 and 720 hours (30 days)
    const MAX_SUPPRESSION_HOURS: u32 = 720; // 30 days

    if hours == 0 {
        anyhow::bail!("Suppression duration must be at least 1 hour");
    }

    if hours > MAX_SUPPRESSION_HOURS {
        anyhow::bail!(
            "Suppression duration cannot exceed {} hours (30 days)",
            MAX_SUPPRESSION_HOURS
        );
    }

    // Load existing cache
    let existing_cache = load()?;

    let mut cached = match existing_cache {
        Some(cached) => cached,
        None => {
            anyhow::bail!(
                "No update check has been performed yet. Run an update check first before suppressing notifications."
            );
        }
    };

    // Check if already up to date
    if cached.status.up_to_date {
        anyhow::bail!(
            "The application is already up to date. There are no update notifications to suppress."
        );
    }

    // Set suppression fields
    cached.suppressed_at = Some(Utc::now());
    cached.suppress_duration_hours = Some(hours);

    // Write back to cache atomically
    write_cache_atomically(&cached)?;

    Ok(())
}

/// Try to obtain an exclusive inter-process lock for update checking.
///
/// This prevents multiple CLI processes from checking for updates simultaneously.
/// The lock is held for the lifetime of the returned [`but_core::sync::LockFile`] and is
/// automatically released when dropped or when the process exits.
///
/// # Returns
///
/// * `Ok(LockFile)` - Successfully acquired the lock
/// * `Err(_)` - Another process is already checking for updates, or the lock file
///   cannot be accessed
pub fn try_update_check_lock() -> anyhow::Result<but_core::sync::LockFile> {
    let lock_path = but_path::app_cache_dir()?.join("update-check.lock");

    // Create cache directory if it doesn't exist
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lock_file = but_core::sync::LockFile::open(&lock_path)?;
    let got_lock = lock_file.try_lock()?;
    if !got_lock {
        anyhow::bail!("Another process is already checking for updates");
    }

    Ok(lock_file)
}
