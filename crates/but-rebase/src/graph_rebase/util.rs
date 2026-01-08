//! Utilities around the step graph for internal use.

use std::collections::HashSet;

use petgraph::visit::EdgeRef as _;

use crate::graph_rebase::{Pick, Step, StepGraph, StepGraphIndex};

/// Find the parents of a given node that are commit - in correct parent
/// ordering.
///
/// We do this via a pruned depth first search.
pub(crate) fn collect_ordered_parents(
    graph: &StepGraph,
    target: StepGraphIndex,
) -> Vec<StepGraphIndex> {
    let mut potential_parent_edges = graph
        .edges_directed(target, petgraph::Direction::Outgoing)
        .collect::<Vec<_>>();
    potential_parent_edges.sort_by_key(|e| e.weight().order);
    potential_parent_edges.reverse();

    let mut seen = potential_parent_edges
        .iter()
        .map(|e| e.target())
        .collect::<HashSet<_>>();

    let mut parents = vec![];

    while let Some(candidate) = potential_parent_edges.pop() {
        if let Step::Pick(Pick { .. }) = graph[candidate.target()] {
            parents.push(candidate.target());
            // Don't pursue the children
            continue;
        };

        let mut outgoings = graph
            .edges_directed(candidate.target(), petgraph::Direction::Outgoing)
            .collect::<Vec<_>>();
        outgoings.sort_by_key(|e| e.weight().order);
        outgoings.reverse();

        for edge in outgoings {
            if seen.insert(edge.target()) {
                potential_parent_edges.push(edge);
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
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/foobar".try_into()?,
            });
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
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/foobar".try_into()?,
            });
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
