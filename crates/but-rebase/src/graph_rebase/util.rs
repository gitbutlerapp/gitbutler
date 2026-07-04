//! Utilities around the step graph for internal use.

use std::collections::HashSet;

use crate::graph_rebase::{Direction, StepGraph, StepGraphIndex};

/// Find the parents of a given node that are commit - in correct parent
/// ordering.
///
/// We do this via a pruned depth first search.
pub(crate) fn collect_ordered_parents(
    graph: &StepGraph,
    target: StepGraphIndex,
) -> Vec<StepGraphIndex> {
    ordered_commit_parents(graph, target)
}

/// Pruned depth-first search for `target`'s commit parents in parent order, descending through
/// non-commit steps.
///
/// A parent slot that carries a reference chain (the edge is a stored approach entry of a chain
/// anchored at its pick) yields to any plain slot resolving to the same pick, and only the
/// first of several carrying slots survives — the same collapse the node-era search produced
/// when a ref path and a direct path reached one pick. Plain duplicate slots are all kept
/// (dup-parents workspace commits).
fn ordered_commit_parents(graph: &StepGraph, target: StepGraphIndex) -> Vec<StepGraphIndex> {
    let mut potential_parent_edges = graph
        .edges_directed(target, Direction::Outgoing)
        .collect::<Vec<_>>();
    potential_parent_edges.sort_by_key(|e| e.weight().order);

    let carries_chain = |edge: &crate::graph_rebase::step_graph::StepEdgeRef<'_>| {
        graph.is_pick(edge.target())
            && graph.anchored_refs().any(|(node, stored)| {
                crate::graph_rebase::positions::ref_approach(graph, node)
                    .contains(&(target, edge.weight().order))
                    && crate::graph_rebase::positions::resolve_to_pick(graph, stored.anchor)
                        == Some(edge.target())
            })
    };
    let plain_targets: HashSet<StepGraphIndex> = potential_parent_edges
        .iter()
        .filter(|e| graph.is_pick(e.target()) && !carries_chain(e))
        .map(|e| e.target())
        .collect();
    let mut emitted_carrying = HashSet::new();

    let mut potential: Vec<(StepGraphIndex, bool)> = potential_parent_edges
        .iter()
        .rev()
        .map(|e| (e.target(), carries_chain(e)))
        .collect();
    let mut seen = potential
        .iter()
        .map(|(t, _)| *t)
        .collect::<HashSet<StepGraphIndex>>();

    let mut parents = vec![];

    while let Some((node, carrying)) = potential.pop() {
        if graph.is_pick(node) {
            if carrying && (plain_targets.contains(&node) || !emitted_carrying.insert(node)) {
                continue;
            }
            parents.push(node);
            // Don't pursue the children
            continue;
        };

        let mut outgoings = graph
            .edges_directed(node, Direction::Outgoing)
            .collect::<Vec<_>>();
        outgoings.sort_by_key(|e| e.weight().order);
        outgoings.reverse();

        for edge in outgoings {
            if seen.insert(edge.target()) {
                potential.push((edge.target(), false));
            }
        }
    }

    parents
}

#[cfg(test)]
mod test {
    mod collect_ordered_parents {
        use std::str::FromStr as _;

        use anyhow::Result;

        use crate::graph_rebase::{Edge, Step, StepGraph, util::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            // Second parent - is a reference
            let c = graph.add_reference("refs/heads/foobar".try_into()?, true);
            // Second parent's first child
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::new_pick(d_id));
            // Second parent's second child
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::new_pick(e_id));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::new_pick(f_id));

            // A's parents
            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(a, c, Edge { order: 1 });
            graph.add_edge(a, f, Edge { order: 2 });

            // C's parents
            graph.add_edge(c, d, Edge { order: 0 });
            graph.add_edge(c, e, Edge { order: 1 });

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[b, d, e, f]);

            Ok(())
        }

        #[test]
        fn insertion_order_is_irrelevant() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            // Second parent - is a reference
            let c = graph.add_reference("refs/heads/foobar".try_into()?, true);
            // Second parent's second child
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::new_pick(d_id));
            // Second parent's first child
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::new_pick(e_id));
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::new_pick(f_id));

            // A's parents
            graph.add_edge(a, f, Edge { order: 2 });
            graph.add_edge(a, c, Edge { order: 1 });
            graph.add_edge(a, b, Edge { order: 0 });

            // C's parents
            graph.add_edge(c, d, Edge { order: 1 });
            graph.add_edge(c, e, Edge { order: 0 });

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[b, e, d, f]);

            Ok(())
        }
    }
}
