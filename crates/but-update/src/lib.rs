#![deny(missing_docs)]
//! This crate provides functionality for updating the GitButler application itself.
//!
//! This includes mechanism to check for new versions, download updates, and apply them in a cross-platform manner.

/// Module with functionality around checking the app updateability status.
mod check;
pub use check::{AppName, CheckUpdateStatus, check_status};
