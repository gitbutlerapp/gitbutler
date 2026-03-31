use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use tempfile::TempDir;

/// Create a temp directory with a `hooks/` subdirectory, returning both.
///
/// This is the shared helper used by both `hook_manager` and `managed_hooks`
/// test modules. It returns a `Result` so callers can propagate errors or
/// `.unwrap()` depending on whether the test function returns `Result`.
pub fn create_hooks_dir() -> Result<(TempDir, PathBuf)> {
    let dir = TempDir::new()?;
    let hooks_dir = dir.path().join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    Ok((dir, hooks_dir))
}
