//! A place for each command, i.e. `but foo` as `pub mod foo` here.
#[cfg(feature = "legacy")]
pub mod legacy;

pub mod completions;
pub mod forge;
pub mod gui;
pub mod help;
pub mod push;
