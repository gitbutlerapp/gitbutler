pub mod branch;
pub use branch::{Branch, BranchId};
pub mod context;
pub mod target;

pub mod errors;

mod files;
pub use files::*;

mod integration;
pub use integration::GITBUTLER_INTEGRATION_REFERENCE;

mod base;
pub use base::*;

pub mod controller;
pub use controller::Controller;

pub mod commands;

mod iterator;
pub use iterator::BranchIterator as Iterator;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub use tests::set_test_target;

mod r#virtual;
pub use r#virtual::*;

mod remote;
pub use remote::*;

mod state;
