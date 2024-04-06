#![feature(error_generic_member_access)]
#![cfg_attr(windows, feature(windows_by_handle))]
#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]

pub mod analytics;
pub mod app;
pub mod commands;
pub mod events;
pub mod logs;
pub mod menu;
pub mod watcher;

pub mod askpass;
pub mod deltas;
pub mod error;
pub mod github;
pub mod keys;
pub mod projects;
pub mod sentry;
pub mod sessions;
pub mod users;
pub mod virtual_branches;
pub mod zip;
