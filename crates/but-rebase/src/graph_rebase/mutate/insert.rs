//! Insertion: placing ranges, commits, and references relative to an anchor.

use crate::graph_rebase::commits::{CommitIndex, ParentEntry};
use crate::graph_rebase::ref_ops;
use crate::graph_rebase::ref_ops::{RefPlace, SplitBoundary};
use crate::graph_rebase::store::RefIndex;
use crate::graph_rebase::{EditorIndex, positions};
use anyhow::{Context as _, Result, bail};
use but_core::RefMetadata;

use super::{commit_entry, ref_entry};
use crate::graph_rebase::anchor::{Anchor, Connect, Range};
use crate::graph_rebase::mutate::InsertSide;
use crate::graph_rebase::{CommitSpec, Editor};

/// What a below-insertion wires onto the range's parent-most, gathered before any
/// re-pointing disturbs the store.
#[derive(Default)]
struct BelowParents {
    /// The parents to add, in order.
    commits: Vec<CommitIndex>,
    /// Per named reference parent: its index into `commits`, and the ref whose group
    /// joins once that parent's final number is known.
    ref_joins: Vec<(usize, RefIndex)>,
    /// Drained parent entry ids awaiting re-statement onto the range's parent-most.
    moved_entry_ids: Vec<crate::graph_rebase::commits::ParentEntryId>,
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Returns the parent number orders assigned to `new_parent_nodes`, in the given order.
    /// Existing parent entries that get renumbered carry their group entries along.
    /// Reparented parents prepend: they define the first-parent line, and existing
    /// parents stay attached after them as merge-side parents.
    fn add_parents(
        &mut self,
        child_commit: CommitIndex,
        new_parent_nodes: impl IntoIterator<Item = CommitIndex>,
    ) -> Vec<usize> {
        new_parent_nodes
            .into_iter()
            .enumerate()
            .map(|(parent_number, parent_commit)| {
                self.store
                    .commits
                    .insert_parent(child_commit, parent_number, parent_commit);
                parent_number
            })
            .collect()
    }

    /// Insert a range (parent-most `range.parent` up to child-most `range.child`) relative
    /// to `target`. [`Connect::Splice`] moves the target's children ([`InsertSide::Above`])
    /// or parents ([`InsertSide::Below`]) onto the range; [`Connect::Only`] wires exactly
    /// the named entries instead. Reparented parents prepend before
    /// existing parents on the range's parent-most entry.
    pub(crate) fn insert_range(
        &mut self,
        target: impl Into<Anchor>,
        range: Range,
        side: InsertSide,
        connect: Connect,
    ) -> Result<()> {
        let out = self.insert_range_impl(target, range, side, connect);
        self.verified(out)
    }

    fn insert_range_impl(
        &mut self,
        target: impl Into<Anchor>,
        range: Range,
        side: InsertSide,
        connect: Connect,
    ) -> Result<()> {
        let entries_to_connect = match connect {
            Connect::Splice => None,
            Connect::Only(entries) if entries.is_empty() => bail!(
                "`Connect::Only` with no entries — use `Connect::Splice` to adopt the target's neighbors"
            ),
            Connect::Only(entries) => Some(entries),
        };
        let Range { child, parent } = range;
        let target = self.resolve_anchor(target)?;

        // An empty range — a lone reference — is pure position data.
        if child == parent && self.store.is_positioned(child) {
            return self.place_reference(child, target, side, entries_to_connect);
        }

        match side {
            InsertSide::Above => self.insert_range_above(target, child, parent, entries_to_connect),
            InsertSide::Below => self.insert_range_below(target, child, parent, entries_to_connect),
        }
    }

    /// Place the lone reference of an empty range into the target's group at the place
    /// `side` names; any entries to connect become the parent entries entering through it.
    fn place_reference(
        &mut self,
        subject: EditorIndex,
        target: EditorIndex,
        side: InsertSide,
        entries_to_connect: Option<Vec<Anchor>>,
    ) -> Result<()> {
        self.ensure_mutable_ref(subject)?;
        let place = match (side, self.store.is_positioned(target)) {
            (InsertSide::Above, true) => RefPlace::Above(ref_entry(target)?),
            (InsertSide::Below, true) => RefPlace::Below(ref_entry(target)?),
            (InsertSide::Above, false) => RefPlace::Bottom(commit_entry(target)?),
            (InsertSide::Below, false) => {
                // On top of the group the target commit's first-parent entry enters.
                let target_commit = commit_entry(target)?;
                let first_parent = self
                    .store
                    .parents(target_commit)
                    .first()
                    .copied()
                    .context("Cannot insert a reference below a parentless commit")?;
                RefPlace::GroupTop {
                    commit: first_parent,
                    entry: ParentEntry {
                        child: target_commit,
                        number: 0,
                    },
                }
            }
        };
        ref_ops::move_ref(&mut self.store, ref_entry(subject)?, place);
        if let Some(entries_to_connect) = entries_to_connect {
            self.add_parent_to_each(&entries_to_connect, subject)?;
        }
        Ok(())
    }

    /// The [`InsertSide::Above`] arm of [`Self::insert_range`]: the range's child-most
    /// takes over the target's children (or gains `entries_to_connect` as children), and the
    /// target connects under the range's parent-most.
    fn insert_range_above(
        &mut self,
        target: EditorIndex,
        child: EditorIndex,
        parent: EditorIndex,
        entries_to_connect: Option<Vec<Anchor>>,
    ) -> Result<()> {
        if let Some(entries_to_connect) = entries_to_connect {
            // Connect the given entries as children of the range's child-most.
            // `add_parent` appends after existing parents and handles a reference
            // child-most (the parent entry joins its group).
            self.add_parent_to_each(&entries_to_connect, child)?;
        } else if self.store.positioned_on(target).is_some() {
            // Above a reference: the range interposes into the target's group.
            return self.splice_above_reference(target, child, parent);
        } else {
            self.splice_above_commit(target, child)?;
        }
        self.connect_target_under(parent, target)
    }

    /// Splice the range above reference `target`: split the group there — members above
    /// move onto the range's child-most commit, the reference and members below are now
    /// entered through its parent-most commit.
    fn splice_above_reference(
        &mut self,
        target: EditorIndex,
        child: EditorIndex,
        parent: EditorIndex,
    ) -> Result<()> {
        let child_commit = self
            .store
            .resolve_to_commit(child)
            .context("Range child should resolve to a commit")?;
        let parent_commit = self
            .store
            .resolve_to_commit(parent)
            .context("Range parent should resolve to a commit")?;
        let (split, on_commit) =
            self.interpose_into_group(target, child_commit, SplitBoundary::Above)?;
        // Connect the parent-most entry to the reference's commit; a reference
        // parent-most (an empty range) re-points instead of gaining parent entries. The
        // target reference and members below are now entered through that parent entry.
        let entry_number = if self.store.is_positioned(parent) {
            let on_entry = EditorIndex::from(on_commit);
            self.insert_parent(parent, on_entry, 0)?;
            // The range is positioned data: the split-off lower group is
            // entered through the range's group, which ends at the commit.
            0
        } else {
            let orders = self.add_parents(commit_entry(parent)?, [on_commit]);
            orders.first().copied().unwrap_or(0)
        };
        ref_ops::settle_group_lower(
            &mut self.store,
            &split.lower,
            ParentEntry {
                child: parent_commit,
                number: entry_number,
            },
        );
        Ok(())
    }

    /// The interposition dance, shared by every operation that puts a commit into a
    /// reference's group: capture the entering entries, split the group at `boundary`
    /// (upper members re-key onto `landing`), redirect the captured entries onto
    /// `landing`, and re-hang the split boundary onto any chain already standing there.
    ///
    /// Capture-before-wiring is structural: this runs before the caller creates any
    /// fresh entry into `landing`, so the redirect can never re-point a fresh entry onto
    /// itself (the self-loop the old duplicated dances guarded by comment). The caller
    /// wires `landing`'s own parents afterward and settles `split.lower` onto the entry
    /// that then enters it. Returns the split and the commit the group stood on.
    fn interpose_into_group(
        &mut self,
        target: EditorIndex,
        landing: CommitIndex,
        boundary: SplitBoundary,
    ) -> Result<(crate::graph_rebase::ref_ops::GroupSplit, CommitIndex)> {
        let on = self
            .store
            .positioned_on(target)
            .expect("caller checked the target is positioned");
        let on_commit = self.resolved_commit(on)?;
        let entering = positions::entering(&self.store, target);
        let split = ref_ops::split_group(&mut self.store, ref_entry(target)?, boundary, landing);
        // The group's parent entries now enter through `landing` — directly when the
        // target topped its group, or through the members that rode above (they carry
        // their statements verbatim, so the graph entries must follow them or the
        // interposed commit is silently bypassed).
        ref_ops::redirect_entries(&mut self.store, &entering, on_commit, landing);
        ref_ops::rehang_split_boundary(&mut self.store, &split, landing);
        Ok((split, on_commit))
    }

    /// Splice the range above commit `target`: the range's child-most takes the target's
    /// place in each child's parent array — the parent number, and any statement on it,
    /// untouched — and the target's groups slide under the range.
    fn splice_above_commit(&mut self, target: EditorIndex, child: EditorIndex) -> Result<()> {
        self.store
            .commits
            .redirect_children(commit_entry(target)?, commit_entry(child)?);
        // Refs sitting on the target move up onto the range's child-most commit.
        if let Some(child_commit) = self.store.resolve_to_commit(child) {
            ref_ops::reposition_refs(
                &mut self.store,
                commit_entry(target)?,
                child_commit,
                ref_ops::Carry::Reclassify,
            );
        }
        Ok(())
    }

    /// Connect `target` under the range's parent-most entry per the ordering policy. A
    /// reference target stands for its commit, with the new parent entry entering its
    /// group; a reference parent-most (an empty range) has no parent entries — it
    /// re-points onto the target instead.
    fn connect_target_under(&mut self, parent: EditorIndex, target: EditorIndex) -> Result<()> {
        if self.store.is_positioned(parent) {
            self.insert_parent(parent, target, 0)?;
        } else {
            let connect_to = match self.store.positioned_on(target) {
                Some(on) => self.resolved_commit(on)?,
                None => commit_entry(target)?,
            };
            let join = (EditorIndex::from(connect_to) != target)
                .then(|| ref_entry(target))
                .transpose()?
                .map(|r| positions::prepare_group_join(&self.store, r));
            let parent_commit = commit_entry(parent)?;
            let orders = self.add_parents(parent_commit, [connect_to]);
            if let (Some(join), Some(order)) = (join, orders.first()) {
                ref_ops::apply_group_join(
                    &mut self.store,
                    &join,
                    ParentEntry {
                        child: parent_commit,
                        number: *order,
                    },
                );
            }
        }
        Ok(())
    }

    /// The [`InsertSide::Below`] arm of [`Self::insert_range`]: the target's parents
    /// (or `entries_to_connect`) become parents of the range's parent-most, and the range's
    /// child-most connects under the target.
    fn insert_range_below(
        &mut self,
        target: EditorIndex,
        child: EditorIndex,
        parent: EditorIndex,
        entries_to_connect: Option<Vec<Anchor>>,
    ) -> Result<()> {
        let gathered = self.gather_below_parents(target, entries_to_connect)?;

        // Ordering matters. A reference target connects before the range gains its own
        // downward parent entry: re-pointing drags its entering parent entries along, and a pre-existing
        // fresh parent entry (a `GroupCarry::All` ref sees every parent entry into its commit) would be dragged
        // too and self-loop the range. A plain target connects after: its orphaned parent entry
        // statements must first be renamed onto the range's parent-most, or the fresh
        // parent entry at parent number 0 would take their names. The gather ran up front,
        // so it still names the pre-re-point target position.
        let target_is_ref = self.store.is_positioned(target);
        if target_is_ref {
            // A reference target re-points onto the range's child-most (standing for its
            // commit when that too is a reference); no parent entry is created.
            self.insert_parent(target, child, 0)?;
        }

        self.wire_parents_below(parent, gathered)?;

        if !target_is_ref {
            // A plain target keeps its existing parents in front; the range appends.
            let parent_number = self.store.parent_count(target);
            self.insert_parent(target, child, parent_number)?;
        }
        Ok(())
    }

    /// What the range's parent-most connects to, gathered before any re-pointing: from
    /// the named entries, from a reference target's one downward link, or by draining a
    /// plain target's parents.
    fn gather_below_parents(
        &mut self,
        target: EditorIndex,
        entries_to_connect: Option<Vec<Anchor>>,
    ) -> Result<BelowParents> {
        let mut gathered = BelowParents::default();
        if let Some(entries_to_connect) = entries_to_connect {
            for any_handle in &entries_to_connect {
                let entry = self.resolve_anchor(any_handle.clone())?;
                // A reference parent: the commit parent entry goes to its commit and the parent entry
                // joins its group once the final parent number is known.
                if self.store.is_positioned(entry) {
                    let commit = self.resolved_commit(entry)?;
                    gathered
                        .ref_joins
                        .push((gathered.commits.len(), ref_entry(entry)?));
                    gathered.commits.push(commit);
                } else {
                    gathered.commits.push(commit_entry(entry)?);
                }
            }
        } else if let Some(t_on) = self.store.positioned_on(target) {
            // A reference target's one downward link is its commit: the range goes
            // between the reference and it. The reference's own re-pointing onto the
            // range happens in the connect step of the caller.
            gathered.commits.push(self.resolved_commit(t_on)?);
        } else {
            // Statements keep naming the drained parent entry ids until the restate moves
            // them onto the range's parent-most.
            let drained = self.store.commits.drain_parents(commit_entry(target)?);
            gathered.moved_entry_ids = drained.iter().map(|&(_, id)| id).collect();
            gathered.commits = drained.into_iter().map(|(parent, _)| parent).collect();
        }
        Ok(gathered)
    }

    /// Wire the gathered parents onto the range's parent-most: a reference parent-most
    /// (an empty range) re-points onto the first of them instead of gaining parent
    /// entries; a commit parent-most gains them per the ordering policy, ref groups join
    /// at their final parent numbers, and drained statements re-state onto the fresh
    /// entries.
    fn wire_parents_below(&mut self, parent: EditorIndex, gathered: BelowParents) -> Result<()> {
        let BelowParents {
            commits,
            ref_joins,
            moved_entry_ids,
        } = gathered;
        if self.store.is_positioned(parent) {
            if let Some(first) = commits.first() {
                let first = EditorIndex::from(*first);
                self.insert_parent(parent, first, 0)?;
            }
            return Ok(());
        }
        let joins: Vec<_> = ref_joins
            .iter()
            .map(|(k, ref_node)| (*k, positions::prepare_group_join(&self.store, *ref_node)))
            .collect();
        let parent_commit = commit_entry(parent)?;
        let new_orders = self.add_parents(parent_commit, commits);
        for (k, join) in &joins {
            if let Some(order) = new_orders.get(*k) {
                ref_ops::apply_group_join(
                    &mut self.store,
                    join,
                    ParentEntry {
                        child: parent_commit,
                        number: *order,
                    },
                );
            }
        }
        // Groups those parent entries carried are now entered through the range's
        // parent-most commit. Only a plain (drained) target has moved parent entries; a
        // reference target contributes none.
        if !moved_entry_ids.is_empty() {
            let restates: Vec<_> = moved_entry_ids
                .iter()
                .zip(new_orders)
                .filter_map(|(&old, new)| {
                    Some((old, self.store.commits.entry_id_at(parent_commit, new)?))
                })
                .collect();
            self.store.restate_entries(&restates);
        }
        Ok(())
    }

    /// [`Self::add_parent`] from each of `entries` onto `parent`.
    fn add_parent_to_each(&mut self, entries: &[Anchor], parent: EditorIndex) -> Result<()> {
        for any_handle in entries {
            let entry = self.resolve_anchor(any_handle.clone())?;
            self.add_parent(entry, parent)?;
        }
        Ok(())
    }

    /// Interpose a new commit between `target` (a positioned reference) and its commit:
    /// the shared dance via [`Self::interpose_into_group`], with the new commit as the
    /// landing — it then gains the group's old commit as its sole parent, and the
    /// split-off lower slice settles onto that entry.
    fn split_group_with_commit(
        &mut self,
        target: EditorIndex,
        new: CommitSpec,
        boundary: SplitBoundary,
    ) -> Result<EditorIndex> {
        // Splitting AT the reference moves the reference itself onto the new commit; an
        // immutable ref must refuse here — at the operation that would move it — so no
        // caller arm can forget the guard (one did once).
        if matches!(boundary, SplitBoundary::At) {
            self.ensure_mutable_ref(target)?;
        }
        let new_idx = self.store.commits.add_commit(new);
        let (split, on_commit) = self.interpose_into_group(target, new_idx, boundary)?;
        self.store.commits.push_parent(new_idx, on_commit);
        ref_ops::settle_group_lower(
            &mut self.store,
            &split.lower,
            ParentEntry {
                child: new_idx,
                number: 0,
            },
        );
        Ok(EditorIndex::from(new_idx))
    }

    /// Insert a new commit relative to `target` — a commit or a reference anchor alike —
    /// returning its index (see [`InsertSide`] for how parent entries rewire).
    pub fn insert_commit(
        &mut self,
        target: impl Into<Anchor>,
        spec: CommitSpec,
        side: InsertSide,
    ) -> Result<CommitIndex> {
        let out = self.insert_commit_impl(target, spec, side);
        self.verified(out)
    }

    fn insert_commit_impl(
        &mut self,
        target: impl Into<Anchor>,
        spec: CommitSpec,
        side: InsertSide,
    ) -> Result<CommitIndex> {
        let target = self.resolve_anchor(target)?;
        let target_positioned = self.store.is_positioned(target);
        let inner = match (side, target_positioned) {
            (InsertSide::Above, false) => {
                // Above a commit: the interposed entry slides under the commit's groups — its
                // children rewire to the new entry with parent numbers preserved (so stored group
                // parent entries stay valid) and every ref sitting on it moves up.
                let new_idx = self.store.commits.add_commit(spec);
                let target_commit = commit_entry(target)?;
                self.store.commits.redirect_children(target_commit, new_idx);
                self.store.commits.push_parent(new_idx, target_commit);
                ref_ops::reposition_refs(
                    &mut self.store,
                    target_commit,
                    new_idx,
                    ref_ops::Carry::Preserve,
                );
                EditorIndex::from(new_idx)
            }
            (InsertSide::Above, true) => {
                // A commit above a reference splits the group at that reference: members above
                // move onto the new commit, the reference and members below are now entered
                // through it.
                self.split_group_with_commit(target, spec, SplitBoundary::Above)?
            }
            (InsertSide::Below, false) => {
                // Below a commit: the commit's whole parent array moves onto the new entry with
                // parent numbers preserved, so groups carried by those parent entries follow the rename.
                let target_commit = commit_entry(target)?;
                let new_idx = self.store.commits.add_commit(spec);
                self.store
                    .commits
                    .transplant_parents(target_commit, new_idx);
                self.store.commits.push_parent(target_commit, new_idx);
                EditorIndex::from(new_idx)
            }
            (InsertSide::Below, true) => {
                // A commit below a reference splits the group there: the reference and members
                // above re-point onto the new commit, members below are entered through it.
                self.split_group_with_commit(target, spec, SplitBoundary::At)?
            }
        };
        inner
            .as_commit()
            .context("insertion always yields a commit entry")
    }

    /// Insert a new mutable reference named `refname` relative to `target`, returning
    /// its index (see [`InsertSide`] for how it takes a position).
    pub fn insert_reference(
        &mut self,
        target: impl Into<Anchor>,
        refname: gix::refs::FullName,
        side: InsertSide,
    ) -> Result<RefIndex> {
        let out = self.insert_reference_impl(target, refname, side);
        self.verified(out)
    }

    fn insert_reference_impl(
        &mut self,
        target: impl Into<Anchor>,
        refname: gix::refs::FullName,
        side: InsertSide,
    ) -> Result<RefIndex> {
        let target = self.resolve_anchor(target)?;
        let target_positioned = self.store.is_positioned(target);
        let new_ref = self.store.add_reference(refname, true, false);
        match (side, target_positioned) {
            (InsertSide::Above, false) => {
                // A reference above a commit becomes the bottom of the commit's stack.
                ref_ops::place_ref(
                    &mut self.store,
                    new_ref,
                    RefPlace::Bottom(commit_entry(target)?),
                );
            }
            (InsertSide::Above, true) => {
                // A reference above a reference joins its group one rank up.
                ref_ops::place_ref(
                    &mut self.store,
                    new_ref,
                    RefPlace::Above(ref_entry(target)?),
                );
            }
            (InsertSide::Below, false) => {
                // A reference below a commit sits on top of the group the commit's first-parent
                // parent entry enters (or starts one).
                let target_commit = commit_entry(target)?;
                if let Some(parent_commit) = self.store.parents(target_commit).first().copied() {
                    ref_ops::place_ref(
                        &mut self.store,
                        new_ref,
                        RefPlace::GroupTop {
                            commit: parent_commit,
                            entry: ParentEntry {
                                child: target_commit,
                                number: 0,
                            },
                        },
                    );
                }
            }
            (InsertSide::Below, true) => {
                // A reference below a reference takes its position; it and everything above
                // shift up.
                ref_ops::place_ref(
                    &mut self.store,
                    new_ref,
                    RefPlace::Below(ref_entry(target)?),
                );
            }
        };
        Ok(new_ref)
    }
}
