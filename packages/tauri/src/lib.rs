#![forbid(unsafe_code)]
#![deny(
    clippy::redundant_closure_for_method_calls,
    clippy::manual_string_new,
    clippy::implicit_clone,
    clippy::map_unwrap_or,
    clippy::needless_for_each
)]

pub mod analytics;
pub mod app;
pub mod assets;
pub mod bookmarks;
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
pub mod keys;
pub mod lock;
pub mod logs;
pub mod paths;
pub mod project_repository;
pub mod projects;
pub mod reader;
pub mod search;
pub mod sessions;
pub mod storage;
pub mod users;
pub mod virtual_branches;
pub mod watcher;
pub mod writer;
pub mod zip;

#[cfg(test)]
pub mod test_utils;
