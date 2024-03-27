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

pub mod analytics;
pub mod app;
pub mod askpass;
pub mod assets;
pub mod commands;
pub mod database;
pub mod dedup;
pub mod deltas;
pub mod error;
pub mod events;
pub mod fs;
pub mod gb_repository;
pub mod git;
pub mod github;
pub mod id;
pub mod keys;
pub mod lock;
pub mod logs;
pub mod menu;
pub mod path;
pub mod project_repository;
pub mod projects;
pub mod reader;
pub mod sentry;
pub mod sessions;
pub mod ssh;
pub mod storage;
pub mod types;
pub mod users;
pub mod virtual_branches;
pub mod watcher;
#[cfg(target_os = "windows")]
pub mod windows;
pub mod writer;
pub mod zip;
