//! GitButler core library for interacting with Git.
//!
//! This library houses a number of Git implementations,
//! over which we abstract a common interface and provide
//! higher-level operations that are implementation-agnostic.

#![cfg_attr(not(feature = "std"), no_std)] // must be first
#![feature(error_in_core)]
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
pub(crate) use integration_tests::*;

mod backend;
pub mod ops;
mod repository;

pub(crate) mod prelude;

#[cfg(feature = "cli")]
pub use backend::cli;
#[cfg(feature = "git2")]
pub use backend::git2;

pub use self::repository::{ConfigScope, Repository};
