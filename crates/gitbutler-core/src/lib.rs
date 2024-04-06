#![feature(error_generic_member_access)]
#![cfg_attr(windows, feature(windows_by_handle))]
#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]

pub mod askpass;
pub mod assets;
pub mod database;
pub mod dedup;
pub mod deltas;
pub mod error;
pub mod fs;
pub mod gb_repository;
pub mod git;
pub mod id;
pub mod keys;
pub mod lock;
pub mod path;
pub mod project_repository;
pub mod projects;
pub mod reader;
pub mod sessions;
pub mod ssh;
pub mod storage;
pub mod types;
pub mod users;
pub mod virtual_branches;
#[cfg(target_os = "windows")]
pub mod windows;
pub mod writer;
pub mod zip;
