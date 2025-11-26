//! Perform the actual rebase operations

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    ReferenceSpec,
    cherry_pick::EmptyCommit,
    cherry_pick_one,
    graph_rebase::{Editor, Step, StepGraph, StepGraphIndex},
    to_commit,
};
use anyhow::{Context, Result};
use petgraph::visit::EdgeRef;

/// Represents the rebase output and the varying degrees of success it had.
pub struct RebaseResult {
    references: Vec<ReferenceSpec>,
    commit_map: HashMap<gix::ObjectId, gix::ObjectId>,
}

impl Editor {
    /// Perform the rebase
    pub fn rebase(&self, repo: &gix::Repository) -> Result<RebaseResult> {
        // First we want to get a list of nodes that can be reached by
        // traversing downwards from the heads that we care about.
        // Usually there would be just one "head" which is an index to access
        // the reference step for `gitbutler/workspace`, but there could be
        // multiple.

        let pick_mode = crate::cherry_pick::PickMode::SkipIfNoop;

        let steps_to_pick = order_steps_picking(&self.graph, &self.heads);

        // A 1 to 1 mapping between the incoming graph and hte output graph
        let mut graph_mapping: HashMap<StepGraphIndex, StepGraphIndex> = HashMap::new();
        // The step graph with updated commit oids
        let mut output_graph = StepGraph::new();

        for step_idx in steps_to_pick {
            // Do the frikkin rebase man!
            let step = self.graph[step_idx].clone();
            match step {
                Step::Pick { id } => {
                    let commit = to_commit(repo, id)?;
                    let graph_parents = collect_ordered_parents(&self.graph, step_idx);

                    match (commit.parents.len(), graph_parents.len()) {
                        (0, 0) => {
                            let new_idx = output_graph.add_node(step);
                            graph_mapping.insert(step_idx, new_idx);
                        }
                        (1, 0) => {
                            todo!("1 to 0 cherry pick is not implemented yet");
                        }
                        (0, 1) => {
                            todo!("0 to 1 cherry pick is not implemented yet");
                        }
                        (1, 1) => {
                            let (_, parent_id) = graph_parents.first().expect("Impossible");

                            let new_commit = cherry_pick_one(
                                repo,
                                *parent_id,
                                id,
                                pick_mode,
                                EmptyCommit::Keep,
                            )?;

                            let new_idx = output_graph.add_node(Step::Pick { id: new_commit });
                            graph_mapping.insert(step_idx, new_idx);
                        }
                        (_, _) => {
                            todo!("N to >2 parents & >2 parents to N is not implemented yet");
                        }
                    };
                }
                Step::Reference { refname } => {}
                Step::None => {}
            };
        }

        todo!()
    }
}

/// Find the parents of a given node that are commit - in correct parent
/// ordering.
///
/// We do this via a pruned depth first search.
fn collect_ordered_parents(
    graph: &StepGraph,
    target: StepGraphIndex,
) -> Vec<(StepGraphIndex, gix::ObjectId)> {
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
        if let Step::Pick { id } = graph[candidate.target()] {
            parents.push((candidate.target(), id));
            // Don't persue the children
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

/// Creates a list of step indicies ordered in the dependency order.
///
/// We do this by first doing a breadth-first traversal down from the heads
/// (which would usually be the `gitbutler/workspace` reference step) in order
/// to determine which steps are reachable, and what the bottom most steps are.
///
/// Then, we do a second traversal up from those bottom most
/// steps.
///
/// This second traversal ensures that all the parents of any given node have
/// been seen, before traversing it.
fn order_steps_picking(graph: &StepGraph, heads: &[StepGraphIndex]) -> VecDeque<StepGraphIndex> {
    let mut seen = heads.iter().cloned().collect::<HashSet<StepGraphIndex>>();
    let mut heads = heads.to_vec();
    // Reachable nodes with no outgoing nodes.
    let mut bases = VecDeque::new();

    while let Some(head) = heads.pop() {
        let edges = graph.edges_directed(head, petgraph::Direction::Outgoing);

        if edges.clone().count() == 0 {
            bases.push_back(head);
            continue;
        }

        for edge in edges {
            let t = edge.target();
            if seen.insert(t) {
                heads.push(t);
            }
        }
    }

    // Now we want to create a vector that contains all the steps in
    // dependency order.
    let mut ordered = bases.clone();
    let mut retraversed = bases.iter().cloned().collect::<HashSet<_>>();

    while let Some(base) = bases.pop_front() {
        for edge in graph.edges_directed(base, petgraph::Direction::Incoming) {
            // We only want to queue nodes for traversing that have had all of their parents traversed.
            let s = edge.source();
            let mut outgoing_edges = graph.edges_directed(s, petgraph::Direction::Outgoing);
            let all_parents_seen = outgoing_edges.clone().count() == 0
                || outgoing_edges.all(|e| retraversed.contains(&e.target()));
            if all_parents_seen && seen.contains(&s) && retraversed.insert(s) {
                bases.push_back(s);
                ordered.push_back(s);
            };
        }
    }

    ordered
}

#[cfg(test)]
mod test {
    mod collect_ordered_parents {
        use std::str::FromStr as _;

        use anyhow::Result;

        use crate::graph_rebase::{Edge, Step, StepGraph, rebase::collect_ordered_parents};

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::Pick { id: a_id });
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::Pick { id: b_id });
            // Second parent - is a reference
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/foobar".try_into()?,
            });
            // Second parent's first child
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::Pick { id: d_id });
            // Second parent's second child
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::Pick { id: e_id });
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::Pick { id: f_id });

            // A's parents
            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(a, c, Edge { order: 1 });
            graph.add_edge(a, f, Edge { order: 2 });

            // C's parents
            graph.add_edge(c, d, Edge { order: 0 });
            graph.add_edge(c, e, Edge { order: 1 });

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[(b, b_id), (d, d_id), (e, e_id), (f, f_id)]);

            Ok(())
        }

        #[test]
        fn insertion_order_is_irrelivant() -> Result<()> {
            let mut graph = StepGraph::new();
            let a_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let a = graph.add_node(Step::Pick { id: a_id });
            // First parent
            let b_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;
            let b = graph.add_node(Step::Pick { id: b_id });
            // Second parent - is a reference
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/foobar".try_into()?,
            });
            // Second parent's second child
            let d_id = gix::ObjectId::from_str("3000000000000000000000000000000000000000")?;
            let d = graph.add_node(Step::Pick { id: d_id });
            // Second parent's first child
            let e_id = gix::ObjectId::from_str("4000000000000000000000000000000000000000")?;
            let e = graph.add_node(Step::Pick { id: e_id });
            // Third parent
            let f_id = gix::ObjectId::from_str("5000000000000000000000000000000000000000")?;
            let f = graph.add_node(Step::Pick { id: f_id });

            // A's parents
            graph.add_edge(a, f, Edge { order: 2 });
            graph.add_edge(a, c, Edge { order: 1 });
            graph.add_edge(a, b, Edge { order: 0 });

            // C's parents
            graph.add_edge(c, d, Edge { order: 1 });
            graph.add_edge(c, e, Edge { order: 0 });

            let parents = collect_ordered_parents(&graph, a);
            assert_eq!(&parents, &[(b, b_id), (e, e_id), (d, d_id), (f, f_id)]);

            Ok(())
        }
    }

    mod order_steps_picking {
        use anyhow::Result;
        use std::str::FromStr;

        use crate::graph_rebase::{
            Edge, Step, StepGraph, rebase::order_steps_picking, testing::TestingDot as _,
        };

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("1000000000000000000000000000000000000000")?,
            });
            let b = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("2000000000000000000000000000000000000000")?,
            });
            let c = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("3000000000000000000000000000000000000000")?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });

            insta::assert_snapshot!(graph.steps_dot(), @r#"
            digraph {
                0 [ label="pick: 1000000000000000000000000000000000000000"]
                1 [ label="pick: 2000000000000000000000000000000000000000"]
                2 [ label="pick: 3000000000000000000000000000000000000000"]
                0 -> 1 [ label="order: 0"]
                1 -> 2 [ label="order: 0"]
            }
            "#);

            let ordered_from_a = order_steps_picking(&graph, &[a]);
            assert_eq!(&ordered_from_a, &[c, b, a]);
            let ordered_from_b = order_steps_picking(&graph, &[b]);
            assert_eq!(&ordered_from_b, &[c, b]);
            let ordered_from_c = order_steps_picking(&graph, &[c]);
            assert_eq!(&ordered_from_c, &[c]);

            Ok(())
        }

        #[test]
        fn complex_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("1000000000000000000000000000000000000000")?,
            });
            let b = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("2000000000000000000000000000000000000000")?,
            });
            let c = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("3000000000000000000000000000000000000000")?,
            });
            let d = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("4000000000000000000000000000000000000000")?,
            });
            let e = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("5000000000000000000000000000000000000000")?,
            });
            let f = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("6000000000000000000000000000000000000000")?,
            });
            let g = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("7000000000000000000000000000000000000000")?,
            });
            let h = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("8000000000000000000000000000000000000000")?,
            });
            let i = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("8000000000000000000000000000000000000000")?,
            });
            let j = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("8000000000000000000000000000000000000000")?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });
            graph.add_edge(c, d, Edge { order: 0 });
            graph.add_edge(d, e, Edge { order: 0 });

            graph.add_edge(f, g, Edge { order: 0 });
            graph.add_edge(g, c, Edge { order: 0 });

            graph.add_edge(h, d, Edge { order: 0 });

            graph.add_edge(i, j, Edge { order: 0 });

            insta::assert_snapshot!(graph.steps_dot(), @r#"
            digraph {
                0 [ label="pick: 1000000000000000000000000000000000000000"]
                1 [ label="pick: 2000000000000000000000000000000000000000"]
                2 [ label="pick: 3000000000000000000000000000000000000000"]
                3 [ label="pick: 4000000000000000000000000000000000000000"]
                4 [ label="pick: 5000000000000000000000000000000000000000"]
                5 [ label="pick: 6000000000000000000000000000000000000000"]
                6 [ label="pick: 7000000000000000000000000000000000000000"]
                7 [ label="pick: 8000000000000000000000000000000000000000"]
                8 [ label="pick: 8000000000000000000000000000000000000000"]
                9 [ label="pick: 8000000000000000000000000000000000000000"]
                0 -> 1 [ label="order: 0"]
                1 -> 2 [ label="order: 0"]
                2 -> 3 [ label="order: 0"]
                3 -> 4 [ label="order: 0"]
                5 -> 6 [ label="order: 0"]
                6 -> 2 [ label="order: 0"]
                7 -> 3 [ label="order: 0"]
                8 -> 9 [ label="order: 0"]
            }
            "#);

            let ordered_from_a = order_steps_picking(&graph, &[f, h]);
            assert_eq!(&ordered_from_a, &[e, d, h, c, g, f]);

            Ok(())
        }

        #[test]
        fn merge_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("1000000000000000000000000000000000000000")?,
            });
            let b = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("2000000000000000000000000000000000000000")?,
            });
            let c = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("3000000000000000000000000000000000000000")?,
            });
            let d = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("4000000000000000000000000000000000000000")?,
            });
            let e = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("5000000000000000000000000000000000000000")?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });

            graph.add_edge(a, d, Edge { order: 1 });
            graph.add_edge(d, e, Edge { order: 0 });
            graph.add_edge(e, b, Edge { order: 0 });

            insta::assert_snapshot!(graph.steps_dot(), @r#"
            digraph {
                0 [ label="pick: 1000000000000000000000000000000000000000"]
                1 [ label="pick: 2000000000000000000000000000000000000000"]
                2 [ label="pick: 3000000000000000000000000000000000000000"]
                3 [ label="pick: 4000000000000000000000000000000000000000"]
                4 [ label="pick: 5000000000000000000000000000000000000000"]
                0 -> 1 [ label="order: 0"]
                1 -> 2 [ label="order: 0"]
                0 -> 3 [ label="order: 1"]
                3 -> 4 [ label="order: 0"]
                4 -> 1 [ label="order: 0"]
            }
            "#);

            let ordered_from_a = order_steps_picking(&graph, &[a]);
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }

        #[test]
        fn merge_flipped_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("1000000000000000000000000000000000000000")?,
            });
            let b = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("2000000000000000000000000000000000000000")?,
            });
            let c = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("3000000000000000000000000000000000000000")?,
            });
            let d = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("4000000000000000000000000000000000000000")?,
            });
            let e = graph.add_node(Step::Pick {
                id: gix::ObjectId::from_str("5000000000000000000000000000000000000000")?,
            });

            graph.add_edge(a, d, Edge { order: 0 });
            graph.add_edge(d, e, Edge { order: 0 });
            graph.add_edge(e, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });

            graph.add_edge(a, b, Edge { order: 1 });

            insta::assert_snapshot!(graph.steps_dot(), @r#"
            digraph {
                0 [ label="pick: 1000000000000000000000000000000000000000"]
                1 [ label="pick: 2000000000000000000000000000000000000000"]
                2 [ label="pick: 3000000000000000000000000000000000000000"]
                3 [ label="pick: 4000000000000000000000000000000000000000"]
                4 [ label="pick: 5000000000000000000000000000000000000000"]
                0 -> 3 [ label="order: 0"]
                3 -> 4 [ label="order: 0"]
                4 -> 1 [ label="order: 0"]
                1 -> 2 [ label="order: 0"]
                0 -> 1 [ label="order: 1"]
            }
            "#);

            let ordered_from_a = order_steps_picking(&graph, &[a]);
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }
    }
}
