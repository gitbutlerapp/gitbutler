#![feature(error_generic_member_access)]
#![cfg_attr(windows, feature(windows_by_handle))]
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

pub mod app;
pub mod commands;

pub mod logs;
pub mod menu;
pub mod watcher;

pub mod askpass;
pub mod config;
pub mod error;
pub mod github;
pub mod keys;
pub mod projects;
pub mod remotes;
pub mod undo;
pub mod users;
pub mod virtual_branches;
pub mod zip;
