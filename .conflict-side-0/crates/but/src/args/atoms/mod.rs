//! This module contains "CLI argument atoms".
//!
//! Instead of using `String` for CLI argument types we should use more specific types from this
//! modules. The types provides ways to be "resolved" that ensures consistent behavior, especially
//! around error messages.

#![warn(missing_docs)]

mod branch_arg;
pub use branch_arg::*;

mod commit_arg;
pub use commit_arg::*;

mod cli_id;
pub use cli_id::*;
