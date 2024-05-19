pub mod entry;
pub mod oplog;
mod reflog;
pub mod snapshot;
mod state;

pub const OPLOG_FILE_NAME: &str = "operations-log.toml";
