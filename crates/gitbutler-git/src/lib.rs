//! GitButler utility library for pushing/fetching Git repositories
//! using the Git CLI.
//!
//! **Important Note:** This is an interim library. Please do not rely on it;
//! it's only used as a temporary measure in the GitButler app until we implement
//! a longer-term solution for managing Git operations.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![cfg_attr(test, feature(async_closure))]
#![cfg_attr(windows, feature(windows_by_handle))]
#![feature(impl_trait_in_assoc_type)]

#[cfg(all(not(debug_assertions), feature = "test-askpass-path"))]
compile_error!("BUG: in production code this flag should not be set, nor do we run test with `cargo test --release`");

mod error;
pub(crate) mod executor;
mod refspec;
mod repository;

#[cfg(feature = "tokio")]
pub use self::executor::tokio;

pub use self::{
    error::Error,
    refspec::{Error as RefSpecError, RefSpec},
    repository::{fetch, push},
};
