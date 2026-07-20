//! Perform the actual rebase operations

use std::{collections::VecDeque, fmt::Write as _};

use anyhow::{Result, bail};
use but_core::RefMetadata;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit},
};

use crate::graph_rebase::{
    Editor, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
    cherry_pick::{CherryPickOutcome, cherry_pick},
    util::{collect_ordered_parents, resolve_to_commit},
};

impl<'ws, 'graph, M: RefMetadata> Editor<'ws, 'graph, M> {
    /// Perform the rebase
    pub fn rebase(self) -> Result<SuccessfulRebase<'ws, 'graph, M>> {
        let mut ref_edits = vec![];
        let mut unchanged_references = vec![];
        let mut history = self.history;

        // The output graph shares the input's indexes; only commit ids change.
        let mut output_graph = self.graph.clone();

        // Process parents before children so every pick lands on already
        // rewritten parents.
        for step_idx in topological_order(&self.graph)? {
            match self.graph.step(step_idx) {
                // Immutable picks are copied verbatim: the commit keeps its
                // id, so there's no cherry-pick to run and nothing to record
                // in the history mapping.
                Step::Pick(pick) if !pick.mutable => {}
                Step::Pick(pick) => {
                    let ontos = match pick.preserved_parents.clone() {
                        Some(ontos) => ontos,
                        None => collect_ordered_parents(&output_graph, step_idx)
                            .into_iter()
                            .map(|parent| match output_graph.step(parent) {
                                Step::Pick(Pick { id, .. }) => Ok(id),
                                _ => bail!("A parent in the output graph is not a pick"),
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
                            output_graph.set_step(step_idx, Step::Pick(new_pick));
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
                Step::Reference { refname, mutable } => {
                    // Immutable references are kept in the graph for traversal
                    // but never moved, created, or deleted.
                    if !mutable {
                        continue;
                    }
                    if refname.category() != Some(gix::refs::Category::LocalBranch) {
                        bail!(
                            "BUG: only local branches may be moved or created, but {refname} is marked mutable"
                        );
                    }
                    let Some(target) = resolve_to_commit(&output_graph, step_idx) else {
                        bail!("References should have at least one parent");
                    };
                    let Step::Pick(Pick { id: to_reference, .. }) = output_graph.step(target)
                    else {
                        bail!("A parent in the output graph is not a pick");
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
                Step::None => {}
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

        history.add_revision(self.graph.indices().map(|index| (index, index)).collect());

        Ok(SuccessfulRebase {
            repo: self.repo,
            initial_references: self.initial_references,
            ref_edits,
            graph: output_graph,
            checkouts: self.checkouts.to_owned(),
            history,
            project_meta: self.project_meta,
            workspace: self.workspace,
            meta: self.meta,
        })
    }
}

/// All step indexes ordered parents-first, so every node is visited only after
/// all of its parents.
fn topological_order(graph: &StepGraph) -> Result<Vec<StepGraphIndex>> {
    let mut remaining_parents = graph
        .indices()
        .map(|index| graph.parents(index).len())
        .collect::<Vec<_>>();
    let mut children: Vec<Vec<StepGraphIndex>> = vec![Vec::new(); graph.len()];
    for index in graph.indices() {
        for parent in graph.parents(index) {
            children[*parent].push(index);
        }
    }

    let mut ready = graph
        .indices()
        .filter(|index| remaining_parents[*index] == 0)
        .collect::<VecDeque<_>>();
    let mut ordered = Vec::with_capacity(graph.len());
    while let Some(index) = ready.pop_front() {
        ordered.push(index);
        for child in &children[index] {
            remaining_parents[*child] -= 1;
            if remaining_parents[*child] == 0 {
                ready.push_back(*child);
            }
        }
    }
    if ordered.len() != graph.len() {
        bail!("BUG: the step graph contains a cycle");
    }
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
    mod topological_order {
        use std::str::FromStr;

        use anyhow::Result;

        use crate::graph_rebase::{
            Step, StepGraph, rebase::topological_order, testing::render_ascii_graph,
        };

        fn pick(graph: &mut StepGraph, byte: u8) -> usize {
            let id =
                gix::ObjectId::from_str(&format!("{byte:040x}")).expect("valid test object id");
            graph.add_node(Step::new_pick(id))
        }

        #[test]
        fn parents_come_before_children() -> Result<()> {
            let mut graph = StepGraph::new();
            let a = pick(&mut graph, 0x10);
            let b = pick(&mut graph, 0x20);
            let c = pick(&mut graph, 0x30);

            *graph.parents_mut(a) = vec![b];
            *graph.parents_mut(b) = vec![c];

            snapbox::assert_data_eq!(
                render_ascii_graph(&graph, |_| None),
                snapbox::str![[r#"
●  0000000
●  0000000
●  0000000
"#]]
            );

            let ordered = topological_order(&graph)?;
            assert_eq!(&ordered, &[c, b, a]);
            Ok(())
        }

        #[test]
        fn merge_parents_come_before_the_merge() -> Result<()> {
            let mut graph = StepGraph::new();
            let merge = pick(&mut graph, 0x10);
            let left = pick(&mut graph, 0x20);
            let right = pick(&mut graph, 0x30);
            let base = pick(&mut graph, 0x40);

            *graph.parents_mut(merge) = vec![left, right];
            *graph.parents_mut(left) = vec![base];
            *graph.parents_mut(right) = vec![base];

            let ordered = topological_order(&graph)?;
            let position = |index: usize| {
                ordered
                    .iter()
                    .position(|candidate| *candidate == index)
                    .expect("every node is ordered")
            };
            assert!(position(base) < position(left));
            assert!(position(base) < position(right));
            assert!(position(left) < position(merge));
            assert!(position(right) < position(merge));
            Ok(())
        }

        #[test]
        fn a_cycle_is_an_error() {
            let mut graph = StepGraph::new();
            let a = pick(&mut graph, 0x10);
            let b = pick(&mut graph, 0x20);
            *graph.parents_mut(a) = vec![b];
            *graph.parents_mut(b) = vec![a];

            assert!(topological_order(&graph).is_err());
        }
    }
}
