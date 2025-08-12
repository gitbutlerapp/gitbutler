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
pub mod commands;

pub mod logs;
pub mod menu;
pub mod window;
pub use window::state::event::ChangeForFrontend;
pub use window::state::WindowState;

pub mod action;
pub mod askpass;
pub mod bot;
pub mod cli;
pub mod config;
pub mod forge;
pub mod github;
pub mod modes;
pub mod open;
pub mod projects;
pub mod remotes;
pub mod repo;
pub mod rules;
pub mod secret;
pub mod undo;
pub mod users;
pub mod virtual_branches;

pub mod settings;
pub mod stack;
pub mod zip;

pub mod diff;
pub mod env;
pub mod workspace;

pub mod csp;
