//! Perform the actual rebase operations

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit},
};
use petgraph::{Direction, visit::EdgeRef};

use crate::graph_rebase::{
    Editor, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
    cherry_pick::{CherryPickOutcome, cherry_pick},
    util::collect_ordered_parents,
};

impl Editor {
    /// Perform the rebase
    pub fn rebase(self) -> Result<SuccessfulRebase> {
        // First we want to get a list of nodes that can be reached by
        // traversing downwards from the heads that we care about.
        // Usually there would be just one "head" which is an index to access
        // the reference step for `gitbutler/workspace`, but there could be
        // multiple.

        validate_reference_constraints(&self.graph)?;

        let mut ref_edits = vec![];
        let steps_to_pick = order_steps_picking(
            &self.graph,
            &self
                .graph
                .externals(Direction::Incoming)
                .collect::<Vec<_>>(),
        );

        // A 1 to 1 mapping between the incoming graph and the output graph
        let mut graph_mapping: HashMap<StepGraphIndex, StepGraphIndex> = HashMap::new();
        // The step graph with updated commit oids
        let mut output_graph = StepGraph::new();
        let mut unchanged_references = vec![];

        for step_idx in steps_to_pick {
            // Do the frikkin rebase man!
            let step = self.graph[step_idx].clone();
            let new_idx = match step {
                Step::Pick(pick) => {
                    let graph_parents = collect_ordered_parents(&self.graph, step_idx);
                    let ontos = match pick.preserved_parents.clone() {
                        Some(ontos) => ontos,
                        None => graph_parents
                            .iter()
                            .map(|idx| {
                                let Some(new_idx) = graph_mapping.get(idx) else {
                                    bail!("A matching parent can't be found in the output graph");
                                };

                                match output_graph[*new_idx] {
                                    Step::Pick(Pick { id, .. }) => Ok(id),
                                    _ => bail!("A parent in the output graph is not a pick"),
                                }
                            })
                            .collect::<Result<Vec<_>>>()?,
                    };

                    let outcome =
                        cherry_pick(&self.repo, pick.id, &ontos, pick.sign_if_configured)?;

                    if matches!(outcome, CherryPickOutcome::ConflictedCommit(_))
                        && !pick.conflictable
                    {
                        bail!(
                            "Commit {} was marked as not conflictable, but resulted in a conflicted state",
                            pick.id
                        );
                    }

                    match outcome {
                        CherryPickOutcome::Commit(new_id)
                        | CherryPickOutcome::ConflictedCommit(new_id)
                        | CherryPickOutcome::Identity(new_id) => {
                            let mut new_pick = pick.clone();
                            new_pick.id = new_id;
                            let new_idx = output_graph.add_node(Step::Pick(new_pick));
                            graph_mapping.insert(step_idx, new_idx);

                            new_idx
                        }
                        CherryPickOutcome::FailedToMergeBases => {
                            // Exit early - the rebase failed because it encountered a commit it couldn't pick
                            // TODO(CTO): Detect if this was the merge commit itself & signal that seperatly
                            bail!("Failed to merge bases for commit {}", pick.id);
                        }
                    }
                }
                Step::Reference { refname } => {
                    let graph_parents = collect_ordered_parents(&self.graph, step_idx);
                    let first_parent_idx = graph_parents
                        .first()
                        .context("References should have at least one parent")?;
                    let Some(new_idx) = graph_mapping.get(first_parent_idx) else {
                        bail!("A matching parent can't be found in the output graph");
                    };

                    let to_reference = match output_graph[*new_idx] {
                        Step::Pick(Pick { id, .. }) => id,
                        _ => bail!("A parent in the output graph is not a pick"),
                    };

                    let reference = self.repo.try_find_reference(&refname)?;

                    if let Some(reference) = reference {
                        let target = reference.target();
                        match target {
                            gix::refs::TargetRef::Object(id) => {
                                if id == to_reference {
                                    unchanged_references.push(refname.clone());
                                } else {
                                    ref_edits.push(RefEdit {
                                        name: refname.clone(),
                                        change: Change::Update {
                                            log: LogChange::default(),
                                            expected: PreviousValue::MustExistAndMatch(
                                                target.into(),
                                            ),
                                            new: Target::Object(to_reference),
                                        },
                                        deref: false,
                                    });
                                }
                            }
                            gix::refs::TargetRef::Symbolic(name) => {
                                bail!("Attempted to update the symbolic reference {}", name);
                            }
                        }
                    } else {
                        ref_edits.push(RefEdit {
                            name: refname.clone(),
                            change: Change::Update {
                                log: LogChange::default(),
                                expected: PreviousValue::MustNotExist,
                                new: Target::Object(to_reference),
                            },
                            deref: false,
                        });
                    };

                    output_graph.add_node(Step::Reference { refname })
                }
                Step::None => output_graph.add_node(Step::None),
            };

            graph_mapping.insert(step_idx, new_idx);

            let mut edges = self
                .graph
                .edges_directed(step_idx, petgraph::Direction::Outgoing)
                .collect::<Vec<_>>();
            edges.sort_by_key(|e| e.weight().order);
            edges.reverse();

            for e in edges {
                let Some(new_parent) = graph_mapping.get(&e.target()) else {
                    bail!("Failed to find corresponding parent");
                };

                output_graph.add_edge(new_idx, *new_parent, e.weight().clone());
            }
        }

        // Find deleted references
        for reference in self.initial_references.iter() {
            if !ref_edits
                .iter()
                .any(|e| e.name.as_ref() == reference.as_ref())
                && !unchanged_references
                    .iter()
                    .any(|e| e.as_ref() == reference.as_ref())
            {
                ref_edits.push(RefEdit {
                    name: reference.clone(),
                    change: Change::Delete {
                        log: gix::refs::transaction::RefLog::AndReference,
                        expected: PreviousValue::MustExist,
                    },
                    deref: false,
                });
            }
        }

        let mut history = self.history;
        history.add_revision(graph_mapping);

        Ok(SuccessfulRebase {
            repo: self.repo,
            initial_references: self.initial_references,
            ref_edits,
            graph: output_graph,
            checkouts: self.checkouts.to_owned(),
            history,
        })
    }
}

/// Validates the `parents_must_be_references` constraint on `Pick` steps in the
/// step graph
fn validate_reference_constraints(graph: &StepGraph) -> Result<()> {
    for ni in graph.node_indices() {
        let node = &graph[ni];
        let Step::Pick(pick) = node else {
            continue;
        };

        if pick.parents_must_be_references && !all_parents_are_references(graph, ni) {
            bail!("Commit {} has parents that are not referenced", pick.id);
        }
    }

    Ok(())
}

/// Returns true if all the parents of a step reach a `Reference` step, skipping
/// over `None`s
pub(crate) fn all_parents_are_references(graph: &StepGraph, target: StepGraphIndex) -> bool {
    let mut potential_parent_edges = graph
        .edges_directed(target, petgraph::Direction::Outgoing)
        .collect::<Vec<_>>();

    let mut seen = potential_parent_edges
        .iter()
        .map(|e| e.target())
        .collect::<HashSet<_>>();

    while let Some(candidate) = potential_parent_edges.pop() {
        match graph[candidate.target()] {
            // We encountered a `pick` step before a `reference` step so we can
            // stop the search and return false early.
            Step::Pick(_) => return false,
            // We can stop searching down this leg since we found a reference.
            Step::Reference { .. } => continue,
            // Skip over `None`s and consider their parents.
            Step::None => {
                let outgoings = graph
                    .edges_directed(candidate.target(), petgraph::Direction::Outgoing)
                    .collect::<Vec<_>>();

                for edge in outgoings {
                    if seen.insert(edge.target()) {
                        potential_parent_edges.push(edge);
                    }
                }
            }
        }
    }

    true
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
    let mut heads = heads.to_vec();
    let mut seen = heads.iter().cloned().collect::<HashSet<StepGraphIndex>>();
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
    mod order_steps_picking {
        use std::str::FromStr;

        use anyhow::Result;

        use crate::graph_rebase::{
            Edge, Step, StepGraph, rebase::order_steps_picking, testing::TestingDot as _,
        };

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "3000000000000000000000000000000000000000",
            )?));

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
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "3000000000000000000000000000000000000000",
            )?));
            let d = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "4000000000000000000000000000000000000000",
            )?));
            let e = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "5000000000000000000000000000000000000000",
            )?));
            let f = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "6000000000000000000000000000000000000000",
            )?));
            let g = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "7000000000000000000000000000000000000000",
            )?));
            let h = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "8000000000000000000000000000000000000000",
            )?));
            let i = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "8000000000000000000000000000000000000000",
            )?));
            let j = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "8000000000000000000000000000000000000000",
            )?));

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
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "3000000000000000000000000000000000000000",
            )?));
            let d = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "4000000000000000000000000000000000000000",
            )?));
            let e = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "5000000000000000000000000000000000000000",
            )?));

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
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "3000000000000000000000000000000000000000",
            )?));
            let d = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "4000000000000000000000000000000000000000",
            )?));
            let e = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "5000000000000000000000000000000000000000",
            )?));

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

    mod all_parents_are_references {
        use std::str::FromStr;

        use anyhow::Result;

        use crate::graph_rebase::{Edge, Step, StepGraph, rebase::all_parents_are_references};

        #[test]
        fn returns_true_when_single_parent_is_reference() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });

            graph.add_edge(a, b, Edge { order: 0 });

            assert!(all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_false_when_single_parent_is_pick() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));

            graph.add_edge(a, b, Edge { order: 0 });

            assert!(!all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_true_when_parent_is_none_leading_to_reference() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::None);
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });

            assert!(all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_false_when_parent_is_none_leading_to_pick() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::None);
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });

            assert!(!all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_true_when_no_parents() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));

            assert!(all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_true_when_all_multiple_parents_are_references() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });
            let c = graph.add_node(Step::Reference {
                refname: "refs/heads/feature".try_into()?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(a, c, Edge { order: 1 });

            assert!(all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_false_when_one_of_multiple_parents_is_pick() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(a, c, Edge { order: 1 });

            assert!(!all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_true_with_deep_none_chain_to_reference() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::None);
            let c = graph.add_node(Step::None);
            let d = graph.add_node(Step::None);
            let e = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });
            graph.add_edge(c, d, Edge { order: 0 });
            graph.add_edge(d, e, Edge { order: 0 });

            assert!(all_parents_are_references(&graph, a));
            Ok(())
        }

        #[test]
        fn returns_false_when_none_leads_to_pick_among_multiple_paths() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::None);
            let c = graph.add_node(Step::None);
            let d = graph.add_node(Step::Reference {
                refname: "refs/heads/main".try_into()?,
            });
            let e = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));

            // Path 1: a -> b -> d (Reference) - OK
            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, d, Edge { order: 0 });

            // Path 2: a -> c -> e (Pick) - NOT OK
            graph.add_edge(a, c, Edge { order: 1 });
            graph.add_edge(c, e, Edge { order: 0 });

            assert!(!all_parents_are_references(&graph, a));
            Ok(())
        }
    }
}
