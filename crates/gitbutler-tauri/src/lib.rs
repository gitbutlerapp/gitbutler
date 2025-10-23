#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]
// FIXME(qix-): Stuff we want to fix but don't have a lot of time for.
// FIXME(qix-): PRs welcome!
#![allow(
    clippy::used_underscore_binding,
    clippy::module_name_repetitions,
    clippy::struct_field_names,
    clippy::too_many_lines
)]

pub mod claude;

pub mod logs;
pub mod menu;
pub mod window;
pub use window::state::{WindowState, event::ChangeForFrontend};

pub mod action;
pub mod askpass;
pub mod bot;
pub mod github;
pub mod projects;

pub mod settings;
pub mod zip;

pub mod env;

pub mod csp;

pub mod secret_migration;
