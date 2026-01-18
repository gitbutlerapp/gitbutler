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
/// This function is `pub(crate)` for internal use but also exposed for testing purposes.
#[doc(hidden)]
pub fn save(status: &CheckUpdateStatus) {
    let result = || -> anyhow::Result<()> {
        let cache_path = cache_file_path()?;

        // Create cache directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Get current time once to ensure consistency across all uses
        let now = Utc::now();

        // Load existing cache to preserve suppression settings
        let existing_cache = load()?;

        // Determine if we should carry over suppression settings
        let (suppressed_at, suppress_duration_hours) = match existing_cache {
            Some(cached) => match (cached.suppressed_at, cached.suppress_duration_hours) {
                (Some(suppressed_at), Some(duration_hours)) => {
                    // Check if suppression period has expired
                    let suppress_until = suppressed_at + chrono::Duration::hours(duration_hours as i64);

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

        let json = serde_json::to_string_pretty(&cached)?;

        // Atomic write: write to temp file, then rename
        // This prevents corruption if the process crashes during write
        // Use process ID to avoid conflicts if multiple processes run concurrently
        let temp_path = cache_path.with_extension(format!("json.tmp.{}", std::process::id()));
        fs::write(&temp_path, json)?;

        // Rename is atomic - if this succeeds, the corrupted cache is replaced
        let rename_result = fs::rename(&temp_path, &cache_path);

        // Clean up temp file if rename failed
        if rename_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }

        rename_result?;

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
