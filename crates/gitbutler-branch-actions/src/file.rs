use std::{
    collections::HashMap,
    path::{self, Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_diff::FileDiff;
use serde::Serialize;

use crate::{
    conflicts,
    hunk::{file_hunks_from_diffs, VirtualBranchHunk},
};

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchFile {
    pub path: path::PathBuf,
    pub hunks: Vec<gitbutler_diff::GitHunk>,
    pub binary: bool,
}

pub(crate) fn list_remote_commit_files(
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
    let diff_files = gitbutler_diff::trees(repository, &parent_tree, &commit_tree)?;

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

// this struct is a mapping to the view `File` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of the
// `Branch.ownership` vector in Rust. an array of them are returned as part of
// the `VirtualBranch` struct, which map to each entry of the `Branch.ownership` vector
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchFile {
    // TODO(ST): `id` is just `path` as string - UI could adapt and avoid this copy.
    pub id: String,
    pub path: PathBuf,
    pub hunks: Vec<VirtualBranchHunk>,
    pub modified_at: u128,
    pub conflicted: bool,
    pub binary: bool,
    pub large: bool,
}

pub trait Get<T> {
    fn get(&self, path: &Path) -> Option<T>;
}
impl Get<VirtualBranchFile> for Vec<VirtualBranchFile> {
    fn get(&self, path: &Path) -> Option<VirtualBranchFile> {
        self.iter().find(|f| f.path == path).cloned()
    }
}

pub(crate) fn list_virtual_commit_files(
    ctx: &CommandContext,
    commit: &git2::Commit,
) -> Result<Vec<VirtualBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff = gitbutler_diff::trees(ctx.repository(), &parent_tree, &commit_tree)?;
    let hunks_by_filepath = virtual_hunks_by_file_diffs(&ctx.project().path, diff);
    Ok(virtual_hunks_into_virtual_files(ctx, hunks_by_filepath))
}

fn virtual_hunks_by_file_diffs<'a>(
    project_path: &'a Path,
    diff: impl IntoIterator<Item = (PathBuf, FileDiff)> + 'a,
) -> HashMap<PathBuf, Vec<VirtualBranchHunk>> {
    file_hunks_from_diffs(
        project_path,
        diff.into_iter()
            .map(move |(file_path, file)| (file_path, file.hunks)),
        None,
    )
}

/// NOTE: There is no use returning an iterator here as this acts like the final product.
pub(crate) fn virtual_hunks_into_virtual_files(
    ctx: &CommandContext,
    hunks: impl IntoIterator<Item = (PathBuf, Vec<VirtualBranchHunk>)>,
) -> Vec<VirtualBranchFile> {
    hunks
        .into_iter()
        .map(|(path, hunks)| {
            let id = path.display().to_string();
            let conflicted = conflicts::is_conflicting(ctx, Some(&path)).unwrap_or(false);
            let binary = hunks.iter().any(|h| h.binary);
            let modified_at = hunks.iter().map(|h| h.modified_at).max().unwrap_or(0);
            debug_assert!(hunks.iter().all(|hunk| hunk.file_path == path));
            VirtualBranchFile {
                id,
                path,
                hunks,
                binary,
                large: false,
                modified_at,
                conflicted,
            }
        })
        .collect::<Vec<_>>()
}
