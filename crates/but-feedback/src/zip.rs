use std::{
    fs,
    io::{self, Read, Write},
    path,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use walkdir::{DirEntry, WalkDir};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Create a zip file from the *contents* of `src_dir` and write the zip file out to `dst_file`,
/// possibly overwriting it if it exists.
pub fn create_zip_file_from_dir(
    src_dir: impl AsRef<Path>,
    dst_file: impl AsRef<Path>,
) -> anyhow::Result<PathBuf> {
    let src_dir = src_dir.as_ref();
    let dst_file = dst_file.as_ref();
    if !src_dir.is_dir() {
        bail!("'{src}' is not a directory", src = src_dir.display());
    }

    let file = fs::File::create(dst_file)?;
    zip_dir(
        &mut WalkDir::new(src_dir).into_iter().filter_map(Result::ok),
        src_dir,
        file,
    )?;

    Ok(dst_file.to_owned())
}

/// Create a zip file with `src` content in a single-file archive, with the file named `src_file_name`,
/// and write the zip file out to `dst_file`, possibly overwriting it if it exists.
pub fn create_zip_file_from_content(
    src: &str,
    src_file_name: &str,
    dst_file: impl AsRef<Path>,
) -> anyhow::Result<PathBuf> {
    let dst_file = dst_file.as_ref();
    let mut zip = ZipWriter::new(fs::File::create(dst_file)?);

    zip.start_file_from_path(src_file_name, file_options())?;
    zip.write_all(src.as_bytes())?;
    zip.finish()?;

    Ok(dst_file.to_owned())
}

fn zip_dir<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &path::Path,
    writer: T,
) -> anyhow::Result<()>
where
    T: io::Write + io::Seek,
{
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o755);

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(prefix)?;

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            zip.start_file_from_path(name, options)?;
            let mut f = fs::File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            zip.add_directory_from_path(name, options)?;
        }
    }
    zip.finish()?;
    Result::Ok(())
}

fn file_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o755)
}
