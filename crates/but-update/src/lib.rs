#![deny(missing_docs)]
//! This crate provides functionality for updating the GitButler application itself.
//!
//! This includes mechanism to check for new versions, download updates, and apply them in a cross-platform manner.

/// Module with functionality around checking the app updateability status.
mod check;
pub use check::{AppName, CheckUpdateStatus, check_status};

/// Module with functionality for caching update check results.
pub mod cache;
pub use cache::{
    cached_app_update, last_checked, suppress_update_notification, try_update_check_lock,
    AppUpdateInfo,
};
