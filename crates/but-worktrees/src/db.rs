//! Database operations for worktrees.

use anyhow::Result;
use gitbutler_command_context::CommandContext;
use std::path::Path;

use crate::{Worktree, WorktreeSource};

/// Save a new worktree to the database.
pub fn save_worktree(ctx: &mut CommandContext, worktree: Worktree) -> Result<()> {
    ctx.db()?.worktrees().insert(worktree.try_into()?)?;
    Ok(())
}

/// Retrieve a worktree by its path.
pub fn get_worktree(ctx: &mut CommandContext, path: &Path) -> Result<Option<Worktree>> {
    let path_str = path.to_string_lossy();
    let worktree = ctx.db()?.worktrees().get(&path_str)?;
    match worktree {
        Some(w) => Ok(Some(w.try_into()?)),
        None => Ok(None),
    }
}

/// Delete a worktree from the database.
pub fn delete_worktree(ctx: &mut CommandContext, path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();
    ctx.db()?.worktrees().delete(&path_str)?;
    Ok(())
}

/// List all worktrees in the database.
pub fn list_worktrees(ctx: &mut CommandContext) -> Result<Vec<Worktree>> {
    let worktrees = ctx.db()?.worktrees().list()?;
    worktrees
        .into_iter()
        .map(|w| w.try_into())
        .collect::<Result<_, _>>()
}

impl TryFrom<but_db::Worktree> for Worktree {
    type Error = anyhow::Error;

    fn try_from(value: but_db::Worktree) -> Result<Self, Self::Error> {
        let source: WorktreeSource = serde_json::from_str(&value.source)?;
        let base = gix::ObjectId::from_hex(value.base.as_bytes())?;
        let path = std::path::PathBuf::from(value.path);

        Ok(Worktree {
            path,
            reference: value.reference,
            base,
            source,
        })
    }
}

impl TryFrom<Worktree> for but_db::Worktree {
    type Error = anyhow::Error;

    fn try_from(value: Worktree) -> Result<Self, Self::Error> {
        let source = serde_json::to_string(&value.source)?;
        let base = value.base.to_hex().to_string();
        let path = value.path.to_string_lossy().to_string();

        Ok(but_db::Worktree {
            path,
            reference: value.reference,
            base,
            source,
        })
    }
}
