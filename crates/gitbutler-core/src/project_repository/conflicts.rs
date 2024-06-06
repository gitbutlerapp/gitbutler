// stuff to manage merge conflict state
// this is the dumbest possible way to do this, but it is a placeholder
// conflicts are stored one path per line in .git/conflicts
// merge parent is stored in .git/base_merge_parent
// conflicts are removed as they are resolved, the conflicts file is removed when there are no more conflicts
// the merge parent file is removed when the merge is complete

use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use itertools::Itertools;

use super::Repository;

pub fn mark<P: AsRef<Path>, A: AsRef<[P]>>(
    repository: &Repository,
    paths: A,
    parent: Option<git2::Oid>,
) -> Result<()> {
    let paths = paths.as_ref();
    if paths.is_empty() {
        return Ok(());
    }
    let conflicts_path = repository.repo().path().join("conflicts");
    // write all the file paths to a file on disk
    let mut file = std::fs::File::create(conflicts_path)?;
    for path in paths {
        file.write_all(path.as_ref().as_os_str().as_encoded_bytes())?;
        file.write_all(b"\n")?;
    }

    if let Some(parent) = parent {
        let merge_path = repository.repo().path().join("base_merge_parent");
        // write all the file paths to a file on disk
        let mut file = std::fs::File::create(merge_path)?;
        file.write_all(parent.to_string().as_bytes())?;
    }

    Ok(())
}

pub fn merge_parent(repository: &Repository) -> Result<Option<git2::Oid>> {
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

pub fn resolve<P: AsRef<Path>>(repository: &Repository, path: P) -> Result<()> {
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

    // remove file
    std::fs::remove_file(conflicts_path)?;

    // re-write file if needed
    if !remaining.is_empty() {
        mark(repository, &remaining, None)?;
    }
    Ok(())
}

pub fn conflicting_files(repository: &Repository) -> Result<Vec<String>> {
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
pub fn is_conflicting(repository: &Repository, path: Option<&Path>) -> Result<bool> {
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
pub fn is_resolving(repository: &Repository) -> bool {
    repository.repo().path().join("base_merge_parent").exists()
}

pub fn clear(repository: &Repository) -> Result<()> {
    let merge_path = repository.repo().path().join("base_merge_parent");
    std::fs::remove_file(merge_path)?;

    for file in conflicting_files(repository)? {
        resolve(repository, &file)?;
    }

    Ok(())
}
