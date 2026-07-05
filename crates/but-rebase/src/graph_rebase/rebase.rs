//! Perform the actual rebase operations

use std::{
    collections::{HashSet, VecDeque},
    fmt::Write as _,
};

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit},
};

use crate::graph_rebase::{
    Editor, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
    cherry_pick::{CherryPickOutcome, cherry_pick},
    util::collect_ordered_parents,
};

impl<'ws, 'graph, M: RefMetadata> Editor<'ws, 'graph, M> {
    /// Perform the rebase IN PLACE: each mutable pick's commit id is rewritten where it
    /// stands, in dependency order, so a pick's parent slots already hold rebased ids by the
    /// time it is picked. Node ids never change — parent arrays, positions, lanes, and every
    /// outstanding selector stay valid across the rebase.
    pub fn rebase(self) -> Result<SuccessfulRebase<'ws, 'graph, M>> {
        crate::graph_rebase::positions::debug_assert_positions_total(&self.graph);

        let mut graph = self.graph;
        let mut history = self.history;
        let mut ref_edits = vec![];
        let mut unchanged_references = vec![];

        // Every tip (a node with no children) seeds the traversal so every commit is
        // visited — immutable picks and tombstones are left untouched where they stand.
        let rebase_heads = graph.tips().collect::<Vec<_>>();
        let steps_to_pick = order_steps_picking(&graph, &rebase_heads);

        for step_idx in steps_to_pick {
            let Step::Pick(pick) = graph.step_view(step_idx) else {
                // Tombstones have nothing to rewrite.
                continue;
            };
            if !pick.mutable {
                // Immutable picks keep their id: no cherry-pick to run, nothing to record
                // in the history mapping.
                continue;
            }

            // Only resolve the graph parents when we actually need them — a pick with
            // `preserved_parents` already carries its onto-commits.
            let ontos = match pick.preserved_parents.clone() {
                Some(ontos) => ontos,
                None => collect_ordered_parents(&graph, step_idx)
                    .iter()
                    .map(|&idx| {
                        graph
                            .commit_id(idx)
                            .context("BUG: ordered parents must be picks")
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

            if matches!(outcome, CherryPickOutcome::ConflictedCommit(_)) && !pick.conflictable {
                bail!(
                    "Commit {} was marked as not conflictable, but resulted in a conflicted state",
                    pick.id
                );
            }

            match outcome {
                CherryPickOutcome::Commit(new_id)
                | CherryPickOutcome::ConflictedCommit(new_id)
                | CherryPickOutcome::Identity(new_id) => {
                    graph.set_commit_id(step_idx, new_id);
                    if !pick.exclude_from_tracking {
                        history.update_mapping(pick.id, new_id);
                    }
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

        // References need no rewrite at all — their position's anchor node now carries the
        // rebased id. All that remains is emitting the ref transaction: every live, mutable,
        // positioned reference moves to its anchor's new commit.
        for step_idx in graph.ref_indices() {
            let record = graph
                .reference_record(step_idx)
                .expect("ref_indices only yields references");
            if !record.live || record.position.is_none() || !record.mutable {
                // Dead records keep their retained name and position; immutable references
                // are kept in the graph for traversal but never moved, created, or deleted.
                continue;
            }
            let refname = record.refname.clone();
            let first_parent_idx =
                crate::graph_rebase::positions::resolve_to_pick(&graph, step_idx)
                    .context("References should resolve to a commit")?;
            let to_reference = match graph.commit_id(first_parent_idx) {
                Some(id) => id,
                None => bail!("A reference's anchor is not a pick"),
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
                                    expected: PreviousValue::MustExistAndMatch(target.into()),
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

        Ok(SuccessfulRebase {
            repo: self.repo,
            initial_references: self.initial_references,
            ref_edits,
            graph,
            checkouts: self.checkouts,
            history,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
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
    // References take no part in the pick order (no edges) and are replayed separately;
    // everything else — picks AND tombstones, even one carrying a leaked anchor — must be
    // traversed, or its subtree is orphaned. Filter by the STEP, not by anchor presence
    // (a non-reference with a stray anchor must not be skipped).
    let mut heads: Vec<StepGraphIndex> = heads
        .iter()
        .copied()
        .filter(|h| !graph.is_reference(*h))
        .collect();
    let mut seen = heads.iter().cloned().collect::<HashSet<StepGraphIndex>>();
    // Reachable nodes with no outgoing nodes.
    let mut bases = VecDeque::new();

    while let Some(head) = heads.pop() {
        let parents = graph.parents(head);

        if parents.is_empty() {
            bases.push_back(head);
            continue;
        }

        for t in parents {
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
        for (s, _) in graph.incoming_legs(base) {
            // We only want to queue nodes for traversing that have had all of their parents traversed.
            let all_parents_seen = graph.parents(s).iter().all(|t| retraversed.contains(t));
            if all_parents_seen && seen.contains(&s) && retraversed.insert(s) {
                bases.push_back(s);
                ordered.push_back(s);
            };
        }
    }

    ordered
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
            Step, StepGraph, rebase::order_steps_picking, testing::render_ascii_graph,
        };

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut graph = StepGraph::default();
            let a = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "1000000000000000000000000000000000000000",
            )?));
            let b = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "2000000000000000000000000000000000000000",
            )?));
            let c = graph.add_node(Step::new_pick(gix::ObjectId::from_str(
                "3000000000000000000000000000000000000000",
            )?));

            graph.push_parent(a, b);
            graph.push_parent(b, c);

            insta::assert_snapshot!(render_ascii_graph(&graph, |_| None), @"
            ●  1000000
            ●  2000000
            ●  3000000
            ");

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
            let mut graph = StepGraph::default();
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

            graph.push_parent(a, b);
            graph.push_parent(b, c);
            graph.push_parent(c, d);
            graph.push_parent(d, e);

            graph.push_parent(f, g);
            graph.push_parent(g, c);

            graph.push_parent(h, d);

            graph.push_parent(i, j);

            insta::assert_snapshot!(render_ascii_graph(&graph, |_| None), @"
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
            ");

            let ordered_from_a = order_steps_picking(&graph, &[f, h]);
            assert_eq!(&ordered_from_a, &[e, d, c, h, g, f]);

            Ok(())
        }

        #[test]
        fn merge_scenario() -> Result<()> {
            let mut graph = StepGraph::default();
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

            graph.push_parent(a, b);
            graph.push_parent(b, c);

            graph.push_parent(a, d);
            graph.push_parent(d, e);
            graph.push_parent(e, b);

            insta::assert_snapshot!(render_ascii_graph(&graph, |_| None), @"
            ●    1000000
            ├─╮
            │ ●  4000000
            │ ●  5000000
            ├─╯
            ●  2000000
            ●  3000000
            ");

            let ordered_from_a = order_steps_picking(&graph, &[a]);
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }

        #[test]
        fn merge_flipped_scenario() -> Result<()> {
            let mut graph = StepGraph::default();
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

            graph.push_parent(a, d);
            graph.push_parent(d, e);
            graph.push_parent(e, b);
            graph.push_parent(b, c);

            graph.push_parent(a, b);

            insta::assert_snapshot!(render_ascii_graph(&graph, |_| None), @"
            ●    1000000
            ├─╮
            ● │  4000000
            ● │  5000000
            ├─╯
            ●  2000000
            ●  3000000
            ");

            let ordered_from_a = order_steps_picking(&graph, &[a]);
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }
    }
}
