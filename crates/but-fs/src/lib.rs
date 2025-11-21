use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use gix::tempfile::{AutoRemove, ContainingDirectory, create_dir::Retries};
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

#[cfg(feature = "legacy")]
mod legacy {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    /// A facility to read, write and delete files.
    #[derive(Debug, Clone)]
    pub struct Storage {
        /// The directory into which all of or files will be written or read-from.
        local_data_dir: PathBuf,
    }

    impl Storage {
        pub fn new(local_data_dir: impl Into<PathBuf>) -> Storage {
            Storage {
                local_data_dir: local_data_dir.into(),
            }
        }

        /// Read the content of the file at `rela_path` which is a path relative to our root directory.
        /// Return `Ok(None)` if the file doesn't exist.
        // TODO(ST): make all these operations write bytes.
        pub fn read(&self, rela_path: impl AsRef<Path>) -> std::io::Result<Option<String>> {
            match fs::read_to_string(self.local_data_dir.join(rela_path)) {
                Ok(content) => Ok(Some(content)),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(err),
            }
        }

        /// Write `content` to `rela_path` atomically, so it's either written completely, or not at all.
        /// Creates the file and intermediate directories.
        ///
        /// ### On Synchronization
        ///
        /// Mutating operations are assumed to be synchronized by the caller,
        /// even though all writes will be atomic.
        ///
        /// If these operations are not synchronized, they will be racy as it's undefined
        /// which *whole* write will win. Thus, operations which touch multiple files and
        /// need them to be consistent *need* to synchronize by some mean.
        ///
        /// Generally, the filesystem is used for synchronization, not in-memory primitives.
        pub fn write(&self, rela_path: impl AsRef<Path>, content: &str) -> std::io::Result<()> {
            let file_path = self.local_data_dir.join(rela_path);
            crate::create_dirs_then_write(file_path, content)
        }

        /// Delete the file or directory at `rela_path`.
        ///
        /// ### Panics
        ///
        /// If a symlink is encountered.
        pub fn delete(&self, rela_path: impl AsRef<Path>) -> std::io::Result<()> {
            let file_path = self.local_data_dir.join(rela_path);
            let md = match file_path.symlink_metadata() {
                Ok(md) => md,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(err) => return Err(err),
            };

            if md.is_dir() {
                fs::remove_dir_all(file_path)?;
            } else if md.is_file() {
                fs::remove_file(file_path)?;
            } else {
                unreachable!("BUG: we do not create or work with symlinks")
            }
            Ok(())
        }
    }
}
#[cfg(feature = "legacy")]
pub use legacy::Storage;

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
