use anyhow::Result;
use gitbutler_stack::VirtualBranchesHandle;
use tempfile::TempDir;

use but_ctx::Context;

use crate::driverless;

fn command_ctx(name: &str) -> Result<(Context, TempDir)> {
    driverless::writable_context("workspace-commit.sh", name)
}

/// When two applied stacks have trees that conflict on the same file,
/// `remerged_workspace_tree_v2` (called by `update_workspace_commit`) detects the
/// gix merge conflict and marks the later stack as `in_workspace = false`.
/// With the fix in `remerged_workspace_commit_v2`, that evicted stack's head must
/// be excluded from the workspace commit's parent list.
///
/// Without the fix, the workspace commit tree would not contain the evicted stack's
/// changes but its head would still be a parent — causing phantom uncommitted changes
/// when diffing the workspace commit against its parents.
#[test]
fn conflicting_stacks_evicted_from_workspace_commit_parents() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("conflicting-stacks")?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks_before = vb_state.list_stacks_in_workspace()?;
    assert_eq!(
        stacks_before.len(),
        2,
        "precondition: 2 stacks in workspace"
    );

    // Rebuild the workspace commit through the legacy path.
    // remerged_workspace_tree_v2 iterates both stacks and merges each tree:
    //   - The first stack merges cleanly onto the target tree
    //   - The second stack conflicts (same file, different content) → in_workspace = false
    // remerged_workspace_commit_v2 (with our fix) then excludes the evicted stack
    // from the workspace commit's parent list.
    gitbutler_branch_actions::update_workspace_commit(&ctx, false)?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    // Exactly one of the two conflicting stacks should have been evicted.
    let stacks_after = vb_state.list_stacks_in_workspace()?;
    assert_eq!(
        stacks_after.len(),
        1,
        "Only the non-conflicting stack should remain in workspace"
    );
    let surviving_stack = &stacks_after[0];

    // The workspace commit must have exactly 1 parent: the surviving stack's head.
    let repo = ctx.repo.get()?;
    let ws_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
    let ws_commit = ws_ref.into_fully_peeled_id()?.object()?.try_into_commit()?;
    let parent_ids: Vec<_> = ws_commit.parent_ids().collect();

    assert_eq!(
        parent_ids.len(),
        1,
        "Workspace commit should have only the surviving stack as parent"
    );

    let surviving_head = surviving_stack.head_oid(&ctx)?;
    assert_eq!(
        parent_ids[0].detach(),
        surviving_head,
        "The only parent should be the surviving stack's head"
    );

    Ok(())
}
