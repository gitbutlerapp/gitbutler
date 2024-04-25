use std::path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::errors;
use crate::git::{self, diff};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    #[serde(with = "crate::serde::path")]
    pub path: path::PathBuf,
    pub hunks: Vec<diff::GitHunk>,
    pub binary: bool,
}

pub fn list_remote_commit_files(
    repository: &git::Repository,
    commit_oid: git::Oid,
) -> Result<Vec<RemoteBranchFile>, errors::ListRemoteCommitFilesError> {
    let commit = match repository.find_commit(commit_oid) {
        Ok(commit) => Ok(commit),
        Err(git::Error::NotFound(_)) => Err(errors::ListRemoteCommitFilesError::CommitNotFound(
            commit_oid,
        )),
        Err(error) => Err(errors::ListRemoteCommitFilesError::Other(error.into())),
    }?;

    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }

    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff_files = diff::trees(repository, &parent_tree, &commit_tree)?;

    Ok(diff_files
        .into_iter()
        .map(|(path, file)| {
            let binary = file.hunks.iter().any(|h| h.binary);
            RemoteBranchFile {
                path,
                hunks: file.hunks,
                binary,
            }
        })
        .collect())
}
