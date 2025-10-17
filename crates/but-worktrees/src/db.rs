//! Database operations for worktrees.

use std::path::Path;

use anyhow::Result;
use bstr::BString;
use gitbutler_command_context::CommandContext;

use crate::WorktreeMeta;

/// Save a new worktree to the database.
pub fn save_worktree_meta(ctx: &mut CommandContext, worktree: WorktreeMeta) -> Result<()> {
    ctx.db()?.worktrees().insert(worktree.try_into()?)?;
    Ok(())
}

/// Retrieve a worktree by its path.
#[allow(unused)]
pub fn get_worktree_meta(ctx: &mut CommandContext, path: &Path) -> Result<Option<WorktreeMeta>> {
    let path_str = path.to_string_lossy();
    let worktree = ctx.db()?.worktrees().get(&gix::path::into_bstr(path))?;
    match worktree {
        Some(w) => Ok(Some(w.try_into()?)),
        None => Ok(None),
    }
}

/// Delete a worktree from the database.
#[allow(unused)]
pub fn delete_worktree_meta(ctx: &mut CommandContext, path: &Path) -> Result<()> {
    ctx.db()?.worktrees().delete(&gix::path::into_bstr(path))?;
    Ok(())
}

/// List all worktrees in the database.
pub fn list_worktree_meta(ctx: &mut CommandContext) -> Result<Vec<WorktreeMeta>> {
    let worktrees = ctx.db()?.worktrees().list()?;
    worktrees
        .into_iter()
        .map(|w| w.try_into())
        .collect::<Result<_, _>>()
}

impl TryFrom<but_db::Worktree> for WorktreeMeta {
    type Error = anyhow::Error;

    fn try_from(value: but_db::Worktree) -> Result<Self, Self::Error> {
        Ok(WorktreeMeta {
            path: gix::path::from_byte_slice(&value.path).to_owned(),
            created_from_ref: value
                .created_from_ref
                .map(|r| gix::refs::FullName::try_from(BString::from(r)))
                .transpose()?,
            base: gix::ObjectId::from_hex(value.base.as_bytes())?,
        })
    }
}

impl TryFrom<WorktreeMeta> for but_db::Worktree {
    type Error = anyhow::Error;

    fn try_from(value: WorktreeMeta) -> Result<Self, Self::Error> {
        Ok(but_db::Worktree {
            path: gix::path::into_bstr(&value.path).to_vec(),
            created_from_ref: value.created_from_ref.map(|c| c.as_bstr().to_vec()),
            base: value.base.to_hex().to_string(),
        })
    }
}
