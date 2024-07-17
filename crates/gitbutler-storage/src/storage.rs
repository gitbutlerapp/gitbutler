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
        gitbutler_fs::create_dirs_then_write(file_path, content)
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
