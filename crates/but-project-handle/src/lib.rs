//! Temporary home of `ProjectHandle` and `ProjectHandleOrLegacyProjectId`.
//!
//! This crate exists only to break the dependency edge between `but-ctx` and
//! `gitbutler-project` while legacy project storage still exists.
//! Once `gitbutler-project` is removed, `ProjectHandle` will be merged back into
//! `but-ctx` and this crate will go away.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod project_handle;

/// A UUID based legacy project identifier carried for compatibility while project storage
/// still lives in `gitbutler-project`.
#[cfg(feature = "legacy")]
pub type LegacyProjectId = but_core::Id<'P'>;

pub use project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};
