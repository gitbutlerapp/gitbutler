//! File-backed storage for comments: a single JSON file in the project data directory
//! (`.git/gitbutler/comments.json` for CLI-registered projects).
//!
//! Comments are ephemeral, so the storage is deliberately the simplest thing that is safe under
//! concurrent GUI and CLI access: reads parse the whole file (writes go through atomic renames,
//! so a partial file is never observed), and every mutation re-reads the file under an exclusive
//! lock before rewriting it, so concurrent writers cannot clobber each other's changes.
//!
//! There is no downgrade protection: an older binary reads a newer file version as empty and
//! overwrites it on its next write — acceptable for data that is ephemeral by contract.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::DiffSide;

const FILE_NAME: &str = "comments.json";
const FILE_VERSION: u32 = 1;
/// Writers hold the lock for milliseconds; a lock file older than this was left behind by a
/// crashed process (`gix-lock` only cleans up on drop, not after SIGKILL) and would otherwise
/// brick all writes forever.
const STALE_LOCK_AGE: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// A comment as persisted in the comments file. See [`crate::DiffComment`] for the field
/// semantics; additionally `line_before`/`line_after` snapshot the same-side neighbouring diff
/// lines (when they existed) to disambiguate identical lines during re-anchoring, and
/// `archived_at_ms` marks archived comments, which are kept for a while so that archiving twice
/// stays a no-op, and purged eventually.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct StoredComment {
    pub id: String,
    pub path: String,
    pub commit_change_id: Option<String>,
    pub side: DiffSide,
    pub line_number: u32,
    pub line_content: String,
    pub line_before: Option<String>,
    pub line_after: Option<String>,
    pub payload: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub archived_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommentsFile {
    version: u32,
    comments: Vec<StoredComment>,
}

/// Handle on a project's comments file.
#[derive(Debug, Clone)]
pub struct CommentStore {
    file_path: PathBuf,
}

impl CommentStore {
    /// The store of the project whose data lives in `project_data_dir`
    /// (`.git/gitbutler` for CLI-registered projects).
    pub fn from_project_data_dir(project_data_dir: impl Into<PathBuf>) -> Self {
        CommentStore {
            file_path: project_data_dir.into().join(FILE_NAME),
        }
    }

    /// All stored comments, archived or not, in insertion order. A missing, unreadable, or
    /// incompatible file reads as an empty store — comments are ephemeral and must never take
    /// anything else down with them.
    pub fn read(&self) -> Vec<StoredComment> {
        let Ok(bytes) = std::fs::read(&self.file_path) else {
            return Vec::new();
        };
        match serde_json::from_slice::<CommentsFile>(&bytes) {
            Ok(file) if file.version == FILE_VERSION => file.comments,
            Ok(_) => Vec::new(),
            Err(err) => {
                tracing::warn!(
                    "ignoring unparsable comments file at {:?}: {err}",
                    self.file_path
                );
                Vec::new()
            }
        }
    }

    /// Mutate the stored comments under an exclusive lock and write them back atomically.
    /// When `mutate` fails, nothing is written.
    ///
    /// The file is re-read *inside* the lock, so mutations must be expressed against the
    /// freshest state (find a comment by id and update a field) — never as a write-back of
    /// previously read data. That discipline is what keeps concurrent GUI and CLI writers from
    /// clobbering each other.
    pub fn update<R>(
        &self,
        mutate: impl FnOnce(&mut Vec<StoredComment>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let parent = self
            .file_path
            .parent()
            .context("comments file has a parent directory")?;
        std::fs::create_dir_all(parent)?;
        self.remove_stale_lock();
        let mut lock = gix::lock::File::acquire_to_update_resource(
            &self.file_path,
            gix::lock::acquire::Fail::AfterDurationWithBackoff(std::time::Duration::from_millis(
                2500,
            )),
            None,
        )?;
        let mut comments = self.read();
        let result = mutate(&mut comments)?;
        let file = CommentsFile {
            version: FILE_VERSION,
            comments,
        };
        lock.write_all(serde_json::to_string_pretty(&file)?.as_bytes())?;
        lock.commit()
            .map_err(|err| err.error)
            .with_context(|| format!("failed to commit comments file at {:?}", self.file_path))?;
        Ok(result)
    }

    /// Remove a leftover lock file from a crashed process so writes don't stay bricked forever.
    fn remove_stale_lock(&self) {
        let mut lock_path = self.file_path.clone().into_os_string();
        lock_path.push(".lock");
        let lock_path = PathBuf::from(lock_path);
        let is_stale = std::fs::metadata(&lock_path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STALE_LOCK_AGE);
        if is_stale {
            tracing::warn!(?lock_path, "removing stale comments lock file");
            let _ = std::fs::remove_file(&lock_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: &str) -> StoredComment {
        StoredComment {
            id: id.to_string(),
            path: "src/a.rs".to_string(),
            commit_change_id: None,
            side: DiffSide::New,
            line_number: 15,
            line_content: "let x = 1;".to_string(),
            line_before: None,
            line_after: None,
            payload: "hello".to_string(),
            created_at_ms: 1000,
            updated_at_ms: 1000,
            archived_at_ms: None,
        }
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = CommentStore::from_project_data_dir(dir.path());
        assert_eq!(store.read(), Vec::new());
    }

    #[test]
    fn update_round_trips_and_rereads_fresh_state() -> anyhow::Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let store = CommentStore::from_project_data_dir(dir.path());
        store.update(|comments| {
            comments.push(stored("1"));
            Ok(())
        })?;

        // A second handle sees the write, and its mutation is applied to the freshest state
        // rather than whatever the first handle had in memory.
        let other = CommentStore::from_project_data_dir(dir.path());
        other.update(|comments| {
            comments.push(stored("2"));
            Ok(())
        })?;

        let ids: Vec<String> = store.read().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["1", "2"]);
        Ok(())
    }

    #[test]
    fn failing_mutation_writes_nothing() -> anyhow::Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let store = CommentStore::from_project_data_dir(dir.path());
        store.update(|comments| {
            comments.push(stored("1"));
            Ok(())
        })?;

        let result: anyhow::Result<()> = store.update(|comments| {
            comments.clear();
            anyhow::bail!("nope");
        });
        assert!(result.is_err(), "the mutation error is propagated");
        assert_eq!(
            store.read().len(),
            1,
            "the failed mutation is not persisted"
        );
        Ok(())
    }

    #[test]
    fn stale_locks_are_broken() -> anyhow::Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let store = CommentStore::from_project_data_dir(dir.path());

        let lock_path = dir.path().join(format!("{FILE_NAME}.lock"));
        std::fs::write(&lock_path, b"")?;
        let stale = std::time::SystemTime::now() - (STALE_LOCK_AGE + STALE_LOCK_AGE);
        std::fs::File::options()
            .write(true)
            .open(&lock_path)?
            .set_modified(stale)?;

        // The stale lock is removed and the write succeeds instead of timing out.
        store.update(|comments| {
            comments.push(stored("1"));
            Ok(())
        })?;
        assert_eq!(store.read().len(), 1);
        Ok(())
    }

    #[test]
    fn incompatible_or_corrupt_files_read_as_empty() -> anyhow::Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let store = CommentStore::from_project_data_dir(dir.path());

        std::fs::write(dir.path().join(FILE_NAME), b"not json at all")?;
        assert_eq!(store.read(), Vec::new());

        std::fs::write(
            dir.path().join(FILE_NAME),
            br#"{"version":999,"comments":[]}"#,
        )?;
        assert_eq!(store.read(), Vec::new());
        Ok(())
    }
}
