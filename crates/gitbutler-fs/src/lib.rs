use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use bstr::BString;
use gix::{
    dir::walk::EmissionMode,
    tempfile::{create_dir::Retries, AutoRemove, ContainingDirectory},
};
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

// Returns an ordered list of relative paths for files inside a directory recursively.
pub fn list_files<P: AsRef<Path>>(
    dir_path: P,
    ignore_prefixes: &[P],
    recursive: bool,
    remove_prefix: Option<P>,
) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    let dir_path = dir_path.as_ref();
    if !dir_path.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(dir_path).max_depth(if recursive { usize::MAX } else { 1 }) {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            let path = entry.path();

            let path = if let Some(prefix) = remove_prefix.as_ref() {
                path.strip_prefix(prefix)?
            } else {
                path
            };

            let path = path.to_path_buf();
            if ignore_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix.as_ref()))
            {
                continue;
            }
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

// Return an iterator of worktree-relative slash-separated paths for files inside the `worktree_dir`, recursively.
// Fails if the `worktree_dir` isn't a valid git repository.
pub fn iter_worktree_files(
    worktree_dir: impl AsRef<Path>,
) -> Result<impl Iterator<Item = BString>> {
    let repo = gix::open(worktree_dir.as_ref())?;
    let index = repo.index_or_empty()?;
    let disabled_interrupt_handling = Default::default();
    let options = repo
        .dirwalk_options()?
        .emit_tracked(true)
        .emit_untracked(EmissionMode::Matching);
    Ok(repo
        .dirwalk_iter(index, None::<&str>, disabled_interrupt_handling, options)?
        .filter_map(Result::ok)
        .map(|e| e.entry.rela_path))
}

/// Write a single file so that the write either fully succeeds, or fully fails,
/// assuming the containing directory already exists.
pub fn write<P: AsRef<Path>>(file_path: P, contents: impl AsRef<[u8]>) -> anyhow::Result<()> {
    let mut temp_file = gix::tempfile::new(
        file_path.as_ref().parent().unwrap(),
        ContainingDirectory::Exists,
        AutoRemove::Tempfile,
    )?;
    temp_file.write_all(contents.as_ref())?;
    Ok(persist_tempfile(temp_file, file_path)?)
}

/// Write a single file so that the write either fully succeeds, or fully fails,
/// and create all leading directories.
pub fn create_dirs_then_write<P: AsRef<Path>>(
    file_path: P,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    let mut temp_file = gix::tempfile::new(
        file_path.as_ref().parent().unwrap(),
        ContainingDirectory::CreateAllRaceProof(Retries::default()),
        AutoRemove::Tempfile,
    )?;
    temp_file.write_all(contents.as_ref())?;
    persist_tempfile(temp_file, file_path)
}

fn persist_tempfile(
    tempfile: gix::tempfile::Handle<gix::tempfile::handle::Writable>,
    to_path: impl AsRef<Path>,
) -> std::io::Result<()> {
    match tempfile.persist(to_path) {
        Ok(Some(_opened_file)) => Ok(()),
        Ok(None) => unreachable!(
            "BUG: a signal has caused the tempfile to be removed, but we didn't install a handler"
        ),
        Err(err) => Err(err.error),
    }
}

/// Reads and parses the state file.
///
/// If the file does not exist, it will be created.
pub fn read_toml_file_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(err) => return Err(err.into()),
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let value: T =
        toml::from_str(&contents).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(value)
}
