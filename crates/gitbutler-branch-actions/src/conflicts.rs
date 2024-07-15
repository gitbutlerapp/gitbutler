/// stuff to manage merge conflict state.
/// This is the dumbest possible way to do this, but it is a placeholder.
/// Conflicts are stored one path per line in .git/conflicts.
/// Merge parent is stored in .git/base_merge_parent.
/// Conflicts are removed as they are resolved, the conflicts file is removed when there are no more conflicts
/// or when the merge is complete.
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use gitbutler_command_context::ProjectRepository;
use itertools::Itertools;

use gitbutler_error::error::Marker;

pub(crate) fn mark<P: AsRef<Path>, A: AsRef<[P]>>(
    repository: &ProjectRepository,
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
        buf.write_all(path.as_ref().as_os_str().as_encoded_bytes())?;
        buf.write_all(b"\n")?;
    }
    gitbutler_fs::write(repository.repo().path().join("conflicts"), buf)?;

    if let Some(parent) = parent {
        // write all the file paths to a file on disk
        gitbutler_fs::write(
            repository.repo().path().join("base_merge_parent"),
            parent.to_string().as_bytes(),
        )?;
    }

    Ok(())
}

pub(crate) fn merge_parent(repository: &ProjectRepository) -> Result<Option<git2::Oid>> {
    let merge_path = repository.repo().path().join("base_merge_parent");
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

pub fn resolve<P: AsRef<Path>>(repository: &ProjectRepository, path: P) -> Result<()> {
    let path = path.as_ref();
    let conflicts_path = repository.repo().path().join("conflicts");
    let file = std::fs::File::open(conflicts_path.clone())?;
    let reader = std::io::BufReader::new(file);
    let mut remaining = Vec::new();
    for line in reader.lines().map_ok(PathBuf::from) {
        let line = line?;
        if line != path {
            remaining.push(line);
        }
    }

    // re-write file if needed, otherwise remove file entirely
    if remaining.is_empty() {
        std::fs::remove_file(conflicts_path)?;
    } else {
        mark(repository, &remaining, None)?;
    }
    Ok(())
}

pub(crate) fn conflicting_files(repository: &ProjectRepository) -> Result<Vec<String>> {
    let conflicts_path = repository.repo().path().join("conflicts");
    if !conflicts_path.exists() {
        return Ok(vec![]);
    }

    let file = std::fs::File::open(conflicts_path)?;
    let reader = std::io::BufReader::new(file);
    Ok(reader.lines().map_while(Result::ok).collect())
}

/// Check if `path` is conflicting in `repository`, or if `None`, check if there is any conflict.
// TODO(ST): Should this not rather check the conflicting state in the index?
pub(crate) fn is_conflicting(repository: &ProjectRepository, path: Option<&Path>) -> Result<bool> {
    let conflicts_path = repository.repo().path().join("conflicts");
    if !conflicts_path.exists() {
        return Ok(false);
    }

    let file = std::fs::File::open(conflicts_path)?;
    let reader = std::io::BufReader::new(file);
    // TODO(ST): This shouldn't work on UTF8 strings.
    let mut files = reader.lines().map_ok(PathBuf::from);
    if let Some(pathname) = path {
        // check if pathname is one of the lines in conflicts_path file
        for line in files {
            let line = line?;

            if line == pathname {
                return Ok(true);
            }
        }
        Ok(false)
    } else {
        Ok(files.next().transpose().map(|x| x.is_some())?)
    }
}

// is this project still in a resolving conflict state?
// - could be that there are no more conflicts, but the state is not committed
pub(crate) fn is_resolving(repository: &ProjectRepository) -> bool {
    repository.repo().path().join("base_merge_parent").exists()
}

pub(crate) fn clear(repository: &ProjectRepository) -> Result<()> {
    let merge_path = repository.repo().path().join("base_merge_parent");
    std::fs::remove_file(merge_path)?;

    for file in conflicting_files(repository)? {
        resolve(repository, &file)?;
    }

    Ok(())
}

pub(crate) trait RepoConflictsExt {
    fn assure_unconflicted(&self) -> Result<()>;
    fn assure_resolved(&self) -> Result<()>;
    fn is_resolving(&self) -> bool;
}

impl RepoConflictsExt for ProjectRepository {
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
