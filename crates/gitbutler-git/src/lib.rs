//! Native Git clone, fetch, and push operations with asynchronous authentication.
#![deny(missing_docs, unsafe_code)]
#![allow(async_fn_in_trait)]
#![allow(unknown_lints)]

#[cfg(feature = "tokio")]
mod context;
mod error;
/// Authentication socket and helper identity-checking utilities.
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
