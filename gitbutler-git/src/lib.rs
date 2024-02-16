//! GitButler core library for interacting with Git.
//!
//! This library houses a number of Git implementations,
//! over which we abstract a common interface and provide
//! higher-level operations that are implementation-agnostic.
//!
//! # Libgit2 Support
//! This library supports libgit2 via the `git2` feature.
//! Not much in the way of assumptions are made about the environment;
//! it's a fairly clean and safe Git backend.
//!
//! # Fork/Exec (CLI) Support
//! This library supports the Git CLI via the `cli` feature.
//! Note that this is a fairly experimental implementation that
//! uses some (ideally portable) hacks for authentication,
//! including a custom executable (or two, in the case of
//! *nix systems) for handling automatic authentication
//! via the API.
//!
//! This means those executables must be situated next to
//! the executable that is running them (as sibling files),
//! for security purposes. They may not be symlinked.
//!
//! This hampers certain use cases, such as implementing
//! [`cli::GitExecutor`] for e.g. remote connections.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![cfg_attr(test, feature(async_closure))]
#![feature(impl_trait_in_assoc_type)]

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use integration_tests::*;

mod backend;
mod refspec;
mod repository;

#[cfg(feature = "cli")]
pub use backend::cli;
#[cfg(feature = "git2")]
pub use backend::git2;

pub use self::{
    refspec::{Error as RefSpecError, RefSpec},
    repository::{Authorization, ConfigScope, Error, Repository},
};
