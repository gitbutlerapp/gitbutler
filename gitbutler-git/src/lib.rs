//! GitButler utility library for pushing/fetching Git repositories
//! using the Git CLI.
//!
//! **Important Note:** This is an interim library. Please do not rely on it;
//! it's only used as a temporary measure in the GitButler app until we implement
//! a longer-term solution for managing Git operations.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![cfg_attr(test, feature(async_closure))]
#![feature(impl_trait_in_assoc_type)]

mod cli;
mod error;
mod refspec;

pub use self::{
    cli::*,
    error::Error,
    refspec::{Error as RefSpecError, RefSpec},
};
