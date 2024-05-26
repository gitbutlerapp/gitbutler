pub mod entry;
pub mod oplog;
mod reflog;
pub mod snapshot;
mod state;

/// The name of the file holding our state, useful for watching for changes.
pub const OPLOG_FILE_NAME: &str = "operations-log.toml";
