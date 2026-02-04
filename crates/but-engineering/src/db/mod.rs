//! Database module for but-engineering.
//!
//! Provides connection handling and table operations for agents and messages.

mod handle;
mod migration;
pub mod table;

pub use handle::DbHandle;
pub use migration::{M, is_transient_error};
