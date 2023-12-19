#![feature(error_generic_member_access)]
#![cfg_attr(target_os = "windows", feature(windows_by_handle))]

pub mod analytics;
pub mod app;
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
pub mod logs;
pub mod menu;
pub mod paths;
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
pub(crate) mod windows;
pub mod writer;
pub mod zip;

#[cfg(test)]
pub mod test_utils;
