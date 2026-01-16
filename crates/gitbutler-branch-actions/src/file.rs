use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use but_ctx::Context;
use gitbutler_cherry_pick::RepositoryExt as _;
use gitbutler_diff::FileDiff;
use serde::Serialize;

use crate::hunk::{VirtualBranchHunk, file_hunks_from_diffs};

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

pub(crate) fn list_virtual_commit_files(
    ctx: &Context,
    commit: &git2::Commit,
    context_lines: bool,
) -> Result<Vec<VirtualBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let repo = &*ctx.git2_repo.get()?;
    let commit_tree = repo
        .find_real_tree(commit, Default::default())
        .context("failed to get commit tree")?;
    let parent_tree = repo
        .find_real_tree(&parent, Default::default())
        .context("failed to get parent tree")?;
    let diff = gitbutler_diff::trees(
        &*ctx.git2_repo.get()?,
        &parent_tree,
        &commit_tree,
        context_lines,
    )?;
    let hunks_by_filepath = virtual_hunks_by_file_diffs(ctx.legacy_project.worktree_dir()?, diff);
    Ok(virtual_hunks_into_virtual_files(hunks_by_filepath))
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
    hunks: impl IntoIterator<Item = (PathBuf, Vec<VirtualBranchHunk>)>,
) -> Vec<VirtualBranchFile> {
    hunks
        .into_iter()
        .map(|(path, hunks)| {
            let id = path.display().to_string();
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
                conflicted: false, // TODO: Get this from the index
            }
        })
        .collect::<Vec<_>>()
}
