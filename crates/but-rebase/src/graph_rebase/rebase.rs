//! Perform the actual rebase operations

use crate::graph_rebase::commits::ParentEntry;
use std::{
    collections::{HashSet, VecDeque},
    fmt::Write as _,
};

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use but_graph::workspace::commit::is_managed_workspace_by_message;
use gix::refs::{
    Target,
    transaction::{Change, LogChange, PreviousValue, RefEdit},
};

use crate::graph_rebase::commits::CommitIndex;
use crate::graph_rebase::store::WsParentKind;
use crate::graph_rebase::{
    CommitSpec, Editor, EditorStore, RebasedEditor,
    cherry_pick::{CherryPickOutcome, cherry_pick, merge_base},
    util::collect_ordered_parents_with_indices,
};

/// The emitted parent positions of a managed workspace merge that are not writable as real
/// parents: a non-faithful position whose commit is identical to an earlier one's, or an
/// ancestor of any other parent — the empty-lane rule (git cannot repeat a parent, and merge
/// semantics drop a parent fully contained in another) applied to whatever surgery
/// produced. Faithful positions — real on-disk parents whose parent entry is untouched — are never
/// dropped: a rebase that changed nothing must reproduce the merge verbatim, however
/// the on-disk parents relate to each other. Equal ids keep the first position; ancestry
/// drops the contained one regardless of order (containment is transitive, so testing
/// against every other parent is exact).
fn contained_parents(
    repo: &gix::Repository,
    ontos: &[gix::ObjectId],
    faithful: impl Fn(usize) -> bool,
) -> Result<HashSet<usize>> {
    let mut contained = HashSet::new();
    for i in 0..ontos.len() {
        if faithful(i) {
            continue;
        }
        for j in 0..ontos.len() {
            if i == j {
                continue;
            }
            let is_contained = if ontos[i] == ontos[j] {
                j < i
            } else {
                merge_base(repo, ontos[i], ontos[j])? == Some(ontos[i])
            };
            if is_contained {
                contained.insert(i);
                break;
            }
        }
    }
    Ok(contained)
}

/// The product rule for managed workspace merges, applied at write time: what gets
/// written is the merge's real parents. Applies only when the merge's parent entries are
/// graph-backed — a preserved-parents commit carries its onto-commits ready-made and is
/// left alone.
///
/// An empty branch is a declared lane, not ancestry — git cannot repeat a parent, and
/// order-dependent merge resolution can drop one contained in another — so a minted
/// entry is never written, however its commit relates to the others. Faithful entries
/// (ingested real parents, parent entry untouched) are written verbatim, which keeps a
/// no-op rebase byte-identical with the product writer instead of "materializing"
/// lanes. Only what surgery produced is left to judge by containment.
fn retain_real_ws_parents(
    store: &EditorStore,
    repo: &gix::Repository,
    commit_idx: CommitIndex,
    spec: &CommitSpec,
    onto_indices: &[Option<usize>],
    ontos: &mut Vec<gix::ObjectId>,
) -> Result<()> {
    let is_managed_ws_commit = spec.preserved_parents.is_none()
        && repo
            .find_commit(spec.id)
            .ok()
            .and_then(|commit| {
                commit
                    .message_raw()
                    .ok()
                    .map(is_managed_workspace_by_message)
            })
            .unwrap_or(false);
    if !is_managed_ws_commit {
        return Ok(());
    }
    // Each onto carries the parent index it came from, so a verdict lands on the entry
    // it was made about — the emitted order skips and flattens, and a position is not
    // a parent index.
    let by_index = store.ws_parent_kinds(commit_idx);
    let kind = |pos: usize| -> WsParentKind {
        onto_indices
            .get(pos)
            .copied()
            .flatten()
            .and_then(|index| by_index.get(index).copied())
            .unwrap_or(WsParentKind::Surgical)
    };
    let mut drop: HashSet<usize> = (0..ontos.len())
        .filter(|&pos| kind(pos) == WsParentKind::Minted)
        .collect();
    drop.extend(contained_parents(repo, ontos, |pos| {
        kind(pos) == WsParentKind::Faithful
    })?);
    // The merge's link to history is not a lane. Every lane can be empty — then none
    // contributes a leg — but the workspace commit still rests on its base, and the
    // entry each lane's anchor names is that resting point. Dropping the last one would
    // sever the workspace from the history it is a view of and leave a root commit, so
    // the floor survives: the merge always keeps a parent.
    if drop.len() == ontos.len()
        && let Some(floor) = drop.iter().copied().min()
    {
        drop.remove(&floor);
    }
    if !drop.is_empty() {
        let mut pos = 0usize;
        ontos.retain(|_| {
            let keep = !drop.contains(&pos);
            pos += 1;
            keep
        });
    }
    Ok(())
}

impl<'meta, M: RefMetadata> Editor<'meta, M> {
    /// Perform the rebase in place: each mutable commit's commit id is rewritten where it
    /// stands, in dependency order, so a commit's parents already hold rebased ids by the
    /// time it is cherry-picked. Entry ids never change — parent arrays, positions, groups, and every
    /// outstanding index stay valid across the rebase.
    #[tracing::instrument(level = "debug", skip_all, err(Debug))]
    pub fn rebase(self) -> Result<RebasedEditor<'meta, M>> {
        crate::graph_rebase::positions::assert_positions_total(&self.store)?;

        let Editor {
            store,
            checkouts,
            repo,
            history,
            project_meta,
            meta,
        } = self;
        let (mut store, mut history) = (store, history);

        // Every tip (an entry with no children) seeds the traversal so every commit is
        // visited — immutable commits and tombstones are left untouched where they stand.
        let rebase_heads = store.commits.tips().collect::<Vec<_>>();
        let to_pick = cherry_pick_order(&store, &rebase_heads)?;

        for commit_idx in to_pick {
            let Some(spec) = store.commits.commit_spec(commit_idx) else {
                // Tombstones have nothing to rewrite.
                continue;
            };
            if !spec.mutable {
                // Immutable commits keep their id: no cherry-pick to run, nothing to record
                // in the history mapping.
                continue;
            }

            // Only resolve the graph parents when we actually need them — a commit with
            // `preserved_parents` already carries its onto-commits.
            let onto_commits: Vec<CommitIndex>;
            let mut onto_indices: Vec<Option<usize>> = Vec::new();
            let mut ontos = match spec.preserved_parents.clone() {
                Some(ontos) => ontos,
                None => {
                    let ordered = collect_ordered_parents_with_indices(&store, commit_idx);
                    onto_indices = ordered.iter().map(|&(_, index)| index).collect();
                    onto_commits = ordered.into_iter().map(|(commit, _)| commit).collect();
                    onto_commits
                        .iter()
                        .map(|&idx| {
                            store
                                .commit_id(idx)
                                .context("BUG: ordered parents must be commits")
                        })
                        .collect::<Result<Vec<_>>>()?
                }
            };

            retain_real_ws_parents(&store, &repo, commit_idx, &spec, &onto_indices, &mut ontos)?;

            let outcome = cherry_pick(
                &repo,
                spec.id,
                &ontos,
                spec.pick_mode,
                spec.tree_merge_mode,
                spec.sign_commit,
            )?;

            if matches!(outcome, CherryPickOutcome::ConflictedCommit(_)) && !spec.conflictable {
                bail!(
                    "Commit {} was marked as not conflictable, but resulted in a conflicted state",
                    spec.id
                );
            }

            match outcome {
                CherryPickOutcome::Commit(new_id)
                | CherryPickOutcome::ConflictedCommit(new_id)
                | CherryPickOutcome::Identity(new_id) => {
                    store.commits.set_commit_id(commit_idx, new_id);
                    if !spec.exclude_from_tracking {
                        history.update_mapping(spec.id, new_id);
                    }
                }
                CherryPickOutcome::FailedToMergeBases {
                    base_merge_failed,
                    bases,
                    onto_merge_failed,
                    ontos,
                } => {
                    // Exit early - the rebase failed because it encountered a commit it couldn't commit
                    bail!(format_base_merge_error(
                        spec.id,
                        base_merge_failed,
                        bases,
                        onto_merge_failed,
                        ontos
                    ));
                }
            }
        }

        // References need no rewrite at all — their position's `on` entry now carries the
        // rebased id. All that remains is deriving the ref transaction.
        let ref_edits = derive_ref_edits(&store, &repo)?;

        Ok(RebasedEditor {
            editor: Editor {
                store,
                checkouts,
                repo,
                history,
                project_meta,
                meta,
            },
            ref_edits,
        })
    }
}

/// The second half of the replay: no reference is rewritten — a ref's target is derived
/// from its position, after every id under it was rewritten in place. Every live,
/// mutable, positioned reference moves to the id its position resolves to (guarded by
/// `MustExistAndMatch`, so a concurrent move fails the transaction loudly), and every
/// mutable reference that existed at creation but is no longer stated is deleted.
fn derive_ref_edits(store: &EditorStore, repo: &gix::Repository) -> Result<Vec<RefEdit>> {
    let mut ref_edits = vec![];
    let mut unchanged_references = vec![];

    for ref_idx in store.ref_indices() {
        let record = store
            .state_of(ref_idx.into())
            .expect("ref_indices only yields references");
        if !record.live || !record.mutable || !store.is_positioned(ref_idx) {
            // Dead records keep their retained name and position; immutable references
            // keep their record and position but are never moved, created, or deleted.
            continue;
        }
        let refname = record.refname.clone();
        let resolved_commit = store
            .resolve_to_commit(ref_idx)
            .context("References should resolve to a commit")?;
        let to_reference = match store.commit_id(resolved_commit) {
            Some(id) => id,
            None => bail!("A reference's position does not resolve to a commit"),
        };

        let expected = match repo.try_find_reference(&refname)? {
            Some(reference) => match reference.target() {
                gix::refs::TargetRef::Object(id) if id == to_reference => {
                    unchanged_references.push(refname);
                    continue;
                }
                target @ gix::refs::TargetRef::Object(_) => {
                    PreviousValue::MustExistAndMatch(target.into())
                }
                gix::refs::TargetRef::Symbolic(name) => {
                    bail!("Attempted to update the symbolic reference {name}");
                }
            },
            None => PreviousValue::MustNotExist,
        };
        ref_edits.push(RefEdit {
            name: refname,
            change: Change::Update {
                log: LogChange::default(),
                expected,
                new: Target::Object(to_reference),
            },
            deref: false,
        });
    }

    // Find deleted references. The deletion universe is the mutable references that
    // existed at creation, straight from the ref table.
    let creation_refs: Vec<gix::refs::FullName> = store.creation_references().cloned().collect();
    for reference in creation_refs.iter() {
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
    Ok(ref_edits)
}

/// Creates a list of step indicies ordered in the dependency order.
///
/// We do this by first doing a breadth-first traversal down from the heads
/// (usually the childless tips of the commit half) in order
/// to determine which steps are reachable, and what the bottom most steps are.
///
/// Then, we do a second traversal up from those bottom most
/// steps.
///
/// This second traversal ensures that all the parents of any given entry have
/// been seen, before traversing it.
fn cherry_pick_order(store: &EditorStore, heads: &[CommitIndex]) -> Result<VecDeque<CommitIndex>> {
    // References take no part in the commit order (no parent entries, replayed separately) —
    // the head type keeps them out. commits and tombstones must all be traversed,
    // or their subtree is orphaned.
    let mut heads: Vec<CommitIndex> = heads.to_vec();
    let mut seen = heads.iter().cloned().collect::<HashSet<CommitIndex>>();
    // Reachable entries with no parents.
    let mut bases = VecDeque::new();

    while let Some(head) = heads.pop() {
        let parents = store.parents(head);

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
        for &ParentEntry { child: s, .. } in store.children_of(base) {
            // We only want to queue entries for traversing that have had all of their parents traversed.
            let all_parents_seen = store.parents(s).iter().all(|t| retraversed.contains(t));
            if all_parents_seen && seen.contains(&s) && retraversed.insert(s) {
                bases.push_back(s);
                ordered.push_back(s);
            };
        }
    }

    // A cycle strands its members: reached on the way down, never re-orderable on the way
    // up because each waits on the other. Rebasing a partial order would silently rewrite
    // some commits and leave the rest pointing at the ids they replaced.
    if ordered.len() != seen.len() {
        bail!("BUG: Rebase editor store contains a cycle");
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
    mod cherry_pick_order {
        use std::str::FromStr;

        use anyhow::Result;

        use crate::graph_rebase::{
            CommitSpec, EditorStore, rebase::cherry_pick_order, testing::render_ascii_graph,
        };

        #[test]
        fn basic_scenario() -> Result<()> {
            let mut store = EditorStore::default();
            let a = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1000000000000000000000000000000000000000",
                )?));
            let b = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "2000000000000000000000000000000000000000",
                )?));
            let c = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "3000000000000000000000000000000000000000",
                )?));

            store.commits.push_parent(a, b);
            store.commits.push_parent(b, c);

            snapbox::assert_data_eq!(
                render_ascii_graph(&store, |_| None),
                snapbox::str![[r#"
●  1000000
●  2000000
●  3000000
"#]]
            );

            let ordered_from_a =
                cherry_pick_order(&store, &store.commits.tips().collect::<Vec<_>>())?;
            assert_eq!(&ordered_from_a, &[c, b, a]);

            Ok(())
        }

        #[test]
        fn incomplete_order_from_a_cycle_is_rejected() -> Result<()> {
            let mut store = EditorStore::default();
            let commit = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1000000000000000000000000000000000000000",
                )?));
            store.commits.push_parent(commit, commit);

            // A self-parent leaves no tip at all, so the walk starts from nothing and
            // orders nothing — the partial result the guard exists to reject.
            let err = cherry_pick_order(&store, &[commit])
                .expect_err("a cyclic store must not produce a partial rebase mapping");
            assert_eq!(err.to_string(), "BUG: Rebase editor store contains a cycle");

            Ok(())
        }

        #[test]
        fn complex_scenario() -> Result<()> {
            let mut store = EditorStore::default();
            let a = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1000000000000000000000000000000000000000",
                )?));
            let b = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "2000000000000000000000000000000000000000",
                )?));
            let c = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "3000000000000000000000000000000000000000",
                )?));
            let d = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "4000000000000000000000000000000000000000",
                )?));
            let e = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "5000000000000000000000000000000000000000",
                )?));
            let f = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "6000000000000000000000000000000000000000",
                )?));
            let g = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "7000000000000000000000000000000000000000",
                )?));
            let h = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "8000000000000000000000000000000000000000",
                )?));
            let i = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "9000000000000000000000000000000000000000",
                )?));
            let j = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1100000000000000000000000000000000000000",
                )?));

            store.commits.push_parent(a, b);
            store.commits.push_parent(b, c);
            store.commits.push_parent(c, d);
            store.commits.push_parent(d, e);

            store.commits.push_parent(f, g);
            store.commits.push_parent(g, c);

            store.commits.push_parent(h, d);

            store.commits.push_parent(i, j);

            snapbox::assert_data_eq!(
                render_ascii_graph(&store, |_| None),
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

            let ordered_from_a = cherry_pick_order(&store, &[f, h])?;
            assert_eq!(&ordered_from_a, &[e, d, c, h, g, f]);

            Ok(())
        }

        #[test]
        fn merge_scenario() -> Result<()> {
            let mut store = EditorStore::default();
            let a = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1000000000000000000000000000000000000000",
                )?));
            let b = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "2000000000000000000000000000000000000000",
                )?));
            let c = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "3000000000000000000000000000000000000000",
                )?));
            let d = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "4000000000000000000000000000000000000000",
                )?));
            let e = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "5000000000000000000000000000000000000000",
                )?));

            store.commits.push_parent(a, b);
            store.commits.push_parent(b, c);

            store.commits.push_parent(a, d);
            store.commits.push_parent(d, e);
            store.commits.push_parent(e, b);

            snapbox::assert_data_eq!(
                render_ascii_graph(&store, |_| None),
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

            let ordered_from_a =
                cherry_pick_order(&store, &store.commits.tips().collect::<Vec<_>>())?;
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }

        #[test]
        fn merge_flipped_scenario() -> Result<()> {
            let mut store = EditorStore::default();
            let a = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "1000000000000000000000000000000000000000",
                )?));
            let b = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "2000000000000000000000000000000000000000",
                )?));
            let c = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "3000000000000000000000000000000000000000",
                )?));
            let d = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "4000000000000000000000000000000000000000",
                )?));
            let e = store
                .commits
                .add_commit(CommitSpec::new(gix::ObjectId::from_str(
                    "5000000000000000000000000000000000000000",
                )?));

            store.commits.push_parent(a, d);
            store.commits.push_parent(d, e);
            store.commits.push_parent(e, b);
            store.commits.push_parent(b, c);

            store.commits.push_parent(a, b);

            snapbox::assert_data_eq!(
                render_ascii_graph(&store, |_| None),
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

            let ordered_from_a =
                cherry_pick_order(&store, &store.commits.tips().collect::<Vec<_>>())?;
            assert_eq!(&ordered_from_a, &[c, b, e, d, a]);

            Ok(())
        }
    }
}
