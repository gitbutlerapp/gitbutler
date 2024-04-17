pub mod branch;
pub use branch::{Branch, BranchId};
pub mod target;

pub mod errors;

mod files;
pub use files::*;

pub mod integration;
pub use integration::GITBUTLER_INTEGRATION_REFERENCE;

mod base;
pub use base::*;

pub mod controller;
pub use controller::Controller;

mod iterator;
pub use iterator::BranchIterator as Iterator;

mod r#virtual;
pub use r#virtual::*;

mod remote;
pub use remote::*;

mod state;
pub use state::VirtualBranchesHandle;
