//! Utilities around the step graph for internal use.

use petgraph::visit::EdgeRef as _;

use crate::graph_rebase::{Step, StepGraph, StepGraphIndex};

/// Specify what stopping points should be considered
pub(crate) enum OrderedParentKind {
    CommitOnly,
    CommitOrReference,
}

/// Find the parents of a given node that are commit - in correct parent
/// ordering.
///
/// We do this via a pruned depth first search.
pub(crate) fn collect_ordered_parents(
    graph: &StepGraph,
    target: StepGraphIndex,
    stop_kind: OrderedParentKind,
) -> Vec<StepGraphIndex> {
    let mut potential_parent_edges = graph
        .edges_directed(target, petgraph::Direction::Outgoing)
        .collect::<Vec<_>>();
    potential_parent_edges.sort_by_key(|e| e.weight().order);
    potential_parent_edges.reverse();

    let mut parents = vec![];

    while let Some(candidate) = potential_parent_edges.pop() {
        match (&stop_kind, &graph[candidate.target()]) {
            (OrderedParentKind::CommitOnly, Step::Pick(..))
            | (OrderedParentKind::CommitOrReference, Step::Pick(_) | Step::Reference { .. }) => {
                parents.push(candidate.target());
                // Don't pursue the children
                continue;
            }
            _ => {}
        }

        let mut outgoings = graph
            .edges_directed(candidate.target(), petgraph::Direction::Outgoing)
            .collect::<Vec<_>>();
        outgoings.sort_by_key(|e| e.weight().order);
        outgoings.reverse();

        potential_parent_edges.extend(outgoings);
    }

    parents
}

#[cfg(test)]
mod test {
    mod collect_ordered_parents {
        use std::str::FromStr as _;

        use anyhow::Result;

        use crate::graph_rebase::{
            Edge, Step, StepGraph,
            util::{OrderedParentKind, collect_ordered_parents},
        };

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            // Second parent - is a reference
            let c = graph.add_node(Step::new_reference("refs/heads/foobar".try_into()?));
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

            let parents = collect_ordered_parents(&graph, a, OrderedParentKind::CommitOnly);
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
            let c = graph.add_node(Step::new_reference("refs/heads/foobar".try_into()?));
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

            let parents = collect_ordered_parents(&graph, a, OrderedParentKind::CommitOnly);
            assert_eq!(&parents, &[b, e, d, f]);

            Ok(())
        }

        #[test]
        fn repeated_parents_are_preserved() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::new_pick(a_id));
            let b_id = gix::ObjectId::from_str("2000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::new_pick(b_id));
            let r1 = graph.add_node(Step::new_reference("refs/heads/r1".try_into()?));
            let r2 = graph.add_node(Step::new_reference("refs/heads/r2".try_into()?));
            let none = graph.add_node(Step::None);

            graph.add_edge(a, r1, Edge { order: 0 });
            graph.add_edge(a, b, Edge { order: 1 });
            graph.add_edge(a, none, Edge { order: 2 });
            graph.add_edge(a, r2, Edge { order: 3 });
            graph.add_edge(r1, b, Edge { order: 0 });
            graph.add_edge(none, b, Edge { order: 0 });
            graph.add_edge(r2, r1, Edge { order: 0 });

            let parents = collect_ordered_parents(&graph, a, OrderedParentKind::CommitOnly);
            assert_eq!(
                &parents,
                &[b, b, b, b],
                "every parent edge of A resolves to B, so B is A's parent four times"
            );

            Ok(())
        }
    }
}
