//! Utilities around the step graph for internal use.

use std::collections::HashSet;

use but_graph::{BoundaryKind, NodeKind};

use crate::graph_rebase::{StepGraph, StepGraphIndex};

/// Whether the node at `index` stands for an addressable commit
/// (a materialized commit or a convergence boundary).
fn is_commit_like(graph: &StepGraph, index: StepGraphIndex) -> bool {
    matches!(
        graph.nodes()[index].kind(),
        NodeKind::Commit { .. }
            | NodeKind::Boundary {
                reason: BoundaryKind::Convergence,
                ..
            }
    )
}

/// Find the commit-like parents of a given node, in parent-slot order.
///
/// Non-commit nodes are transparent: a reference stands on its target (its
/// *last* parent, matching the node-graph convention), a placeholder expands
/// into all of its parents in order, and a shallow boundary contributes
/// nothing. Commits reachable through several paths are emitted once, at
/// their first encounter.
pub(crate) fn collect_ordered_parents(
    graph: &StepGraph,
    target: StepGraphIndex,
) -> Vec<StepGraphIndex> {
    let mut pending = graph.parents(target).to_vec();
    pending.reverse();
    let mut seen = pending.iter().copied().collect::<HashSet<_>>();
    let mut parents = Vec::new();

    while let Some(candidate) = pending.pop() {
        if is_commit_like(graph, candidate) {
            parents.push(candidate);
            // Don't pursue the commit's own parents.
            continue;
        }
        for slot in expansion_slots(graph, candidate).iter().rev() {
            if seen.insert(*slot) {
                pending.push(*slot);
            }
        }
    }

    parents
}

/// The parent slots a transparent node stands on: the target (last) parent for
/// a reference, every parent for a placeholder, nothing for a shallow boundary.
fn expansion_slots(graph: &StepGraph, index: StepGraphIndex) -> &[StepGraphIndex] {
    match graph.nodes()[index].kind() {
        NodeKind::Reference(_) => {
            let parents = graph.parents(index);
            if parents.is_empty() {
                &[]
            } else {
                &parents[parents.len() - 1..]
            }
        }
        NodeKind::None => graph.parents(index),
        NodeKind::Commit { .. }
        | NodeKind::Boundary {
            reason: BoundaryKind::Convergence,
            ..
        } => unreachable!("commit-like nodes are never expanded"),
        NodeKind::Boundary {
            reason: BoundaryKind::Shallow,
            ..
        } => &[],
    }
}

/// Resolve `index` itself to the commit-like node it stands on, following
/// transparent nodes.
///
/// For a reference this is its target commit; for a placeholder the first
/// commit its ordered parents resolve to.
pub(crate) fn resolve_to_commit(
    graph: &StepGraph,
    index: StepGraphIndex,
) -> Option<StepGraphIndex> {
    let mut pending = vec![index];
    let mut seen = HashSet::new();
    while let Some(candidate) = pending.pop() {
        if !seen.insert(candidate) {
            continue;
        }
        if is_commit_like(graph, candidate) {
            return Some(candidate);
        }
        for slot in expansion_slots(graph, candidate).iter().rev() {
            pending.push(*slot);
        }
    }
    None
}

#[cfg(test)]
mod test {
    mod collect_ordered_parents {
        use std::str::FromStr as _;

        use anyhow::Result;

        use crate::graph_rebase::{Step, StepGraph, util::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            // Second parent - is a reference standing on d
            let c = graph.add_node(Step::new_reference("refs/heads/foobar".try_into()?));
            // The reference's target
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::new_pick(d_id));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::new_pick(f_id));

            // A's parents
            *graph.parents_mut(a) = vec![b, c, f];
            // C's target
            *graph.parents_mut(c) = vec![d];

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[b, d, f]);

            Ok(())
        }

        #[test]
        fn a_placeholder_expands_into_all_its_parents() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // The placeholder left behind by a removed merge commit
            let none = graph.add_node(Step::None);
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::new_pick(d_id));
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::new_pick(e_id));

            *graph.parents_mut(a) = vec![none];
            *graph.parents_mut(none) = vec![d, e];

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[d, e]);

            Ok(())
        }

        #[test]
        fn a_workspace_reference_stands_on_its_last_parent() -> Result<()> {
            let mut graph = StepGraph::new();
            let child_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let child = graph.add_node(Step::new_pick(child_id));
            let workspace_ref =
                graph.add_node(Step::new_reference("refs/heads/gitbutler/workspace".try_into()?));
            let overlay = graph.add_node(Step::new_reference("refs/heads/stack".try_into()?));
            let target_id = gix::ObjectId::from_str("2000000000000000000000000000000000000000")?;
            let target = graph.add_node(Step::new_pick(target_id));
            let overlay_target_id =
                gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let overlay_target = graph.add_node(Step::new_pick(overlay_target_id));

            *graph.parents_mut(child) = vec![workspace_ref];
            // Overlay legs first, own target last.
            *graph.parents_mut(workspace_ref) = vec![overlay, target];
            *graph.parents_mut(overlay) = vec![overlay_target];

            let parents = collect_ordered_parents(&graph, child);
            assert_eq!(
                &parents,
                &[target],
                "the reference stands on its target, not its overlays"
            );

            Ok(())
        }
    }
}
