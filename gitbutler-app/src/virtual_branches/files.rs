use std::path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::git::{self, diff, show};

use super::errors;
use crate::virtual_branches::context;

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    pub path: path::PathBuf,
    pub hunks: Vec<diff::Hunk>,
    pub binary: bool,
}

pub fn list_remote_commit_files(
    repository: &git::Repository,
    commit_oid: git::Oid,
    context_lines: u32,
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
    let diff = diff::trees(repository, &parent_tree, &commit_tree, context_lines)?;

    let files = diff
        .into_iter()
        .map(|(file_path, hunks)| RemoteBranchFile {
            path: file_path.clone(),
            hunks: hunks.clone(),
            binary: hunks.iter().any(|h| h.binary),
        })
        .collect::<Vec<_>>();

    let files = files_with_hunk_context(repository, &parent_tree, files, 3)
        .context("failed to add context to hunk")?;
    Ok(files)
}

fn files_with_hunk_context(
    repository: &git::Repository,
    parent_tree: &git::Tree,
    mut files: Vec<RemoteBranchFile>,
    context_lines: usize,
) -> Result<Vec<RemoteBranchFile>> {
    for file in &mut files {
        if file.binary {
            continue;
        }
        // Get file content as it looked before the diffs
        let file_content_before =
            show::show_file_at_tree(repository, file.path.clone(), parent_tree)
                .context("failed to get file contents at HEAD")?;
        let file_lines_before = file_content_before.split('\n').collect::<Vec<_>>();

        file.hunks = file
            .hunks
            .iter()
            .map(|hunk| {
                if hunk.diff.is_empty() {
                    // noop on empty diff
                    hunk.clone()
                } else {
                    context::hunk_with_context(
                        &hunk.diff,
                        hunk.old_start as usize,
                        hunk.new_start as usize,
                        hunk.binary,
                        context_lines,
                        &file_lines_before,
                        hunk.change_type,
                    )
                }
            })
            .collect::<Vec<diff::Hunk>>();
    }
    Ok(files)
}
