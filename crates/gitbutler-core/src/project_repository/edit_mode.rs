// stuff to manage edit mode state
// if in edit mode, the commit sha that is checked out is in .git/gitbutler/edit_sha

use std::{io::Write, str::FromStr};
use anyhow::Result;

use super::Repository;
use crate::git;

pub fn set_edit_mode(
  repository: &Repository,
  edit_sha: &git::Oid,
  restore_snapshot_sha: Option<String>
) -> Result<()> {
    let edit_mode_path = repository.git_repository.path().join("gitbutler").join("edit_sha");

    // write all the file paths to a file on disk
    let mut file = std::fs::File::create(edit_mode_path)?;
    file.write_all(edit_sha.to_string().as_bytes())?;
    file.write_all(b"\n")?;

    if let Some(restore_snapshot_sha) = restore_snapshot_sha {
      let restore_path = repository.git_repository.path().join("gitbutler").join("restore_sha");
      let mut file = std::fs::File::create(restore_path)?;
      file.write_all(restore_snapshot_sha.as_bytes())?;
      file.write_all(b"\n")?;
    }

    Ok(())
}

pub fn clear_edit_mode(
  repository: &Repository
) -> Result<Option<String>> {
    let edit_mode_path = repository.git_repository.path().join("gitbutler").join("edit_sha");
    let restore_path = repository.git_repository.path().join("gitbutler").join("restore_sha");

    // delete the file
    std::fs::remove_file(edit_mode_path)?;

    // if restore path exists, read the value and delete the file
    if restore_path.exists() {
      let restore_sha = std::fs::read_to_string(&restore_path)?;
      let restore_sha = restore_sha.trim();
      std::fs::remove_file(restore_path)?;
      return Ok(Some(restore_sha.to_string()));
    }

    Ok(None)
}