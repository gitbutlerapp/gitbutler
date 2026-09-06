//! Disconnect surgery: cutting a range out of its surroundings and healing the wound.
use std::collections::{HashMap, HashSet};

use crate::graph_rebase::commits::{CommitIndex, ParentEntry};
use crate::graph_rebase::ref_ops;
use crate::graph_rebase::store::RefIndex;
use crate::graph_rebase::{EditorIndex, EditorStore, positions};
use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RefMetadata;

use super::ref_entry;
use crate::graph_rebase::Editor;
use crate::graph_rebase::anchor::{Anchor, Cut, Range};
use crate::graph_rebase::mutate::Reconnect;

/// ParentEntry positions captured at one instant (a frame), looked up later against parent arrays
/// that have since shifted. `current` maps a captured position to today's parent number;
/// removals and inserts are noted so later lookups stay aligned. `None` means the named
/// parent entry itself was already removed.
#[derive(Default)]
struct NumberTranslation {
    map: HashMap<CommitIndex, Vec<Option<usize>>>,
}

impl NumberTranslation {
    /// Today's parent number of the parent entry captured as `(source, frame_number)`, or `None` if it was
    /// removed. The identity map is built lazily, so a source must be looked up before any
    /// of its parent numbers mutate.
    fn current(
        &mut self,
        store: &EditorStore,
        source: CommitIndex,
        frame_number: usize,
    ) -> Option<usize> {
        self.map
            .entry(source)
            .or_insert_with(|| (0..store.parent_count(source)).map(Some).collect())
            .get(frame_number)
            .copied()
            .flatten()
    }

    fn note_remove(&mut self, source: CommitIndex, removed: usize) {
        // `current` always precedes a note in both sever loops; the expect keeps that
        // contract loud rather than silently guessing a frame width here.
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for entry in entries.iter_mut() {
            match entry {
                Some(parent_number) if *parent_number == removed => *entry = None,
                Some(parent_number) if *parent_number > removed => *parent_number -= 1,
                _ => {}
            }
        }
    }

    fn note_insert(&mut self, source: CommitIndex, inserted: usize) {
        let entries = self.map.get_mut(&source).expect("looked up before noting");
        for parent_number in entries.iter_mut().flatten() {
            if *parent_number >= inserted {
                *parent_number += 1;
            }
        }
    }
}

/// One severed parent: the parent number its entry occupied at capture time (shifts
/// preserve relative order, so these still sort reconnects), and the parent it pointed at.
#[derive(Clone, Copy)]
struct SeveredParent {
    number: usize,
    parent: CommitIndex,
}

/// The parent phase's product, consumed by every later phase — holding one is what
/// licenses a reconnect, so the phases cannot run out of order.
struct SeveredParents {
    /// In capture order: the order the cut actually severed them.
    severed: Vec<SeveredParent>,
    /// In parent-number order: the order reconnected children adopt.
    by_number: Vec<SeveredParent>,
    /// The top ref of each group a severed entry carried — the interposed parents that
    /// disconnected child refs stack above.
    carried_tops: Vec<RefIndex>,
}

impl SeveredParents {
    fn new(severed: Vec<SeveredParent>, carried_tops: Vec<RefIndex>) -> Self {
        let mut by_number = severed.clone();
        by_number.sort_by_key(|s| s.number);
        Self {
            severed,
            by_number,
            carried_tops,
        }
    }

    /// The commit a rewired reference resolves through: the first severed parent by
    /// parent number.
    fn group_commit(&self) -> Option<CommitIndex> {
        self.by_number.first().map(|s| s.parent)
    }

    /// The first parent the cut severed, in capture order — where dangling refs re-adopt.
    fn first_severed(&self) -> Option<CommitIndex> {
        self.severed.first().map(|s| s.parent)
    }
}

/// Which of the candidate neighbors a [`Cut`] admits, resolved once.
struct CutFilter(Option<HashSet<CommitIndex>>);

impl CutFilter {
    fn from_entries(entries: &Option<Vec<EditorIndex>>) -> Self {
        Self(
            entries
                .as_ref()
                .map(|entries| entries.iter().filter_map(|s| s.as_commit()).collect()),
        )
    }

    fn admits(&self, commit: CommitIndex) -> bool {
        self.0.as_ref().is_none_or(|ids| ids.contains(&commit))
    }

    /// `Cut::All`: no neighbor was named, everything is admitted.
    fn admits_everything(&self) -> bool {
        self.0.is_none()
    }
}

/// The parent phase's subject: the range's parent side, captured before any severing.
struct ParentSever {
    /// The range's parent-most commit.
    commit: CommitIndex,
    /// Its parents, as frame-coordinate `(parent number, parent)` pairs.
    outgoing: Vec<(usize, CommitIndex)>,
    filter: CutFilter,
}

/// The child phase's subject: everything about the range's child side, captured before
/// any severing.
struct ChildSever {
    /// The range's child-most commit.
    commit: CommitIndex,
    /// The entries into it — bounded to the ref group's entering entries when the child
    /// bound was a reference.
    incoming: Vec<ParentEntry>,
    filter: CutFilter,
    /// `Some` when the child bound was a reference: the entries entering its group.
    ref_entries: Option<Vec<ParentEntry>>,
    ref_depth: Option<usize>,
    /// Requested ref children that ride the range onto the severed parents.
    moving_refs: Vec<RefIndex>,
}

/// The compiled request: both phase subjects, and whether the wound heals. Everything
/// ref-flavored in the caller's input has been translated to commit-level facts.
struct DisconnectPlan {
    parent: ParentSever,
    child: ChildSever,
    heal: bool,
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Disconnect a range (child and parent may be the same entry) from its surroundings:
    /// `children` severs parent entries to children of `target.child`, `parents` severs parent entries to
    /// parents of `target.parent`. A [`Cut::Only`] must name direct neighbors or this
    /// errors. [`Reconnect::Heal`] rewires the severed children onto the severed parents;
    /// [`Reconnect::Skip`] leaves them apart — and is required with `parents:
    /// Cut::Nothing`, since healing with no parents severed has nothing to stitch to.
    pub fn disconnect(
        &mut self,
        target: Range,
        children: Cut,
        parents: Cut,
        reconnect: Reconnect,
    ) -> Result<()> {
        let out = self.disconnect_impl(target, children, parents, reconnect);
        self.verified(out)
    }

    // The complexity epicenter of the editor, with its ordering carried by structure:
    // cross-phase, sever_children and readopt_range_danglers consume the SeveredParents
    // only sever_parents can produce; within a phase, sever_entry fuses
    // capture-carried-groups with the removal. The one ordering still guarded by comment
    // lives in parents.rs: a group join is prepared after the insert that renames
    // statements.
    fn disconnect_impl(
        &mut self,
        target: Range,
        children: Cut,
        parents: Cut,
        reconnect: Reconnect,
    ) -> Result<()> {
        let heal = matches!(reconnect, Reconnect::Heal);
        // A single-entry range that is just a reference: it leaves its group (members
        // above close the gap) and gives up its entries — with a heal they stay as plain
        // entries onto the commit, without one they are removed outright.
        if target.child == target.parent && self.store.is_positioned(target.child) {
            self.ensure_mutable_ref(target.child)?;
            ref_ops::unhook_ref(&mut self.store, ref_entry(target.child)?, !heal);
            return Ok(());
        }

        let plan = self.plan_disconnect(target, children, parents, heal)?;
        // One translation spans both sever phases: the overlap case (the range's
        // parent-most sitting directly above the child-most's commit) captures the same
        // parent entry in both frames.
        let mut numbers = NumberTranslation::default();
        let severed = self.sever_parents(plan.parent, &mut numbers)?;
        self.sever_children(plan.child, &severed, &mut numbers, plan.heal);
        self.readopt_range_danglers(&severed);
        Ok(())
    }

    /// Compile the caller's request into the two phase subjects: translate every
    /// ref-flavored input to commit-level facts, and reject invalid cuts up front.
    fn plan_disconnect(
        &self,
        target: Range,
        children: Cut,
        parents: Cut,
        heal: bool,
    ) -> Result<DisconnectPlan> {
        // The child bound's ref context, captured while the store is quiet. A reference
        // range stands for the commit it resolves to: entries are the truth for commits,
        // and the reference's group rides the commit's links as position data. A
        // reference child only owns the entries entering its own group — plain entries
        // into its commit belong to it and stay.
        let child_is_ref = self.store.is_positioned(target.child);
        let child_ref_entries =
            child_is_ref.then(|| positions::entering(&self.store, target.child));
        let child_ref_depth = child_is_ref.then(|| positions::ref_depth(&self.store, target.child));

        // The range bounds as commits.
        let child_commit = self.resolved_commit(self.resolve_bound(target.child))?;
        let parent_commit = self.resolved_commit(self.resolve_bound(target.parent))?;

        // Each Cut resolves to named neighbors; `None` = all of them.
        let children_cut = match children {
            Cut::All => None,
            Cut::Nothing => Some(Vec::new()),
            Cut::Only(children) => Some(self.resolve_cut(children)?),
        };
        let parents_cut = match parents {
            Cut::All => None,
            Cut::Nothing if !heal => Some(Vec::new()),
            Cut::Nothing => {
                return Err(anyhow!(
                    "cutting no parents requires `Reconnect::Skip`: healing stitches the \
                     severed children to the severed parents, and no parents were severed"
                ));
            }
            Cut::Only(parents) => Some(self.resolve_cut(parents)?),
        };

        // Named neighbors that are references stand for the links their positions
        // decorate: a parent reference maps to its resolved commit; a child reference
        // maps to the commit(s) entering its group, and the member itself (with
        // everything above it in its group) rides the range onto the severed parents.
        let parents_cut =
            parents_cut.map(|parents| parents.into_iter().map(|h| self.resolve_bound(h)).collect());
        let mut moving_refs: Vec<RefIndex> = Vec::new();
        let children_cut = children_cut.map(|children| {
            children
                .into_iter()
                .flat_map(|entry| {
                    if self.store.is_positioned(entry) {
                        let entries = positions::entering(&self.store, entry)
                            .into_iter()
                            .map(|ParentEntry { child, .. }| EditorIndex::from(child))
                            .collect::<Vec<_>>();
                        moving_refs.extend(entry.as_ref());
                        entries
                    } else {
                        vec![entry]
                    }
                })
                .collect::<Vec<_>>()
        });

        // The neighbors on each side, as frame coordinates.
        let incoming = self
            .store
            .children_of(EditorIndex::from(child_commit))
            .iter()
            .copied()
            .filter(|entry| {
                child_ref_entries
                    .as_ref()
                    .is_none_or(|entering| entering.contains(entry))
            })
            .collect::<Vec<_>>();
        let outgoing = self
            .store
            .parents(parent_commit)
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();

        // Named cut members must be direct neighbors of the range.
        verify_cut_members(
            parents_cut.as_deref(),
            children_cut.as_deref(),
            &outgoing.iter().map(|(_, parent)| *parent).collect(),
            &incoming.iter().map(|entry| entry.child).collect(),
        )?;

        Ok(DisconnectPlan {
            parent: ParentSever {
                commit: parent_commit,
                outgoing,
                filter: CutFilter::from_entries(&parents_cut),
            },
            child: ChildSever {
                commit: child_commit,
                incoming,
                filter: CutFilter::from_entries(&children_cut),
                ref_entries: child_ref_entries,
                ref_depth: child_ref_depth,
                moving_refs,
            },
            heal,
        })
    }

    /// A bound or cut member that is a reference stands for the commit it resolves to;
    /// commits pass through (and so does an unborn reference, to fail cleanly later).
    fn resolve_bound(&self, entry: EditorIndex) -> EditorIndex {
        match self.store.resolve_to_commit(entry) {
            Some(commit) if EditorIndex::from(commit) != entry => EditorIndex::from(commit),
            _ => entry,
        }
    }

    /// Phase 2 of [`Self::disconnect`]: sever the admitted parents. Groups the removed
    /// entries carried lose them from their entering sets.
    fn sever_parents(
        &mut self,
        subject: ParentSever,
        numbers: &mut NumberTranslation,
    ) -> Result<SeveredParents> {
        let ParentSever {
            commit,
            outgoing,
            filter,
        } = subject;
        let mut severed: Vec<SeveredParent> = Vec::new();
        let mut carried_tops: Vec<RefIndex> = Vec::new();
        for (frame_number, target_parent) in outgoing {
            if !filter.admits(target_parent) {
                continue;
            }
            // The translation resolves the captured name to today's parent number (earlier removals
            // shift parent entries down). Shifts preserve relative order, so the recorded parent numbers
            // still sort the disconnected parents as their captured orders did.
            let parent_number = numbers
                .current(&self.store, commit, frame_number)
                .context("BUG: disconnected parent entry vanished")?;
            let removed = ParentEntry {
                child: commit,
                number: parent_number,
            };
            if let Some(top) = self.sever_entry(removed, target_parent) {
                carried_tops.push(top);
            }
            numbers.note_remove(commit, parent_number);
            severed.push(SeveredParent {
                number: parent_number,
                parent: target_parent,
            });
        }
        Ok(SeveredParents::new(severed, carried_tops))
    }

    /// Phase 3 of [`Self::disconnect`]: sever the admitted children and, when healing,
    /// reconnect them to the severed parents, then move the groups that rode the range
    /// onto the first severed parent.
    fn sever_children(
        &mut self,
        ctx: ChildSever,
        severed: &SeveredParents,
        numbers: &mut NumberTranslation,
        heal: bool,
    ) {
        // A rewired reference resolves through its first (lowest-parent number) severed parent.
        let group_commit = severed.group_commit();
        for ParentEntry {
            child: source_child,
            number: frame_number,
        } in ctx.incoming
        {
            if !ctx.filter.admits(source_child) {
                continue;
            }
            // Earlier removals on the same child shift this parent entry down; the translation resolves the
            // captured name to the parent number the store uses now.
            let Some(parent_number) = numbers.current(&self.store, source_child, frame_number)
            else {
                // The parent loop already removed this parent entry: the range bounds can name
                // overlapping parent entries when the range's parent-most sits directly above
                // the child-most's commit. Only the reconnect still applies.
                if heal {
                    self.reconnect_to_parents(severed, source_child);
                }
                continue;
            };
            let entry = ParentEntry {
                child: source_child,
                number: parent_number,
            };
            if heal
                && ctx.ref_entries.is_none()
                && !severed.by_number.is_empty()
                && positions::enters_group_resolving_to(&self.store, entry, ctx.commit)
            {
                // A parent entry that carried the target's group is a parent entry into the group — it never
                // lost its parent. Fan it out in place: the first disconnected parent takes
                // the parent entry's parent number (the statement keeps its name, so the carried
                // groups follow), the rest are inserted right after.
                let mut targets = severed.by_number.iter().map(|s| s.parent);
                self.store.commits.replace_parent(
                    source_child,
                    parent_number,
                    targets.next().expect("non-empty"),
                );
                for (offset, target) in targets.enumerate() {
                    self.store.commits.insert_parent(
                        source_child,
                        parent_number + 1 + offset,
                        target,
                    );
                    numbers.note_insert(source_child, parent_number + 1 + offset);
                }
                continue;
            }
            // Remove the child parent entry; groups it carried lose it from their derived
            // entering set automatically.
            self.store.remove_parent(source_child, parent_number);
            numbers.note_remove(source_child, parent_number);
            if heal {
                self.reconnect_to_parents(severed, source_child);
            }
        }
        // The target's groups were the interposed direct children of its commit: a full child
        // disconnect rewires them onto the first disconnected parent, entering parent entries preserved. A
        // reference child bound means the range includes that reference and its group
        // at or below its rank — those stay with the range.
        if let Some(landing) = group_commit {
            for moving_ref in &ctx.moving_refs {
                ref_ops::transfer_stack(&mut self.store, *moving_ref, ctx.commit, landing);
            }
        }
        if ctx.filter.admits_everything()
            && let Some(landing) = group_commit
        {
            match &ctx.ref_entries {
                None => {
                    // When the disconnected parent entry carried a group, the interposed parent
                    // was that group's top ref — the child refs stack above it and follow it
                    // through later moves. Step 2 removed the parent entry that used to enter this
                    // group; step 3's reconnect added fresh parent entries into `commit`. The combined
                    // stack now sits behind all of those fresh parent entries (`GroupCarry::All`), which
                    // is also right when `commit` is a merge.
                    let landed = severed.carried_tops.first().is_some_and(|&top| {
                        ref_ops::land_stack_above(&mut self.store, ctx.commit, top, landing)
                    });
                    if !landed {
                        // A worktree's checked-out branch follows the commit its
                        // worktree stands on: it keeps its seat through the move, while
                        // ordinary references stay behind in the lineage (fixed by the
                        // worktree-move regression this exempts; see move_mixed tests).
                        let keep_seated: Vec<_> = self
                            .checkouts
                            .iter()
                            .filter_map(|checkout| match checkout {
                                crate::graph_rebase::Checkout::Worktree { entry, .. } => {
                                    entry.as_ref()
                                }
                                crate::graph_rebase::Checkout::Head { .. } => None,
                            })
                            .collect();
                        ref_ops::reposition_refs_except(
                            &mut self.store,
                            ctx.commit,
                            landing,
                            ref_ops::Carry::Preserve,
                            &keep_seated,
                        );
                    }
                }
                Some(child_bound_entries) => {
                    // The bound and its group at or below its depth stay with the range;
                    // the group slice above it follows the commit move verbatim.
                    ref_ops::carry_stack_above(
                        &mut self.store,
                        ctx.commit,
                        child_bound_entries,
                        ctx.ref_depth.unwrap_or_default(),
                        landing,
                    );
                }
            }
        }
    }

    /// Phase 4 of [`Self::disconnect`]: references whose commit sat on the
    /// now-disconnected range re-point to the range's first disconnected parent — dangling
    /// references follow where the commit's place went; their entering parent entries stay.
    fn readopt_range_danglers(&mut self, severed: &SeveredParents) {
        if let Some(onto) = severed.first_severed() {
            ref_ops::readopt_dangling_refs(&mut self.store, onto);
        }
    }

    /// Remove `entry`, returning the top ref of the group it carried toward `toward`
    /// (the interposed parent disconnected child refs stack above), if any. Capture and
    /// removal are one operation: the capture-before-remove ordering the epicenter
    /// comment used to guard is structural here.
    fn sever_entry(&mut self, entry: ParentEntry, toward: CommitIndex) -> Option<RefIndex> {
        // Groups this entry carried — read against the live parent arrays; removing the
        // entry afterwards drops it from every derived read automatically (no group
        // bookkeeping needed).
        let carried: Vec<_> = self
            .store
            .positioned_refs()
            .filter(|&r| positions::entering(&self.store, r).contains(&entry))
            .collect();
        let top = carried
            .into_iter()
            .filter(|&r| self.store.resolve_to_commit(r) == Some(toward))
            .max_by_key(|&r| (positions::ref_depth(&self.store, r), r));
        self.store.remove_parent(entry.child, entry.number);
        top
    }

    /// Reconnect `child_commit` to the severed parents, appended after its existing
    /// parents in their original relative order.
    fn reconnect_to_parents(&mut self, severed: &SeveredParents, child_commit: CommitIndex) {
        for s in &severed.by_number {
            self.store.commits.push_parent(child_commit, s.parent);
        }
    }

    /// Resolve a [`Cut::Only`] selection to live entries, rejecting the empty set.
    fn resolve_cut(&self, neighbors: Vec<Anchor>) -> Result<Vec<EditorIndex>> {
        if neighbors.is_empty() {
            bail!("`Cut::Only` with no neighbors — use `Cut::Nothing` to sever nothing");
        }
        neighbors
            .into_iter()
            .map(|neighbor| self.resolve_anchor(neighbor))
            .collect()
    }
}

/// Named cut members must be direct neighbors of the range (`None` = all of them,
/// nothing to check).
fn verify_cut_members(
    parents: Option<&[EditorIndex]>,
    children: Option<&[EditorIndex]>,
    available_parents: &HashSet<CommitIndex>,
    available_children: &HashSet<CommitIndex>,
) -> Result<()> {
    for entry in parents.into_iter().flatten() {
        if !entry
            .as_commit()
            .is_some_and(|c| available_parents.contains(&c))
        {
            bail!("a named parent to cut is not a direct parent of the range");
        }
    }
    for entry in children.into_iter().flatten() {
        if !entry
            .as_commit()
            .is_some_and(|c| available_children.contains(&c))
        {
            bail!("a named child to cut is not a direct child of the range");
        }
    }
    Ok(())
}
