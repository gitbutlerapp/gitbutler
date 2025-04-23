pub mod entry;
mod oplog;
pub use oplog::OplogExt;
pub mod reflog;
mod snapshot;
pub use snapshot::SnapshotExt;
mod state;

/// The name of the file holding our state, useful for watching for changes.
pub const OPLOG_FILE_NAME: &str = "operations-log.toml";
