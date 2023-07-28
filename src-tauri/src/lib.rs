#[macro_use(defer)]
extern crate scopeguard;

pub mod actors_watcher;
pub mod app;
pub mod bookmarks;
pub mod database;
pub mod dedup;
pub mod deltas;
pub mod events;
pub mod files;
pub mod fs;
pub mod gb_repository;
pub mod project_repository;
pub mod projects;
pub mod pty;
pub mod reader;
pub mod search;
pub mod sessions;
pub mod storage;
pub mod users;
pub mod virtual_branches;
pub mod watcher;
pub mod writer;
pub mod zip;
