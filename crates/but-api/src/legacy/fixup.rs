use gitbutler_command_context::CommandContext;
use std::collections::{BTreeMap, BTreeSet};
use tracing::instrument;

/// Validate and fix workspace stack `in_workspace` status of `virtual_branches.toml`
/// so they match what's actually in the workspace.
/// If there is a change, the data is written back.
///
/// Errors are silently ignored to allow the application to continue loading even if
/// the migration fails - the workspace will still be functional, just potentially
/// with stale metadata that can confuse 'old' code.
///
/// NOTE: This isn't needed for new code - it won't base any decisions on the metadata alone.
#[instrument(level = tracing::Level::DEBUG, skip(ctx))]
pub fn reconcile_in_workspace_state_of_vb_toml(ctx: &mut CommandContext) -> Option<()> {
    fn make_heads_match(
        ws_stack: &but_graph::projection::Stack,
        vb_stack: &mut but_meta::virtual_branches_legacy_types::Stack,
    ) -> bool {
        // Always leave extra segments.

        // Add missing segments
        let segments_to_add: Vec<_> = ws_stack
            .segments
            .iter()
            .filter_map(|s| {
                s.ref_name().and_then(|rn| {
                    let is_in_vb_stack_branches =
                        vb_stack.heads.iter().any(|sb| sb.name == rn.shorten());
                    (!is_in_vb_stack_branches).then_some((s, rn))
                })
            })
            .collect();

        for (segment, segment_name) in segments_to_add {
            vb_stack
                .heads
                .push(but_meta::virtual_branches_legacy_types::StackBranch {
                    head: but_meta::virtual_branches_legacy_types::CommitOrChangeId::CommitId(
                        segment
                            .commits
                            .first()
                            .map_or(gix::hash::Kind::Sha1.null(), |c| c.id)
                            .to_string(),
                    ),
                    name: segment_name.shorten().to_string(),
                    description: None,
                    pr_number: None,
                    archived: false,
                    review_id: None,
                });
        }

        // finally, put them in order, for good measure.
        let previous_heads = vb_stack.heads.clone();
        let original_positions_by_name: BTreeMap<_, _> = vb_stack
            .heads
            .iter()
            // Use our order
            .rev()
            .enumerate()
            .map(|(idx, s)| (s.name.clone(), idx))
            .collect();
        vb_stack.heads.sort_by_key(|sb| {
            ws_stack
                .segments
                .iter()
                .position(|s| s.ref_name().is_some_and(|rn| rn.shorten() == sb.name))
                .or_else(|| original_positions_by_name.get(&sb.name).copied())
        });
        // The ws_stack order is top to bottom, the other is bottom to top.
        vb_stack.heads.reverse();
        vb_stack.heads != previous_heads
    }
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();
    let (_repo, mut meta, graph) = ctx
        .graph_and_meta_mut_and_repo_from_reference(
            "refs/heads/gitbutler/workspace"
                .try_into()
                .expect("statically known to be valid"),
            perm,
        )
        .ok()?;
    let ws = graph.to_workspace().ok()?;

    let mut seen = BTreeSet::new();
    for (ws_stack, in_workspace_stack_id) in ws.stacks.iter().filter_map(|s| s.id.map(|id| (s, id)))
    {
        seen.insert(in_workspace_stack_id);
        let Some(vb_stack) = meta.data_mut().branches.get_mut(&in_workspace_stack_id) else {
            tracing::warn!(
                "Didn't find stack with id {in_workspace_stack_id} in branches metadata, and it would have to be created or old code may fail"
            );
            continue;
        };

        let made_heads_match = make_heads_match(ws_stack, vb_stack);
        if !vb_stack.in_workspace {
            tracing::warn!(
                "Fixing stale metadata of stack {in_workspace_stack_id} to be considered inside the workspace",
            );
            vb_stack.in_workspace = true;
            meta.set_changed_to_necessitate_write();
        }
        if made_heads_match {
            tracing::warn!(
                "Adjusted segments in stack {in_workspace_stack_id} to match what's actually there"
            );
            meta.set_changed_to_necessitate_write();
        }
    }

    let stack_ids_to_put_in_workspace: Vec<_> = meta
        .data()
        .branches
        .keys()
        .filter(|stack_id| !seen.contains(stack_id))
        .copied()
        .collect();
    for stack_id_not_in_workspace in stack_ids_to_put_in_workspace {
        let vb_stack = meta
            .data_mut()
            .branches
            .get_mut(&stack_id_not_in_workspace)
            .expect("BUG: we just traversed this stack-id");
        if vb_stack.in_workspace {
            tracing::warn!(
                "Fixing stale metadata of stack {stack_id_not_in_workspace} to be considered outside the workspace",
            );
            vb_stack.in_workspace = false;
            meta.set_changed_to_necessitate_write();
        }
    }
    None
}
