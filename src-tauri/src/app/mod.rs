mod app;
pub mod deltas;
pub mod gb_repository;
pub mod project_repository;
pub mod projects;
pub mod pty;
pub mod reader;
pub mod search;
pub mod sessions;
pub mod users;
pub mod watcher;
mod writer;

#[cfg(test)]
mod reader_tests;

pub use app::{AddProjectError, App};
pub use project_repository::FileStatus;
