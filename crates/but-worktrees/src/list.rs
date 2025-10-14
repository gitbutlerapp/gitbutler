use anyhow::Result;
use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::{StacksFilter, stacks_v3};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeReadPermission;
use serde::{Deserialize, Serialize};

use crate::{Worktree, WorktreeHealthStatus, db::list_worktrees, gc::get_health};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct ListWorktreeOutcome {
    pub entries: Vec<WorktreeListEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct WorktreeListEntry {
    pub status: WorktreeHealthStatus,
    pub worktree: Worktree,
}

/// Creates a new worktree off of a branches given name.
pub fn worktree_list(
    ctx: &mut CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<ListWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
    let heads = stacks.into_iter().flat_map(|s| s.heads).collect::<Vec<_>>();

    let entries = list_worktrees(ctx)?
        .into_iter()
        .map(|w| {
            Ok(WorktreeListEntry {
                status: get_health(&repo, &w, &heads)?,
                worktree: w,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ListWorktreeOutcome { entries })
}
