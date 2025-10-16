use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeReadPermission;
use serde::Serialize;

use crate::{Worktree, WorktreeHealthStatus, db::list_worktrees, gc::get_health};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct ListWorktreeOutcome {
    pub entries: Vec<WorktreeListEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct WorktreeListEntry {
    pub status: WorktreeHealthStatus,
    pub worktree: Worktree,
}

/// Creates a new worktree off of a branches given name.
pub fn worktree_list(
    ctx: &mut CommandContext,
    perm: &WorktreeReadPermission,
) -> Result<ListWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;
    let (repo, _, graph) = ctx.graph_and_meta(repo, perm)?;
    let ws = graph.to_workspace()?;
    let ws_segment_names = ws
        .stacks
        .into_iter()
        .flat_map(|s| s.segments)
        .filter_map(|s| s.ref_name)
        .collect::<Vec<_>>();

    let entries = list_worktrees(ctx)?
        .into_iter()
        .map(|w| {
            Ok(WorktreeListEntry {
                status: get_health(&repo, &w, &ws_segment_names)?,
                worktree: w,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ListWorktreeOutcome { entries })
}
