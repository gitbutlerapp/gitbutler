//! Database reference metadata maintenance.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod garbage_collect;
pub use garbage_collect::garbage_collect;
