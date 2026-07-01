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
