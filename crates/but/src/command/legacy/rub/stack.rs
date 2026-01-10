use anyhow::bail;
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use but_ctx::Context;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
};
use colored::Colorize;

use crate::utils::OutputChannel;

/// Stack one branch on top of another by applying it with the correct order parameter.
/// This moves the `source_branch` to be stacked on top of `target_branch`.
pub(crate) fn stack_branch_on_top(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_branch: &str,
    target_branch: &str,
) -> anyhow::Result<()> {
    // Get repository and metadata to find the target position
    let guard = ctx.shared_worktree_access();
    let (repo, _meta, graph) =
        ctx.graph_and_meta_and_repo_from_head(ctx.repo.get()?.clone(), guard.read_permission())?;

    // Convert branch names to full reference names
    let source_ref = repo.find_reference(source_branch)?;
    let target_ref = repo.find_reference(target_branch)?;

    let target_ref_name = target_ref.name();

    // Get the workspace to understand current stack order
    let ws = graph.to_workspace()?;

    // Find the target branch's position in the workspace
    // We want to find where in the stacks list the target branch exists
    let target_position = ws.stacks.iter().position(|stack| {
        // Check if this stack is the target branch itself (top-level stack)
        if let Some(stack_ref_name) = stack.ref_name() {
            if stack_ref_name.as_bstr() == target_ref_name.as_bstr() {
                return true;
            }
        }
        // Check if the target is a segment within this stack
        stack.segments.iter().any(|segment| {
            if let Some(seg_ref_name) = segment.ref_name() {
                seg_ref_name.as_bstr() == target_ref_name.as_bstr()
            } else {
                false
            }
        })
    });

    let target_position = match target_position {
        Some(pos) => pos,
        None => {
            bail!(
                "Target branch {} is not in the current workspace. It must be applied first.",
                target_branch.blue().underline()
            );
        }
    };

    // The order parameter should place the source branch right after the target
    // Since stacks are ordered, and we want to insert after target_position,
    // we use target_position (0-indexed position where to insert)
    let insert_order = target_position;

    let source_ref_full_name = source_ref.name().to_owned();

    drop(ws);
    drop(graph);
    drop(guard);
    drop(source_ref);
    drop(target_ref);
    drop(repo);

    // Now apply the source branch with the order parameter
    let mut guard = ctx.exclusive_worktree_access();
    let (repo, mut meta, graph) =
        ctx.graph_and_meta_mut_and_repo_from_head(guard.write_permission())?;
    let ws = graph.to_workspace()?;

    // Apply the branch with the order parameter to control its position
    let outcome = but_workspace::branch::apply(
        source_ref_full_name.as_ref(),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::default(),
            on_workspace_conflict: OnWorkspaceMergeConflict::default(),
            workspace_reference_naming: WorkspaceReferenceNaming::default(),
            uncommitted_changes: UncommitedWorktreeChanges::default(),
            order: Some(insert_order),
            new_stack_id: None,
        },
    )?
    .into_owned();

    // Report success
    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{} Stacked {} on top of {}",
            "✓".green(),
            source_branch.blue().underline(),
            target_branch.blue().underline()
        )?;

        if outcome.workspace_changed() {
            writeln!(human_out, "  Workspace updated with new stack order")?;
        }
    }

    if let Some(json_out) = out.for_json() {
        let result = serde_json::json!({
            "success": true,
            "source_branch": source_branch,
            "target_branch": target_branch,
            "workspace_changed": outcome.workspace_changed(),
        });
        json_out.write_value(&result)?;
    }

    Ok(())
}
