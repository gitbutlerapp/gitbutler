pub mod entry;
mod oplog;
pub use oplog::peel_restore_snapshot;
pub use oplog::{LocalTargetSnapshot, OplogExt, RestoreKind};
mod reflog;
mod snapshot;
pub use snapshot::SnapshotExt;
mod state;

/// The name of the file holding our state, useful for watching for changes.
const OPLOG_FILE_NAME: &str = "operations-log.toml";
