//! Database metadata maintenance and legacy oplog payload conversion.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "legacy")]
mod garbage_collect;
#[cfg(feature = "legacy")]
pub use garbage_collect::garbage_collect;

#[cfg(feature = "legacy")]
pub mod legacy_storage;

#[cfg(feature = "legacy")]
pub mod virtual_branches_legacy_types;
