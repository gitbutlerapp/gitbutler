use std::path;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::git::{self, diff, CommitExt};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    pub path: path::PathBuf,
    pub hunks: Vec<diff::GitHunk>,
    pub binary: bool,
}

pub fn list_remote_commit_files(
    repository: &git::Repository,
    commit_id: git::Oid,
) -> Result<Vec<RemoteBranchFile>> {
    let commit = repository.find_commit(commit_id).map_err(|err| match err {
        git::Error::NotFound(_) => anyhow!("commit {commit_id} not found"),
        err => err.into(),
    })?;

    // If it's a merge commit, we list nothing. In the future we could to a fork exec of `git diff-tree --cc`
    if commit.parent_count() != 1 {
        return Ok(vec![]);
    }

    let commit_tree = commit.tree().context("failed to get commit tree")?;

    // if we have a conflicted commit, we just need a vector of the files that conflicted
    if commit.is_conflicted() {
        let conflict_files_list = commit_tree.get_name(".conflict-files").unwrap();
        let files_list = conflict_files_list.to_object(repository.into()).unwrap();
        let list = files_list
            .as_blob()
            .context("failed to get conflict files list")?;
        // split this list blob into lines and return a Vec of RemoteBranchFile
        let files = list
            .content()
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                let path = path::PathBuf::from(std::str::from_utf8(line).unwrap());
                RemoteBranchFile {
                    path,
                    hunks: vec![],
                    binary: false,
                }
            })
            .collect();
        return Ok(files);
    }

    let parent = commit.parent(0).context("failed to get parent commit")?;
    let mut parent_tree = parent.tree().context("failed to get parent tree")?;

    if parent.is_conflicted() {
        parent_tree = repository.find_real_tree(&parent, None).unwrap();
    }

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
