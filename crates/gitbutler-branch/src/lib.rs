//! GitButler internal library containing functionaliry related to branches, i.e. the virtual branches implementation
pub mod controller;
pub use controller::Controller;

pub mod r#virtual;
pub use r#virtual::*;

pub mod assets;

pub mod base;

pub mod integration;

pub mod files;

pub mod remote;
