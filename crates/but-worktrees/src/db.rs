//! File-based metadata storage for worktrees.
//!
//! Stores metadata in `.git/worktrees/<id>/` alongside Git's own worktree metadata:
//! - `gitbutler-created-from`: The git reference this worktree was created from
//! - `gitbutler-base`: The base commit OID for cherry-picking

use std::path::PathBuf;

use anyhow::{Context, Result};
use bstr::ByteSlice;

use crate::{WorktreeId, WorktreeMeta};

const CREATED_FROM_FILE: &str = "gitbutler-created-from";
const BASE_FILE: &str = "gitbutler-base";

/// Get the `.git/worktrees/<id>/` directory for a given worktree ID.
fn worktree_git_dir(repo: &gix::Repository, id: &WorktreeId) -> PathBuf {
    repo.git_dir().join("worktrees").join(id.as_str())
}

/// Save worktree metadata to files in `.git/worktrees/<id>/`.
pub fn save_worktree_meta(repo: &gix::Repository, worktree: WorktreeMeta) -> Result<()> {
    let git_dir = worktree_git_dir(repo, &worktree.id);

    // Ensure the directory exists
    std::fs::create_dir_all(&git_dir).context("Failed to create worktree git directory")?;

    // Write created_from_ref if present
    if let Some(ref created_from) = worktree.created_from_ref {
        std::fs::write(git_dir.join(CREATED_FROM_FILE), created_from.as_bstr())
            .context("Failed to write gitbutler-created-from file")?;
    }

    // Write base commit (as bytes to match read operations)
    std::fs::write(
        git_dir.join(BASE_FILE),
        worktree.base.to_hex().to_string().as_bytes(),
    )
    .context("Failed to write gitbutler-base file")?;

    Ok(())
}

/// Retrieve worktree metadata by its ID.
pub fn get_worktree_meta(repo: &gix::Repository, id: &WorktreeId) -> Result<Option<WorktreeMeta>> {
    let git_dir = worktree_git_dir(repo, id);

    // Check if metadata files exist
    let base_file = git_dir.join(BASE_FILE);
    if !base_file.exists() {
        return Ok(None);
    }

    // Read base commit
    let base_bytes = std::fs::read(&base_file).context("Failed to read gitbutler-base file")?;
    let base = gix::ObjectId::from_hex(base_bytes.trim()).context("Invalid base commit OID")?;

    // Read created_from_ref if present
    let created_from_file = git_dir.join(CREATED_FROM_FILE);
    let created_from_ref = if created_from_file.exists() {
        let ref_bytes = std::fs::read(&created_from_file)
            .context("Failed to read gitbutler-created-from file")?;
        let ref_bstr = bstr::BString::from(ref_bytes.trim());
        Some(gix::refs::FullName::try_from(ref_bstr)?)
    } else {
        None
    };

    Ok(Some(WorktreeMeta {
        id: id.clone(),
        created_from_ref,
        base,
    }))
}

/// List all worktrees with GitButler metadata.
pub fn list_worktree_meta(repo: &gix::Repository) -> Result<Vec<WorktreeMeta>> {
    let mut result = Vec::new();

    // Use gix to discover all worktrees, then check if we have metadata for each
    for worktree in repo.worktrees()? {
        let id = WorktreeId::from_bstr(worktree.id());
        if let Some(meta) = get_worktree_meta(repo, &id)? {
            result.push(meta);
        }
    }

    Ok(result)
}
