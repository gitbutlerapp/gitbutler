//! Compatibility adapters for legacy GitButler metadata and snapshots.
#![forbid(unsafe_code)]

#[cfg(feature = "legacy")]
mod legacy;
#[cfg(feature = "legacy")]
pub use legacy::{garbage_collect, storage as legacy_storage};

#[cfg(feature = "legacy")]
pub mod virtual_branches_legacy_types;
