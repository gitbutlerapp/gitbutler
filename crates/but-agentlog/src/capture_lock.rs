use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use but_core::{RepositoryExt, sync::LockFile};

const CAPTURE_LOCK_FILE: &str = "agentlog-capture.lock";

pub(crate) fn with_capture_lock<T>(
    repo_path: &Path,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let _lock = acquire_capture_lock(repo_path)?;
    work()
}

fn acquire_capture_lock(repo_path: &Path) -> Result<LockFile> {
    let lock_path = capture_lock_path(repo_path)?;
    let mut file_lock = LockFile::open(&lock_path)
        .with_context(|| format!("failed to open capture lock '{}'", lock_path.display()))?;
    file_lock
        .lock()
        .with_context(|| format!("failed to acquire capture lock '{}'", lock_path.display()))?;

    Ok(file_lock)
}

fn capture_lock_path(repo_path: &Path) -> Result<PathBuf> {
    let repo = gix::discover(repo_path).context("failed to discover Git repository")?;
    let storage_path = repo
        .gitbutler_storage_path()
        .context("failed to locate GitButler storage path")?;
    std::fs::create_dir_all(&storage_path).with_context(|| {
        format!(
            "failed to create GitButler storage directory '{}'",
            storage_path.display()
        )
    })?;
    Ok(storage_path.join(CAPTURE_LOCK_FILE))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use but_core::sync::LockFile;

    use super::{acquire_capture_lock, capture_lock_path};

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        gix::init(dir.path()).expect("gitoxide repo init");
        dir
    }

    #[test]
    fn capture_lock_blocks_second_entrant() {
        let repo = setup_repo();
        let first = acquire_capture_lock(repo.path()).expect("first capture lock");
        let mut second = LockFile::open(capture_lock_path(repo.path()).expect("capture lock path"))
            .expect("second lock file");
        assert!(
            !second.try_lock().expect("try second capture lock"),
            "second lock attempt should fail while first capture lock is held"
        );

        drop(first);
        assert!(
            second.try_lock().expect("retry second capture lock"),
            "capture lock should be released on drop"
        );
    }
}
