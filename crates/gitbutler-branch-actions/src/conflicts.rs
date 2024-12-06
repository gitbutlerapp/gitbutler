use std::ffi::OsStr;
/// stuff to manage merge conflict state.
/// This is the dumbest possible way to do this, but it is a placeholder.
/// Conflicts are stored one path per line in .git/conflicts.
/// Merge parent is stored in .git/base_merge_parent.
/// Conflicts are removed as they are resolved, the conflicts file is removed when there are no more conflicts
/// or when the merge is complete.
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use bstr::ByteSlice;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Marker;

pub(crate) fn mark<P: AsRef<Path>, A: AsRef<[P]>>(
    ctx: &CommandContext,
    paths: A,
    parent: Option<git2::Oid>,
) -> Result<()> {
    let paths = paths.as_ref();
    if paths.is_empty() {
        return Ok(());
    }
    // write all the file paths to a file on disk
    let mut buf = Vec::<u8>::with_capacity(512);
    for path in paths {
        let path = path.as_ref();
        let path_bytes = path.as_os_str().as_encoded_bytes();
        // Have to search for line separators as `ByteSlice::lines()` will recognize both.
        if path_bytes.find_byte(b'\n').is_some() || path_bytes.find(b"\r\n").is_some() {
            bail!("Conflicting path {path:?} contains newlines which are illegal in this context")
        }
        buf.write_all(path_bytes)?;
        buf.write_all(b"\n")?;
    }
    gitbutler_fs::write(conflicts_path(ctx), &buf)?;

    if let Some(parent) = parent {
        // write all the file paths to a file on disk
        gitbutler_fs::write(merge_parent_path(ctx), parent.to_string().as_bytes())?;
    }
    Ok(())
}

fn conflicts_path(ctx: &CommandContext) -> PathBuf {
    ctx.repo().path().join("conflicts")
}

fn merge_parent_path(ctx: &CommandContext) -> PathBuf {
    ctx.repo().path().join("base_merge_parent")
}

pub(crate) fn merge_parent(ctx: &CommandContext) -> Result<Option<git2::Oid>> {
    use std::io::BufRead;

    let merge_path = merge_parent_path(ctx);
    if !merge_path.exists() {
        return Ok(None);
    }

    let file = std::fs::File::open(merge_path)?;
    let reader = std::io::BufReader::new(file);
    let mut lines = reader.lines();
    if let Some(parent) = lines.next() {
        let parent = parent?;
        let parent: git2::Oid = parent.parse()?;
        Ok(Some(parent))
    } else {
        Ok(None)
    }
}

pub fn resolve<P: AsRef<Path>>(ctx: &CommandContext, path_to_resolve: P) -> Result<()> {
    let path_to_resolve = path_to_resolve.as_ref();
    let path_to_resolve = path_to_resolve.as_os_str().as_encoded_bytes();
    let conflicts_path = conflicts_path(ctx);
    let path_per_line = std::fs::read(&conflicts_path)?;
    let remaining: Vec<_> = path_per_line
        .lines()
        .filter(|path| *path != path_to_resolve)
        .map(|path| unsafe { OsStr::from_encoded_bytes_unchecked(path) })
        .collect();

    // re-write file if needed, otherwise remove file entirely
    if remaining.is_empty() {
        std::fs::remove_file(conflicts_path)?;
    } else {
        mark(ctx, &remaining, None)?;
    }
    Ok(())
}

pub(crate) fn conflicting_files(ctx: &CommandContext) -> Result<Vec<PathBuf>> {
    let conflicts_path = conflicts_path(ctx);
    if !conflicts_path.exists() {
        return Ok(vec![]);
    }

    let path_per_line = std::fs::read(conflicts_path)?;
    Ok(path_per_line
        .lines()
        .map(|path| unsafe { OsStr::from_encoded_bytes_unchecked(path) }.into())
        .collect())
}

/// Check if `path` is conflicting in `repository`, or if `None`, check if there is any conflict.
// TODO(ST): Should this not rather check the conflicting state in the index?
pub(crate) fn is_conflicting(repository: &CommandContext, path: Option<&Path>) -> Result<bool> {
    let conflicts_path = conflicts_path(repository);
    if !conflicts_path.exists() {
        return Ok(false);
    }

    let path_per_line = std::fs::read(conflicts_path)?;
    let mut files = path_per_line
        .lines()
        .map(|path| unsafe { OsStr::from_encoded_bytes_unchecked(path) });
    let is_in_conflicts_file_or_has_conflicts = if let Some(path) = path {
        files.any(|p| p == path)
    } else {
        files.next().is_some()
    };
    Ok(is_in_conflicts_file_or_has_conflicts)
}

// is this project still in a resolving conflict state?
// - could be that there are no more conflicts, but the state is not committed
pub(crate) fn is_resolving(ctx: &CommandContext) -> bool {
    merge_parent_path(ctx).exists()
}

pub(crate) fn clear(ctx: &CommandContext) -> Result<()> {
    remove_file_ignore_missing(merge_parent_path(ctx))?;
    remove_file_ignore_missing(conflicts_path(ctx))?;
    Ok(())
}

fn remove_file_ignore_missing(path: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::remove_file(path).or_else(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(err)
        }
    })
}

pub(crate) trait RepoConflictsExt {
    fn assure_unconflicted(&self) -> Result<()>;
    fn assure_resolved(&self) -> Result<()>;
    fn is_resolving(&self) -> bool;
}

impl RepoConflictsExt for CommandContext {
    fn is_resolving(&self) -> bool {
        is_resolving(self)
    }

    fn assure_resolved(&self) -> Result<()> {
        if self.is_resolving() {
            Err(anyhow!("project has active conflicts")).context(Marker::ProjectConflict)
        } else {
            Ok(())
        }
    }

    fn assure_unconflicted(&self) -> Result<()> {
        if is_conflicting(self, None)? {
            Err(anyhow!("project has active conflicts")).context(Marker::ProjectConflict)
        } else {
            Ok(())
        }
    }
}
