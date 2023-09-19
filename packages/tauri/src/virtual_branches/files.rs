use std::path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::git::{self, diff};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    pub path: path::PathBuf,
    pub hunks: Vec<diff::Hunk>,
    pub binary: bool,
}

pub fn list_remote_commit_files(
    repository: &git::Repository,
    commit: &git::Commit,
) -> Result<Vec<RemoteBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff = diff::trees(repository, &parent_tree, &commit_tree)?;

    let files = diff
        .into_iter()
        .map(|(file_path, hunks)| RemoteBranchFile {
            path: file_path.clone(),
            hunks: hunks.clone(),
            binary: hunks.iter().any(|h| h.binary),
        })
        .collect::<Vec<_>>();

    Ok(files)
}
