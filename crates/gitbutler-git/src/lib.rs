//! GitButler utility library for pushing/fetching Git repositories
//! using the Git CLI.
//!
//! **Important Note:** This is an interim library. Please do not rely on it;
//! it's only used as a temporary measure in the GitButler app until we implement
//! a longer-term solution for managing Git operations.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![allow(unknown_lints)]

#[cfg(feature = "tokio")]
mod context;
mod error;
/// utilities to execute a command
pub mod executor;
mod refspec;
mod repository;

#[cfg(feature = "tokio")]
pub use self::context::{GitContextExt, PushResult, fetch_with_askpass, push_with_askpass};
#[cfg(feature = "tokio")]
pub use self::executor::tokio;
pub use self::{
    error::Error,
    refspec::{Error as RefSpecError, RefSpec},
    repository::{clone, fetch, push},
};
