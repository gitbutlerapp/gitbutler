use anyhow::Result;
use but_ctx::{Context, access::WorktreeReadPermission};
use serde::Serialize;

use crate::{Worktree, WorktreeId, db::list_worktree_meta};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct ListWorktreeOutcome {
    pub entries: Vec<Worktree>,
}

/// Lists worktrees
pub fn worktree_list(
    ctx: &mut Context,
    _perm: &WorktreeReadPermission,
) -> Result<ListWorktreeOutcome> {
    let repo = ctx.open_repo_for_merging()?;

    let metas = list_worktree_meta(&repo)?;

    let entries = repo
        .worktrees()?
        .into_iter()
        .filter_map(|w| {
            let path = w.base().ok()?;

            // Extract ID from path to find matching metadata
            let id = WorktreeId::from_path(&path).ok()?;
            let meta = metas.iter().find(|meta| meta.id == id);

            Some(Worktree {
                id,
                path,
                created_from_ref: meta.and_then(|m| m.created_from_ref.clone()),
                base: meta.map(|m| m.base),
            })
        })
        .collect();

    Ok(ListWorktreeOutcome { entries })
}
