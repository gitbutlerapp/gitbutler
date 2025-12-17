//! Implementations for `but-core` metadata traits, associating data with Git entities.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "legacy")]
mod legacy;
#[cfg(feature = "legacy")]
pub use legacy::VirtualBranchesTomlMetadata;

#[cfg(feature = "legacy")]
pub mod virtual_branches_legacy_types;
