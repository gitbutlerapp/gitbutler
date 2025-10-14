//! Database operations for worktrees.

use anyhow::Result;
use bstr::BString;
use gitbutler_command_context::CommandContext;
use std::path::Path;

use crate::{Worktree, WorktreeSource};

/// Save a new worktree to the database.
pub fn save_worktree(ctx: &mut CommandContext, worktree: Worktree) -> Result<()> {
    ctx.db()?.worktrees().insert(worktree.try_into()?)?;
    Ok(())
}

/// Retrieve a worktree by its path.
#[allow(unused)]
pub fn get_worktree(ctx: &mut CommandContext, path: &Path) -> Result<Option<Worktree>> {
    let path_str = path.to_string_lossy();
    let worktree = ctx.db()?.worktrees().get(&gix::path::into_bstr(path))?;
    match worktree {
        Some(w) => Ok(Some(w.try_into()?)),
        None => Ok(None),
    }
}

/// Delete a worktree from the database.
#[allow(unused)]
pub fn delete_worktree(ctx: &mut CommandContext, path: &Path) -> Result<()> {
    ctx.db()?.worktrees().delete(&gix::path::into_bstr(path))?;
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
        let path = gix::path::from_byte_slice(&value.path).to_owned();

        Ok(Worktree {
            path,
            reference: gix::refs::FullName::try_from(BString::from(value.reference))?,
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

        Ok(but_db::Worktree {
            path: gix::path::into_bstr(&value.path).to_vec(),
            reference: value.reference.as_bstr().to_vec(),
            base,
            source,
        })
    }
}
