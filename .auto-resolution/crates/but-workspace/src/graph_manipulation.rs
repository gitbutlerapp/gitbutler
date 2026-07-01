//! Shared graph editor traversal and edge-manipulation helpers.

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use but_graph::workspace::{Stack, StackSegment};
use but_rebase::graph_rebase::{
    Editor, LookupStep, Selector, Step, ToSelector,
    mutate::{SegmentDelimiter, SelectorSet, SomeSelectors},
};
use std::collections::HashSet;

/// Payload containing information about how to disconnect a segment in the graph.
pub struct DisconnectParameters {
    /// The bounds of the segment to disconnect.
    pub(crate) delimiter: SegmentDelimiter<Selector, Selector>,
    /// The children of the child-most segment bound to disconnect.
    pub(crate) children_to_disconnect: SelectorSet,
    /// The parents of the parent-most segment bound to disconnect.
    pub(crate) parents_to_disconnect: SelectorSet,
}

/// Get the right disconnect parameters for the given subject segment and source stack.
///
/// This function determines which are the right parents and children to disconnect,
/// as well as the right segment delimiter to move.
pub fn get_disconnect_parameters<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    source_stack: &Stack,
    subject_segment: &StackSegment,
    workspace_head: gix::ObjectId,
) -> anyhow::Result<DisconnectParameters> {
    let index_of_segment = source_stack
        .segments
        .iter()
        .position(|segment| segment.id == subject_segment.id)
        .context("BUG: Unable to find subject segment on source stack.")?;

    let subject_segment_ref_name = subject_segment
        .ref_name()
        .context("Subject segment doesn't have a ref name.")?;
    let delimiter_child = editor
        .select_reference(subject_segment_ref_name)
        .context("Failed to find subject reference in graph.")?;
    let delimiter_parent = match subject_segment.commits.last() {
        Some(last_commit) => editor
            .select_commit(last_commit.id)
            .context("Failed to find last commit in subject segment in graph.")?,
        None => {
            // Subject segment is empty, move only the reference
            delimiter_child
        }
    };

    // The delimiter for the segment we want to move, is the reference selector
    // as the child, and the last commit inside the branch as the parent.
    // If the branch is empty, we take the reference selector as the parent as well.
    let delimiter = SegmentDelimiter {
        child: delimiter_child,
        parent: delimiter_parent,
    };

    // Disconnect the subject from the base directly below its parent-delimiter — the branch's last
    // commit, or its reference when the branch is empty. The base is the first-parent edge (lowest
    // edge order); if the bottom commit is a merge, its higher-order parents must travel with the
    // subject rather than be cut. We read this from the rebase editor graph (which
    // `disconnect_segment_from` validates against) rather than the workspace projection: when the
    // target is ahead of the merge base the projection's base segment is anonymous and resolves to
    // the base commit, while the editor graph keeps the target reference node between the branch and
    // that commit, so only the editor-graph first parent matches the edge being checked.
    let parents_to_disconnect = match editor
        .direct_parents(delimiter.parent)?
        .into_iter()
        .min_by_key(|(_, order)| *order)
    {
        Some((base, _)) => SelectorSet::Some(SomeSelectors::new(vec![base])?),
        None => SelectorSet::All,
    };

    if index_of_segment == 0 {
        // This is the top-most segment in the stack, so the parent is the workspace commit.
        let workspace_head_selector = editor
            .select_commit(workspace_head)
            .context("Failed to find workspace head in graph.")?;
        let selectors = SomeSelectors::new(vec![workspace_head_selector])?;
        let children_to_disconnect = SelectorSet::Some(selectors);

        return Ok(DisconnectParameters {
            delimiter,
            children_to_disconnect,
            parents_to_disconnect,
        });
    }

    // Segment on top of the subject segment in the stack.
    let child_segment = source_stack.segments.get(index_of_segment - 1).context(
        "BUG: Unable to find child segment of subject segment but expected it to exist.",
    )?;

    // If branch stacked on top of the branch we want to move is empty, we only need to disconnect
    // the reference from it.
    // Otherwise, disconnect the last commit on the segment.
    let child_selector = match child_segment.commits.last() {
        Some(last_commit) => editor
            .select_commit(last_commit.id)
            .context("Failed to find last commit of child segment in graph."),
        None => {
            // The segment on top of the subject segment is empty. Select the reference.
            let child_segment_ref_name = child_segment
                .ref_name()
                .context("Child segment doesn't have a ref name.")?;
            editor
                .select_reference(child_segment_ref_name)
                .context("Failed to find child segment reference in graph.")
        }
    }?;
    let selectors = SomeSelectors::new(vec![child_selector])?;
    let children_to_disconnect = SelectorSet::Some(selectors);

    Ok(DisconnectParameters {
        delimiter,
        children_to_disconnect,
        parents_to_disconnect,
    })
}

/// Determine which parent to disconnect from the subject commit.
///
/// Preference rules:
/// - Prefer a `Pick` parent first, which aligns with linear first-parent ancestry.
/// - If no commit parent edge is found, fall back to a `Reference` parent.
///
/// If no explicit parent candidate exists, return `SelectorSet::All` as a safe fallback.
pub fn determine_parent_selector<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    subject_commit_selector: Selector,
) -> anyhow::Result<SelectorSet> {
    let mut parents = editor.direct_parents(subject_commit_selector)?;
    parents.sort_by_key(|(_, order)| *order);

    let preferred = parents
        .iter()
        .find(|(selector, _)| matches!(editor.lookup_step(*selector), Ok(Step::Pick(_))))
        .or_else(|| {
            parents.iter().find(|(selector, _)| {
                matches!(editor.lookup_step(*selector), Ok(Step::Reference { .. }))
            })
        })
        .map(|(selector, _)| *selector);

    match preferred {
        Some(selector) => {
            let selectors = SomeSelectors::new(vec![selector])?;
            Ok(SelectorSet::Some(selectors))
        }
        None => Ok(SelectorSet::All),
    }
}

/// Which direct edge set to resolve from a selector.
#[derive(Clone, Copy)]
pub(crate) enum EdgeSelection {
    /// Resolve direct child edges.
    Children,
    /// Resolve direct parent edges.
    Parents,
}

/// Disconnect all parent edges from a single selector without reconnecting them.
///
/// `editor` is the mutable graph editor whose connectivity will be updated.
///
/// `selector` is the node whose parent edges should be removed.
///
/// Returns `Ok(())` after all direct parent edges of `selector` have been
/// removed from the editor graph.
pub(crate) fn disconnect_selector_from_all_parents<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    selector: Selector,
) -> Result<()> {
    editor.disconnect_segment_from(
        SegmentDelimiter {
            child: selector,
            parent: selector,
        },
        SelectorSet::None,
        SelectorSet::All,
        true,
    )?;

    Ok(())
}

/// Resolve concrete direct edges selected by a selector set, preserving edge order.
///
/// `editor` provides the direct parent or child edges that can be selected.
///
/// `target` is the node whose adjacent edges should be filtered.
///
/// `selectors` describes which neighboring selectors to keep, or whether to
/// keep all or none of them.
///
/// `edge_selection` chooses whether neighbors are read from direct children or
/// direct parents of `target`.
///
/// Returns the selected neighboring selectors paired with their existing edge
/// order values.
pub(crate) fn selected_edges_from_set<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    target: Selector,
    selectors: &SelectorSet,
    edge_selection: EdgeSelection,
) -> Result<Vec<(Selector, usize)>> {
    let available = match edge_selection {
        EdgeSelection::Children => editor.direct_children(target)?,
        EdgeSelection::Parents => editor.direct_parents(target)?,
    };

    match selectors {
        SelectorSet::All => Ok(available),
        SelectorSet::None => Ok(Vec::new()),
        SelectorSet::Some(some_selectors) => {
            let mut selected = Vec::new();
            for selector in some_selectors.as_slice() {
                let selector = selector.to_selector(editor)?;
                let Some((_, order)) = available
                    .iter()
                    .find(|(candidate, _)| *candidate == selector)
                else {
                    bail!("Selected edge endpoint wasn't found among direct neighbors")
                };
                selected.push((selector, *order));
            }
            Ok(selected)
        }
    }
}

/// Reconnect a rebuilt segment to previously selected children and parents.
///
/// `editor` is the mutable graph editor whose edges will be recreated.
///
/// `delimiter` identifies the rebuilt segment's child-most and parent-most
/// selectors.
///
/// `children` are the previously captured child edges that should point back to
/// `delimiter.child`. If the child is already connected to `delimiter.child`, no
/// new edge is added. Otherwise, the original edge order is reused when
/// available, or the next free order is used when another parent already
/// occupies it.
///
/// `parents` are the previously captured parent edges that should be restored
/// from `delimiter.parent`, with fresh order values appended after any existing
/// parents already connected there. If a parent is already connected to
/// `delimiter.parent`, no new edge is added. Otherwise, the appended order is
/// reused when available, or advanced to the next free order on collision.
///
/// Returns `Ok(())` after the captured child and parent edges have been
/// reattached to the rebuilt segment.
pub(crate) fn connect_segment_to_edges<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    delimiter: SegmentDelimiter<Selector, Selector>,
    children: &[(Selector, usize)],
    parents: &[(Selector, usize)],
) -> Result<()> {
    for (child, order) in children {
        let direct_parents = editor.direct_parents(*child)?;
        if direct_parents
            .iter()
            .any(|(parent, _)| *parent == delimiter.child)
        {
            continue;
        }
        editor.add_edge(
            *child,
            delimiter.child,
            next_available_order(direct_parents.iter().map(|(_, order)| *order), *order),
        )?;
    }

    let parent_order_offset = editor
        .direct_parents(delimiter.parent)?
        .into_iter()
        .map(|(_, order)| order)
        .max()
        .map(|max| max + 1)
        .unwrap_or(0);

    for (parent, order) in parents {
        let direct_parents = editor.direct_parents(delimiter.parent)?;
        if direct_parents
            .iter()
            .any(|(existing_parent, _)| *existing_parent == *parent)
        {
            continue;
        }
        let desired_order = parent_order_offset + *order;
        editor.add_edge(
            delimiter.parent,
            *parent,
            next_available_order(
                direct_parents
                    .iter()
                    .map(|(_, existing_order)| *existing_order),
                desired_order,
            ),
        )?;
    }

    Ok(())
}

fn next_available_order(
    existing_orders: impl Iterator<Item = usize>,
    desired_order: usize,
) -> usize {
    let used_orders = existing_orders.collect::<HashSet<_>>();
    let mut order = desired_order;
    while used_orders.contains(&order) {
        order += 1;
    }
    order
}

/// Return a direct parent of `child` when `step` refers to a pick that is already connected.
///
/// This is useful when rebuilding an editor segment and we want to reuse an existing
/// pick node without adding a duplicate edge to the same commit.
///
/// `editor` provides access to the current parent edges and commit selectors.
///
/// `child` is the node whose direct parents should be inspected.
///
/// `step` is the candidate step whose pick commit should be matched against the
/// already-connected parents of `child`.
///
/// Returns the matching direct parent selector when `step` already corresponds
/// to an attached pick parent, or `None` otherwise.
pub(crate) fn already_connected_parent_for_step<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    child: Selector,
    step: &Step,
) -> Result<Option<Selector>> {
    let Step::Pick(pick) = step else {
        return Ok(None);
    };

    let Some(existing_pick) = editor.try_select_commit(pick.id) else {
        return Ok(None);
    };

    let direct_parents = editor.direct_parents(child)?;
    Ok(direct_parents
        .into_iter()
        .find_map(|(parent, _)| (parent == existing_pick).then_some(parent)))
}

/// Connect `child` to `parent_step`, reusing an existing pick node when possible.
///
/// The new edge gets the smallest currently unused parent order on `child`, which keeps
/// parent ordering stable while allowing callers to splice additional parents into a node.
///
/// `editor` is the mutable graph editor that may reuse an existing pick or add a
/// new step before creating the edge.
///
/// `child` is the selector that should gain a new direct parent.
///
/// `parent_step` describes the parent node to connect, either by reusing an
/// existing pick/reference selector or by inserting a new pick step first.
///
/// Returns the selector of the connected parent node.
pub(crate) fn connect_parent_step<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    child: Selector,
    parent_step: Step,
) -> Result<Selector> {
    let parent = match parent_step {
        Step::Pick(pick) => {
            if let Some(existing_pick) = editor.try_select_commit(pick.id) {
                existing_pick
            } else {
                editor.add_step(Step::Pick(pick))?
            }
        }
        Step::Reference { ref refname } => editor.select_reference(refname.as_ref())?,
        Step::None => bail!("BUG: trying to connect to none"),
    };

    let used_orders = editor
        .direct_parents(child)?
        .into_iter()
        .map(|(_, order)| order)
        .collect::<HashSet<_>>();
    let mut order = 0;
    while used_orders.contains(&order) {
        order += 1;
    }

    editor.add_edge(child, parent, order)?;
    Ok(parent)
}

/// Find all parent-reachable nodes from and including the provided tip.
///
/// `editor` provides the parent-edge traversal used for the walk.
///
/// `tip` is the starting selector whose ancestors should be collected.
///
/// Returns the set containing `tip` and every selector reachable from it by
/// repeatedly following direct parent edges.
pub(crate) fn traverse_nodes<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    tip: Selector,
) -> Result<HashSet<Selector>> {
    let mut seen = HashSet::from([tip]);
    let mut tips = vec![tip];

    while let Some(tip) = tips.pop() {
        for (parent, _) in editor.direct_parents(tip)? {
            if seen.insert(parent) {
                tips.push(parent);
            }
        }
    }

    Ok(seen)
}
