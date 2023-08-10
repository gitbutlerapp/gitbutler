pub mod branch;
pub use branch::Branch;
pub mod target;

mod integration;
pub use integration::GITBUTLER_INTEGRATION_BRANCH_NAME;

mod base;
pub use base::*;

pub mod controller;

pub mod commands;

mod iterator;
pub use iterator::BranchIterator as Iterator;

#[cfg(test)]
mod tests;

mod vbranch;
pub use vbranch::*;
