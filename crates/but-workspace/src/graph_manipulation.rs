//! Shared graph traversal and edge-manipulation helpers.

use anyhow::{Context, Result, bail};
use but_graph::workspace::{Stack, StackSegment};
use but_graph::{
    MutableNodeGraph, NodeIndex, NodeKind,
    edit::{Pick, SegmentDelimiter, SelectorSet, SomeSelectors, ToSelector},
};
use std::collections::HashSet;

/// Payload containing information about how to disconnect a segment in the graph.
pub struct DisconnectParameters {
    /// The bounds of the segment to disconnect.
    pub(crate) delimiter: SegmentDelimiter<NodeIndex, NodeIndex>,
    /// The children of the child-most segment bound to disconnect.
    pub(crate) children_to_disconnect: SelectorSet,
    /// The parents of the parent-most segment bound to disconnect.
    pub(crate) parents_to_disconnect: SelectorSet,
}

/// Get the right disconnect parameters for the given subject segment and source stack.
///
/// This function determines which are the right parents and children to disconnect,
/// as well as the right segment delimiter to move.
pub fn get_disconnect_parameters(
    graph: &MutableNodeGraph,
    source_stack: &Stack,
    subject_segment: &StackSegment,
    workspace_head: Option<gix::ObjectId>,
) -> anyhow::Result<DisconnectParameters> {
    let index_of_segment = source_stack
        .segments
        .iter()
        .position(|segment| segment.id == subject_segment.id)
        .context("BUG: Unable to find subject segment on source stack.")?;

    let subject_segment_ref_name = subject_segment
        .ref_name()
        .context("Subject segment doesn't have a ref name.")?;
    let delimiter_child = graph
        .select_reference(subject_segment_ref_name)
        .context("Failed to find subject reference in graph.")?;
    let delimiter_parent = match subject_segment.commits.last() {
        Some(last_commit) => graph
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
    // subject rather than be cut. We read this from the mutable graph (which
    // `disconnect_segment_from` validates against) rather than the workspace projection: when the
    // target is ahead of the merge base the projection's base segment is anonymous and resolves to
    // the base commit, while the mutable graph keeps the target reference node between the branch and
    // that commit, so only the mutable-graph first parent matches the edge being checked.
    let parents_to_disconnect = match graph
        .direct_parents(delimiter.parent)?
        .into_iter()
        .min_by_key(|(_, order)| *order)
    {
        Some((base, _)) => SelectorSet::Some(SomeSelectors::new(vec![base])?),
        None => SelectorSet::All,
    };

    if index_of_segment == 0 {
        // Managed workspaces have a workspace commit above the top-most segment. Ad-hoc
        // workspaces do not have such a child, so there is no child edge to disconnect there.
        let children_to_disconnect = workspace_head
            .map(|workspace_head| -> anyhow::Result<SelectorSet> {
                let workspace_head_selector = graph
                    .select_commit(workspace_head)
                    .context("Failed to find workspace head in graph.")?;
                Ok(SelectorSet::Some(SomeSelectors::new(vec![
                    workspace_head_selector,
                ])?))
            })
            .transpose()?
            .unwrap_or(SelectorSet::None);

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
        Some(last_commit) => graph
            .select_commit(last_commit.id)
            .context("Failed to find last commit of child segment in graph."),
        None => {
            // The segment on top of the subject segment is empty. Select the reference.
            let child_segment_ref_name = child_segment
                .ref_name()
                .context("Child segment doesn't have a ref name.")?;
            graph
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
pub fn determine_parent_selector(
    graph: &MutableNodeGraph,
    subject_commit_selector: NodeIndex,
) -> anyhow::Result<SelectorSet> {
    let mut parents = graph.direct_parents(subject_commit_selector)?;
    parents.sort_by_key(|(_, order)| *order);

    let preferred = parents
        .iter()
        .find(|(selector, _)| graph.pick_at(*selector).is_some())
        .or_else(|| {
            parents.iter().find(|(selector, _)| {
                matches!(graph.nodes()[*selector].kind(), NodeKind::Reference(_))
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
/// `graph` is the mutable graph whose connectivity will be updated.
///
/// `selector` is the node whose parent edges should be removed.
///
/// Returns `Ok(())` after all direct parent edges of `selector` have been
/// removed from the graph.
pub(crate) fn disconnect_selector_from_all_parents(
    graph: &mut MutableNodeGraph,
    selector: NodeIndex,
) -> Result<()> {
    graph.disconnect_segment_from(
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
/// `graph` provides the direct parent or child edges that can be selected.
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
pub(crate) fn selected_edges_from_set(
    graph: &MutableNodeGraph,
    target: NodeIndex,
    selectors: &SelectorSet,
    edge_selection: EdgeSelection,
) -> Result<Vec<(NodeIndex, usize)>> {
    let available = match edge_selection {
        EdgeSelection::Children => graph.direct_children(target)?,
        EdgeSelection::Parents => graph.direct_parents(target)?,
    };

    match selectors {
        SelectorSet::All => Ok(available),
        SelectorSet::None => Ok(Vec::new()),
        SelectorSet::Some(some_selectors) => {
            let mut selected = Vec::new();
            for selector in some_selectors.as_slice() {
                let selector = selector.to_selector(graph)?;
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
/// `graph` is the mutable graph whose edges will be recreated.
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
/// `delimiter.parent`, no new edge is added. Otherwise, the edge is inserted
/// at the recorded parent slot, shifting later slots.
///
/// Returns `Ok(())` after the captured child and parent edges have been
/// reattached to the rebuilt segment.
pub(crate) fn connect_segment_to_edges(
    graph: &mut MutableNodeGraph,
    delimiter: SegmentDelimiter<NodeIndex, NodeIndex>,
    children: &[(NodeIndex, usize)],
    parents: &[(NodeIndex, usize)],
) -> Result<()> {
    for (child, order) in children {
        let direct_parents = graph.direct_parents(*child)?;
        if direct_parents
            .iter()
            .any(|(parent, _)| *parent == delimiter.child)
        {
            continue;
        }
        graph.add_edge(*child, delimiter.child, *order)?;
    }

    let parent_order_offset = graph
        .direct_parents(delimiter.parent)?
        .into_iter()
        .map(|(_, order)| order)
        .max()
        .map(|max| max + 1)
        .unwrap_or(0);

    for (parent, order) in parents {
        let direct_parents = graph.direct_parents(delimiter.parent)?;
        if direct_parents
            .iter()
            .any(|(existing_parent, _)| *existing_parent == *parent)
        {
            continue;
        }
        let _ = direct_parents;
        graph.add_edge(delimiter.parent, *parent, parent_order_offset + *order)?;
    }

    Ok(())
}

/// Return a direct parent of `child` when `pick` refers to a commit that is already connected.
///
/// This is useful when rebuilding a graph segment and we want to reuse an existing
/// pick node without adding a duplicate edge to the same commit.
///
/// `graph` provides access to the current parent edges and commit selectors.
///
/// `child` is the node whose direct parents should be inspected.
///
/// `pick` is the candidate pick whose commit should be matched against the
/// already-connected parents of `child`.
///
/// Returns the matching direct parent selector when `pick` already corresponds
/// to an attached pick parent, or `None` otherwise.
pub(crate) fn already_connected_parent_for_pick(
    graph: &MutableNodeGraph,
    child: NodeIndex,
    pick: &Pick,
) -> Result<Option<NodeIndex>> {
    let Some(existing_pick) = graph.try_select_commit(pick.id) else {
        return Ok(None);
    };

    let direct_parents = graph.direct_parents(child)?;
    Ok(direct_parents
        .into_iter()
        .find_map(|(parent, _)| (parent == existing_pick).then_some(parent)))
}

/// Connect `child` to `pick`, reusing an existing pick node when possible.
///
/// The new edge becomes the child's first parent, keeping the connected chain
/// on the first-parent path while extra parents shift after it.
///
/// `graph` is the mutable graph that may reuse an existing pick or add a
/// new commit node before creating the edge.
///
/// `child` is the selector that should gain a new direct parent.
///
/// `pick` describes the parent commit to connect, either by reusing an
/// existing pick selector or by adding a new commit node first.
///
/// Returns the selector of the connected parent node.
pub(crate) fn connect_parent_pick(
    graph: &mut MutableNodeGraph,
    child: NodeIndex,
    pick: Pick,
) -> Result<NodeIndex> {
    let parent = match graph.try_select_commit(pick.id) {
        Some(existing) => existing,
        None => graph.add_commit(pick),
    };

    // The chain parent is the first-parent path; any pre-existing parents
    // (e.g. a synthetic merge's second parent) shift after it.
    graph.add_edge(child, parent, 0)?;
    Ok(parent)
}

/// Find all parent-reachable nodes from and including the provided tip.
///
/// `graph` provides the parent-edge traversal used for the walk.
///
/// `tip` is the starting selector whose ancestors should be collected.
///
/// Returns the set containing `tip` and every selector reachable from it by
/// repeatedly following direct parent edges.
pub(crate) fn traverse_nodes(
    graph: &MutableNodeGraph,
    tip: NodeIndex,
) -> Result<HashSet<NodeIndex>> {
    let mut seen = HashSet::from([tip]);
    let mut tips = vec![tip];

    while let Some(tip) = tips.pop() {
        for (parent, _) in graph.direct_parents(tip)? {
            if seen.insert(parent) {
                tips.push(parent);
            }
        }
    }

    Ok(seen)
}
