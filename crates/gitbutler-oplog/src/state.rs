use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::Result;
use but_fs::read_toml_file_or_default;
use serde::{Deserialize, Deserializer, Serialize};

use super::OPLOG_FILE_NAME;

/// SystemTime used to be serialized as u64 of seconds, but is now a proper SystemTime struct.
/// This function will handle the old format gracefully.
fn unfailing_system_time_deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(SystemTime::deserialize(deserializer).unwrap_or(SystemTime::UNIX_EPOCH))
}

fn unix_epoch() -> SystemTime {
    SystemTime::UNIX_EPOCH
}

/// This tracks the head of the oplog, persisted in operations-log.toml.  
#[derive(Serialize, Deserialize, Debug)]
pub struct Oplog {
    /// This is the sha of the last oplog commit
    #[serde(with = "but_serde::oid_opt", default)]
    pub head_sha: Option<git2::Oid>,
    /// The time when the last snapshot was created. Seconds since Epoch
    #[serde(
        deserialize_with = "unfailing_system_time_deserialize",
        default = "unix_epoch"
    )]
    pub modified_at: SystemTime,
}

impl Default for Oplog {
    fn default() -> Self {
        Self {
            head_sha: None,
            modified_at: SystemTime::UNIX_EPOCH,
        }
    }
}

pub(crate) struct OplogHandle {
    /// The path to the file containing the oplog head state.
    file_path: PathBuf,
}

impl OplogHandle {
    /// Creates a new concurrency-safe handle to the state of the oplog.
    pub fn new(base_path: &Path) -> Self {
        let file_path = base_path.join(OPLOG_FILE_NAME);
        Self { file_path }
    }

    /// Persists the oplog head for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_oplog_head(&self, sha: git2::Oid) -> Result<()> {
        let mut oplog = self.read_file()?;
        oplog.head_sha = Some(sha);
        self.write_file(oplog)?;
        Ok(())
    }

    /// Gets the oplog head sha for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn oplog_head(&self) -> Result<Option<git2::Oid>> {
        let oplog = self.read_file()?;
        Ok(oplog.head_sha)
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<Oplog> {
        read_toml_file_or_default(&self.file_path)
    }

    fn write_file(&self, mut oplog: Oplog) -> Result<()> {
        oplog.modified_at = SystemTime::now();
        but_fs::write(&self.file_path, toml::to_string(&oplog)?)
    }
}
