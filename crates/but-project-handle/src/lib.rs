//! Temporary home of `ProjectHandle` and `ProjectHandleOrLegacyProjectId`.
//!
//! This crate exists only to break the dependency edge between `but-ctx` and
//! `gitbutler-project` while legacy project storage still exists.
//! Once `gitbutler-project` is removed, `ProjectHandle` will be merged back into
//! `but-ctx` and this crate will go away.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "legacy")]
mod legacy_project_id;
mod project_handle;
mod storage_path;

/// A UUID based legacy project identifier carried for compatibility while project storage
/// still lives in `gitbutler-project`.
#[cfg(feature = "legacy")]
pub use legacy_project_id::LegacyProjectId;

pub use project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};
pub use storage_path::{
    DEFAULT_STORAGE_DIR_NAME, REFRESH_SENTINEL_PATH, gitbutler_storage_path,
    gitbutler_storage_path_for_channel, process_sentinel_token, storage_path_config_key,
    storage_path_config_key_for_app_channel, write_refresh_sentinel,
};
