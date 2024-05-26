pub mod entry;
mod oplog;
mod reflog;
mod snapshot;
mod state;

/// The name of the file holding our state, useful for watching for changes.
pub const OPLOG_FILE_NAME: &str = "operations-log.toml";
