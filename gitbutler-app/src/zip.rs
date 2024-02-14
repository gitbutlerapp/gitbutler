pub mod commands;
mod controller;
pub use controller::Controller;

use std::{
    fs,
    io::{self, Read, Write},
    path, time,
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use walkdir::{DirEntry, WalkDir};
use zip::{result::ZipError, write, CompressionMethod, ZipWriter};

#[derive(Clone)]
pub struct Zipper {
    cache: path::PathBuf,
}

impl TryFrom<&AppHandle> for Zipper {
    type Error = anyhow::Error;

    fn try_from(handle: &AppHandle) -> Result<Self> {
        if let Some(zipper) = handle.try_state::<Self>() {
            Ok(zipper.inner().clone())
        } else {
            let app_cache_dir = handle
                .path_resolver()
                .app_cache_dir()
                .context("failed to get app cache dir")?;
            Self::new(app_cache_dir)
        }
    }
}

impl Zipper {
    fn new<P: AsRef<path::Path>>(cache_dir: P) -> Result<Self, anyhow::Error> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir).context("failed to create cache dir")?;
        let cache = cache_dir.join("archives");
        Ok(Self { cache })
    }

    // takes a path to create zip of, returns path of a created archive.
    pub fn zip<P: AsRef<path::Path>>(&self, path: P) -> Result<path::PathBuf> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(anyhow::anyhow!("{} does not exist", path.display()));
        }
        if !path.is_dir() {
            return Err(anyhow::anyhow!("{} is not a directory", path.display()));
        }
        let path_hash = calculate_path_hash(path)?;
        fs::create_dir_all(&self.cache).context("failed to create cache dir")?;
        let archive_path = self.cache.join(format!("{}.zip", path_hash));
        if !archive_path.exists() {
            doit(path, &archive_path, CompressionMethod::Bzip2)?;
        }
        Ok(archive_path)
    }
}

fn doit<P: AsRef<path::Path>>(
    src_dir: P,
    dst_file: P,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<()> {
    let src = src_dir.as_ref();
    let dst = dst_file.as_ref();
    if !src.is_dir() {
        return Err(ZipError::FileNotFound);
    }

    let file = fs::File::create(dst).unwrap();

    let walkdir = WalkDir::new(src);
    let it = walkdir.into_iter();

    zip_dir(&mut it.filter_map(Result::ok), src, file, method)?;

    Ok(())
}

fn zip_dir<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &path::Path,
    writer: T,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<()>
where
    T: io::Write + io::Seek,
{
    let mut zip = ZipWriter::new(writer);
    let options = write::FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o755);

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(prefix).unwrap();

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            #[allow(deprecated)]
            zip.start_file_from_path(name, options)?;
            let mut f = fs::File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            #[allow(deprecated)]
            zip.add_directory_from_path(name, options)?;
        }
    }
    zip.finish()?;
    Result::Ok(())
}

// returns hash of a path by calculating metadata hash of all files in it.
fn calculate_path_hash<P: AsRef<path::Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    let mut hasher = Sha256::new();

    if path.is_dir() {
        let entries = fs::read_dir(path)?;
        let mut entry_paths: Vec<_> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        entry_paths.sort();

        for entry_path in entry_paths {
            file_hash(&mut hasher, &entry_path).with_context(|| {
                format!(
                    "failed to calculate hash of file {}",
                    entry_path.to_str().unwrap()
                )
            })?;
        }
    } else if path.is_file() {
        file_hash(&mut hasher, path).with_context(|| {
            format!(
                "failed to calculate hash of file {}",
                path.to_str().unwrap()
            )
        })?;
    }

    Ok(format!("{:X}", hasher.finalize()))
}

fn file_hash<P: AsRef<path::Path>>(digest: &mut Sha256, path: P) -> Result<()> {
    let path = path.as_ref();
    let metadata = fs::metadata(path).context("failed to get metadata")?;
    digest.update(path.to_str().unwrap().as_bytes());
    digest.update(metadata.len().to_string().as_bytes());
    digest.update(
        metadata
            .modified()
            .unwrap_or(time::UNIX_EPOCH)
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
            .as_bytes(),
    );
    digest.update(
        metadata
            .created()
            .unwrap_or(time::UNIX_EPOCH)
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
            .as_bytes(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_zip_dir() {
        let tmp_dir = tempdir().unwrap();
        let tmp_dir_path = tmp_dir.path();
        let file_path = tmp_dir_path.join("test.txt");
        let mut file = File::create(file_path).unwrap();
        file.write_all(b"test").unwrap();

        let zipper_cache = tempdir().unwrap();
        let zipper = Zipper::new(zipper_cache.path()).unwrap();
        let zip_file_path = zipper.zip(tmp_dir).unwrap();
        assert!(zip_file_path.exists());
    }

    #[test]
    fn test_zip_file() {
        let tmp_dir = tempdir().unwrap();
        let tmp_dir_path = tmp_dir.path();
        let file_path = tmp_dir_path.join("test.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test").unwrap();

        let zipper_cache = tempdir().unwrap();
        let zipper = Zipper::new(zipper_cache.path()).unwrap();
        zipper.zip(file_path).unwrap_err();
    }

    #[test]
    fn test_zip_once() {
        let tmp_dir = tempdir().unwrap();
        let tmp_dir_path = tmp_dir.path();
        let file_path = tmp_dir_path.join("test.txt");
        let mut file = File::create(file_path).unwrap();
        file.write_all(b"test").unwrap();

        let zipper_cache = tempdir().unwrap();
        let zipper = Zipper::new(zipper_cache.path()).unwrap();
        assert_eq!(zipper.zip(&tmp_dir).unwrap(), zipper.zip(&tmp_dir).unwrap());
        assert_eq!(WalkDir::new(tmp_dir).into_iter().count(), 1);
    }
}
