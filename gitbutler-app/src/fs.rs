use std::path::{Path, PathBuf};

use anyhow::Result;
use walkdir::WalkDir;

// Returns an ordered list of relative paths for files inside a directory recursively.
pub fn list_files<P: AsRef<Path>>(dir_path: P, ignore_prefixes: &[P]) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    let dir_path = dir_path.as_ref();
    if !dir_path.exists() {
        return Ok(files);
    }
    for entry in WalkDir::new(dir_path) {
        let entry = entry?;
        if !entry.file_type().is_dir() {
            let path = entry.path();
            let path = path.strip_prefix(dir_path)?;
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
