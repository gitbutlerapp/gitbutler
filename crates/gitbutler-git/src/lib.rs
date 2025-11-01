//! GitButler utility library for pushing/fetching Git repositories
//! using the Git CLI.
//!
//! **Important Note:** This is an interim library. Please do not rely on it;
//! it's only used as a temporary measure in the GitButler app until we implement
//! a longer-term solution for managing Git operations.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![allow(unknown_lints)]

#[cfg(all(
    not(debug_assertions),
    not(feature = "benches"),
    feature = "test-askpass-path"
))]
compile_error!(
    "BUG: in production code this flag should not be set, nor do we run test with `cargo test --release`. Benches must use `--features benches`"
);

mod error;
/// utilities to execute a command
pub mod executor;
mod refspec;
mod repository;

#[cfg(feature = "tokio")]
pub use self::executor::tokio;
pub use self::{
    error::Error,
    refspec::{Error as RefSpecError, RefSpec},
    repository::{fetch, push, sign_commit},
};
