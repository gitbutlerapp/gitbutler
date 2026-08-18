//! Repository setup helpers for debug commands.

use std::path::Path;

use anyhow::Result;

use crate::args::Args;

/// Discover the repository for the provided CLI arguments.
pub(crate) fn repo_from_args(args: &Args) -> Result<gix::Repository> {
    repo_from_path(&args.current_dir)
}

/// Discover the repository located at or above `path`.
pub(crate) fn repo_from_path(path: &Path) -> Result<gix::Repository> {
    Ok(gix::discover(path)?)
}

/// A throwaway database handle: debug commands inspect arbitrary repositories
/// without a project database, and worktree discovery stays off.
pub(crate) fn debug_db() -> Result<but_db::DbHandle> {
    but_db::DbHandle::new_at_path(":memory:")
}
