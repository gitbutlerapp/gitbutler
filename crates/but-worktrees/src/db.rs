//! File-based metadata storage for worktrees.
//!
//! Stores metadata in `.git/worktrees/<name>/` alongside Git's own worktree metadata:
//! - `gitbutler-created-from`: The git reference this worktree was created from
//! - `gitbutler-base`: The base commit OID for cherry-picking

use anyhow::{Context, Result};
use bstr::ByteSlice;
use gitbutler_command_context::CommandContext;
use std::path::{Path, PathBuf};

use crate::WorktreeMeta;

const CREATED_FROM_FILE: &str = "gitbutler-created-from";
const BASE_FILE: &str = "gitbutler-base";

/// Get the `.git/worktrees/<name>/` directory for a given worktree path.
///
/// The worktree path is typically `.git/gitbutler/worktrees/{uuid}`, and we extract
/// the basename (UUID) to find the corresponding `.git/worktrees/{uuid}/` directory.
fn worktree_git_dir(project_path: &Path, worktree_path: &Path) -> Result<PathBuf> {
    let basename = worktree_path
        .file_name()
        .context("Worktree path has no filename")?;

    Ok(project_path.join(".git").join("worktrees").join(basename))
}

/// Save worktree metadata to files in `.git/worktrees/<name>/`.
pub fn save_worktree_meta(ctx: &mut CommandContext, worktree: WorktreeMeta) -> Result<()> {
    let git_dir = worktree_git_dir(&ctx.project().path, &worktree.path)?;

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

/// Retrieve worktree metadata by its path.
pub fn get_worktree_meta(ctx: &mut CommandContext, path: &Path) -> Result<Option<WorktreeMeta>> {
    let git_dir = worktree_git_dir(&ctx.project().path, path)?;

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
        path: path.to_owned(),
        created_from_ref,
        base,
    }))
}

/// List all worktrees with GitButler metadata.
pub fn list_worktree_meta(ctx: &mut CommandContext) -> Result<Vec<WorktreeMeta>> {
    let repo = ctx.gix_repo_for_merging()?;

    let mut result = Vec::new();

    // Use gix to discover all worktrees, then check if we have metadata for each
    for worktree in repo.worktrees()? {
        let path = match worktree.base() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Try to read our metadata for this worktree
        if let Some(meta) = get_worktree_meta(ctx, &path)? {
            result.push(meta);
        }
    }

    Ok(result)
}
