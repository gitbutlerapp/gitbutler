use std::path;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::git::diff;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    pub path: path::PathBuf,
    pub hunks: Vec<diff::GitHunk>,
    pub binary: bool,
}

pub fn list_remote_commit_files(
    repository: &git2::Repository,
    commit_id: git2::Oid,
) -> Result<Vec<RemoteBranchFile>> {
    let commit = repository
        .find_commit(commit_id)
        .map_err(|err| match err.code() {
            git2::ErrorCode::NotFound => anyhow!("commit {commit_id} not found"),
            _ => err.into(),
        })?;

    // If it's a merge commit, we list nothing. In the future we could to a fork exec of `git diff-tree --cc`
    if commit.parent_count() != 1 {
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
