#![deny(missing_docs)]
//! Application update checking and management for GitButler.
//!
//! This crate provides functionality to check for new versions of GitButler applications
//! (CLI and GUI), cache update check results, and manage update notifications.
//!
//! # Core Functionality
//!
//! - **Update Checking**: Query the GitButler update server for available updates
//! - **Caching**: Persist update check results to avoid redundant network requests
//! - **Suppression**: Temporarily hide update notifications for a configurable duration
//! - **Locking**: Prevent concurrent update checks across multiple processes
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use but_update::{AppName, check_status, available_update};
//! # use but_settings::AppSettings;
//!
//! # fn example(app_settings: &AppSettings, cache: &mut but_db::AppCacheHandle) -> anyhow::Result<()> {
//! // Check for updates (results are automatically cached)
//! if let Some(status) = check_status(AppName::Cli, app_settings, cache)? {
//!     if !status.up_to_date {
//!         println!("Update available: {}", status.latest_version);
//!     }
//! } else {
//!     println!("Another process is checking for updates");
//! }
//!
//! // Later, retrieve cached update info without making a network request
//! if let Some(update) = available_update(cache)? {
//!     println!("Version {} is available", update.available_version);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Caching Behavior
//!
//! Update check results are automatically cached to the platform's cache directory.
//! The cache includes:
//! - Timestamp of the last check
//! - Update status (up-to-date or update available)
//! - Version information and release notes
//! - Suppression settings (if the user has snoozed the update)
//!
//! # Suppression
//!
//! Users can temporarily suppress update notifications for 1-720 hours (30 days):
//!
//! ```rust,no_run
//! use but_update::suppress_update;
//!
//! # fn example(cache: &mut but_db::AppCacheHandle) -> anyhow::Result<()> {
//! // Suppress update notifications for 24 hours
//! suppress_update(cache, 24)?;
//! # Ok(())
//! # }
//! ```

/// Module with functionality around checking the app updateability status.
mod check;

pub use check::{AppName, CheckUpdateStatus, check_status};

/// Module with functionality for caching update check results.
pub mod cache;
pub use cache::{AvailableUpdate, available_update, last_checked, suppress_update};

/// Return `true` if we are still holding a lock on our data as we are fetching it.
pub fn is_probably_still_running(cache: &mut but_db::AppCacheHandle) -> bool {
    cache
        .immediate_transaction_nonblocking()
        .ok()
        .flatten()
        .is_none()
}
