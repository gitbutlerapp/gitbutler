//! Perform the actual rebase operations

use std::{collections::HashMap, fmt::Write as _};

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit},
};
use petgraph::{algo::toposort, visit::EdgeRef};

use crate::graph_rebase::{
    Editor, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
    cherry_pick::{CherryPickOutcome, cherry_pick},
    util::collect_ordered_parents,
};

impl<'ws, 'graph, M: RefMetadata> Editor<'ws, 'graph, M> {
    /// Perform the rebase
    pub fn rebase(self) -> Result<SuccessfulRebase<'ws, 'graph, M>> {
        let mut ref_edits = vec![];
        let steps_to_pick = order_steps_picking(&self.graph)?;

        // A 1 to 1 mapping between the incoming graph and the output graph
        let mut graph_mapping: HashMap<StepGraphIndex, StepGraphIndex> = HashMap::new();
        // The step graph with updated commit oids
        let mut output_graph = StepGraph::new();
        let mut unchanged_references = vec![];

        let mut history = self.history;

        for step_idx in steps_to_pick {
            // Do the frikkin rebase man!
            let step = self.graph[step_idx].clone();
            let new_idx = match step {
                Step::Pick(pick) if !pick.mutable => {
                    // Immutable picks are copied verbatim: the commit keeps its
                    // id, so there's no cherry-pick to run and nothing to record
                    // in the history mapping.
                    output_graph.add_node(Step::Pick(pick))
                }
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

                    let outcome = cherry_pick(
                        &self.repo,
                        pick.id,
                        &ontos,
                        pick.pick_mode,
                        pick.tree_merge_mode,
                        pick.sign_commit,
                    )?;

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
                            if !pick.exclude_from_tracking {
                                history.update_mapping(pick.id, new_id);
                            }

                            new_idx
                        }
                        CherryPickOutcome::FailedToMergeBases {
                            base_merge_failed,
                            bases,
                            onto_merge_failed,
                            ontos,
                        } => {
                            // Exit early - the rebase failed because it encountered a commit it couldn't pick
                            bail!(format_base_merge_error(
                                pick.id,
                                base_merge_failed,
                                bases,
                                onto_merge_failed,
                                ontos
                            ));
                        }
                    }
                }
                Step::Reference { refname, mutable } => {
                    // Immutable references are kept in the graph for traversal
                    // but never moved, created, or deleted.
                    if mutable {
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
                                    bail!("Attempted to update the symbolic reference {name}");
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
                        }
                    }

                    output_graph.add_node(Step::Reference { refname, mutable })
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

        // Find deleted references. `initial_references` only contains mutable
        // references, so immutable references are never considered for deletion.
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

        history.add_revision(graph_mapping);

        Ok(SuccessfulRebase {
            repo: self.repo,
            initial_references: self.initial_references,
            ref_edits,
            graph: output_graph,
            checkouts: self.checkouts.to_owned(),
            history,
            workspace: self.workspace,
            meta: self.meta,
            db: self.db,
        })
    }
}

/// Return every graph step in parent-before-child dependency order.
fn order_steps_picking(graph: &StepGraph) -> Result<Vec<StepGraphIndex>> {
    let mut ordered = match toposort(graph, None) {
        Ok(ordered) => ordered,
        Err(_) => bail!("BUG: Rebase editor graph contains a cycle"),
    };
    // Editor edges point from child to parent, opposite Petgraph's topological order.
    ordered.reverse();
    Ok(ordered)
}

fn format_base_merge_error(
    target: gix::ObjectId,
    base_merge_failed: bool,
    bases: Option<Vec<gix::ObjectId>>,
    onto_merge_failed: bool,
    ontos: Option<Vec<gix::ObjectId>>,
) -> String {
    fn fmt_side(out: &mut String, kind: &str, failed: bool, shas: Option<Vec<gix::ObjectId>>) {
        if failed {
            if let Some(shas) = shas {
                writeln!(
                    out,
                    "Encountered a conflict while merging the commit's {kind}: {}.",
                    shas.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .ok();
            } else {
                writeln!(
                    out,
                    "Encountered a conflict while merging the commit's {kind}."
                )
                .ok();
            }
        }
    }

    let mut out = "".to_string();
    writeln!(
        &mut out,
        "Failed to merge bases while cherry picking commit {target}."
    )
    .ok();
    fmt_side(&mut out, "original bases", base_merge_failed, bases);
    fmt_side(&mut out, "new bases", onto_merge_failed, ontos);
    writeln!(
        &mut out,
        "Any ids mentioned may be in-memory and inaccessible through the git CLI."
    )
    .ok();
    out
}

#[cfg(test)]
mod test {
    mod order_steps_picking {
        use std::str::FromStr;

        use anyhow::Result;

        use crate::graph_rebase::{
            Edge, Step, StepGraph, rebase::order_steps_picking, testing::render_ascii_graph,
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

            snapbox::assert_data_eq!(
                render_ascii_graph(&graph, |_| None),
                snapbox::str![[r#"
●  1000000
●  2000000
●  3000000
"#]]
            );

            let ordered_from_a = order_steps_picking(&graph)?;
            assert_eq!(&ordered_from_a, &[c, b, a]);

            Ok(())
        }

        #[test]
        fn incomplete_order_from_a_cycle_is_rejected() -> Result<()> {
            let mut graph = StepGraph::new();
            let commit = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            graph.add_edge(commit, commit, Edge { order: 0 });

            let err = order_steps_picking(&graph)
                .expect_err("a cyclic graph must not produce a partial rebase mapping");
            assert_eq!(err.to_string(), "BUG: Rebase editor graph contains a cycle");

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
                "9000000000000000000000000000000000000000",
            )?));
            let j = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1100000000000000000000000000000000000000",
            )?));

            graph.add_edge(a, b, Edge { order: 0 });
            graph.add_edge(b, c, Edge { order: 0 });
            graph.add_edge(c, d, Edge { order: 0 });
            graph.add_edge(d, e, Edge { order: 0 });

            graph.add_edge(f, g, Edge { order: 0 });
            graph.add_edge(g, c, Edge { order: 0 });

            graph.add_edge(h, d, Edge { order: 0 });

            graph.add_edge(i, j, Edge { order: 0 });

            snapbox::assert_data_eq!(
                render_ascii_graph(&graph, |_| None),
                snapbox::str![[r#"
●  1000000
●  2000000
│ ●  6000000
│ ●  7000000
├─╯
●  3000000
│ ●  8000000
├─╯
●  4000000
●  5000000
●  9000000
●  1100000
"#]]
            );

            let ordered_from_a = order_steps_picking(&graph)?;
            assert_eq!(
                ordered_from_a.len(),
                graph.node_count(),
                "topological ordering must include every graph component"
            );
            let position = |node| {
                ordered_from_a
                    .iter()
                    .position(|candidate| *candidate == node)
                    .expect("every graph node is present")
            };
            for (child, parent) in [
                (a, b),
                (b, c),
                (c, d),
                (d, e),
                (f, g),
                (g, c),
                (h, d),
                (i, j),
            ] {
                assert!(
                    position(parent) < position(child),
                    "every parent must be ordered before its child"
                );
            }

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

            snapbox::assert_data_eq!(
                render_ascii_graph(&graph, |_| None),
                snapbox::str![[r#"
●    1000000
├─╮
│ ●  4000000
│ ●  5000000
├─╯
●  2000000
●  3000000
"#]]
            );

            let ordered_from_a = order_steps_picking(&graph)?;
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

            snapbox::assert_data_eq!(
                render_ascii_graph(&graph, |_| None),
                snapbox::str![[r#"
●    1000000
├─╮
● │  4000000
● │  5000000
├─╯
●  2000000
●  3000000
"#]]
            );

            let ordered_from_a = order_steps_picking(&graph)?;
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }
    }
}
