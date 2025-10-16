use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeReadPermission;
use serde::Serialize;

use crate::{Worktree, db::list_worktree_meta};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct ListWorktreeOutcome {
    pub entries: Vec<Worktree>,
}

/// Lists worktrees
pub fn worktree_list(
    ctx: &mut CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<ListWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;

    let metas = list_worktree_meta(ctx)?;

    let entries = repo
        .worktrees()?
        .into_iter()
        .filter_map(|w| {
            let path = w.base().ok()?;
            let meta = metas.iter().find(|meta| meta.path == path);

            Some(Worktree {
                path,
                created_from_ref: meta.and_then(|m| m.created_from_ref.clone()),
                base: meta.map(|m| m.base),
            })
        })
        .collect();

    Ok(ListWorktreeOutcome { entries })
}
