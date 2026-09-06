//! Parent-list primitives: adding, inserting, detaching, and re-pointing parents.
use std::collections::HashSet;

use crate::graph_rebase::commits::ParentEntry;
use crate::graph_rebase::ref_ops;
use crate::graph_rebase::{EditorIndex, positions};
use anyhow::{Context as _, Result, bail};
use but_core::RefMetadata;

use super::{commit_entry, ref_entry};
use crate::graph_rebase::Editor;
use crate::graph_rebase::anchor::Anchor;

impl<M: RefMetadata> Editor<'_, M> {
    /// Append a parent entry from `child` to `parent` after `child`'s existing parents.
    ///
    /// Reference endpoints behave as in [`Self::insert_parent`].
    pub fn add_parent(
        &mut self,
        child: impl Into<Anchor>,
        parent: impl Into<Anchor>,
    ) -> Result<()> {
        self.insert_parent(child, parent, usize::MAX)
    }

    /// Insert a parent entry from `child` to `parent` at `parent number` among `child`'s ordered parents
    /// (clamped to the end); parents at `parent number` and later shift up, statements following.
    ///
    /// A parent entry from a reference positions it at the parent, never a raw parent entry (references
    /// are positions): a live reference re-points through the layout machinery; a dead one
    /// (which upstream-integration retention still redirects) just re-points its retained
    /// position — no group cascade, since its stored position is stale. A parent entry into a
    /// reference enters its group: the commit-level entry goes to the resolved commit and the reference (with
    /// members below it) gains the new parent entry.
    pub fn insert_parent(
        &mut self,
        child: impl Into<Anchor>,
        parent: impl Into<Anchor>,
        parent_number: usize,
    ) -> Result<()> {
        let out = self.insert_parent_impl(child, parent, parent_number);
        self.verified(out)
    }

    fn insert_parent_impl(
        &mut self,
        child: impl Into<Anchor>,
        parent: impl Into<Anchor>,
        parent_number: usize,
    ) -> Result<()> {
        let child = self.resolve_anchor(child)?;
        let parent = self.resolve_anchor(parent)?;
        self.ensure_acyclic(child, parent)?;

        if self.store.state_of(child).is_some() {
            // A parent entry from a reference re-points it; a reference parent merely gains an
            // entering parent entry and may stay immutable.
            self.ensure_mutable_ref(child)?;
            let onto = match self.store.positioned_on(parent) {
                Some(parent_on) => self.resolved_commit(parent_on)?,
                None => commit_entry(parent)?,
            };
            let child_ref = ref_entry(child)?;
            if self.store.is_reference(child_ref) {
                ref_ops::repoint_ref(&mut self.store, child_ref, onto);
            } else {
                self.store.set_retained_position(child_ref, onto);
            }
            return Ok(());
        }
        let parent_is_ref = self.store.is_positioned(parent);
        let parent_commit = match self.store.positioned_on(parent) {
            Some(on) => self.resolved_commit(on)?,
            None => commit_entry(parent)?,
        };
        let child_commit = commit_entry(child)?;
        let parent_number =
            self.store
                .commits
                .insert_parent(child_commit, parent_number, parent_commit);
        // The group is captured after the insert: normalization and the shift rename the
        // child's statements, so a pre-capture would hold stale parent entry names.
        if parent_is_ref {
            let join = positions::prepare_group_join(&self.store, ref_entry(parent)?);
            ref_ops::apply_group_join(
                &mut self.store,
                &join,
                ParentEntry {
                    child: child_commit,
                    number: parent_number,
                },
            );
        }
        Ok(())
    }

    /// Reject a parent entry that would make `child` its own ancestor.
    ///
    /// Checked in release too: a cycle has no valid commit order, so letting one in trades a
    /// clear error here for a silently partial rebase later.
    fn ensure_acyclic(&self, child: EditorIndex, parent: EditorIndex) -> Result<()> {
        let mut seen = HashSet::from([parent]);
        let mut tips = vec![parent];

        while let Some(tip) = tips.pop() {
            for parent in self.store.parents(tip) {
                if seen.insert(parent.into()) {
                    tips.push(parent.into());
                }
            }
        }

        if seen.contains(&child) {
            bail!("BUG: this parent would make the child its own ancestor");
        }
        Ok(())
    }

    /// Sever all parent entries between a child and parent, returning the (pre-removal, ascending)
    /// parent numbers they occupied. Later parents shift down, statements following.
    ///
    /// `detach` severs a link between two commits that both survive; [`Self::remove_commit`]
    /// deletes a commit.
    pub fn detach(
        &mut self,
        child: impl Into<Anchor>,
        parent: impl Into<Anchor>,
    ) -> Result<Vec<usize>> {
        let out = self.detach_impl(child, parent);
        self.verified(out)
    }

    /// Re-point every `child`→`from` parent entry onto `to`: the parent entries are removed and `to`
    /// takes the lowest freed parent number, so the lane keeps its position. Several
    /// freed parent entries collapse into one — two parallel parent entries to one parent are almost
    /// always unintentional, and keeping both would merge `to` twice. Returns the
    /// freed parent numbers; empty when no parent entry connected `child` to `from` (then
    /// nothing is inserted).
    pub fn reparent(
        &mut self,
        child: impl Into<Anchor>,
        from: impl Into<Anchor>,
        to: impl Into<Anchor>,
    ) -> Result<Vec<usize>> {
        let child = self.resolve_anchor(child)?;
        let removed = self.detach(child, from)?;
        if let Some(&parent_number) = removed.iter().min() {
            self.insert_parent(child, to, parent_number)?;
        }
        Ok(removed)
    }

    fn detach_impl(
        &mut self,
        child: impl Into<Anchor>,
        parent: impl Into<Anchor>,
    ) -> Result<Vec<usize>> {
        let child = self.resolve_anchor(child)?;
        let parent = self.resolve_anchor(parent)?;

        // A reference child holds one conceptual downward parent entry (order 0) — its commit. It is
        // reported but not cleared; a follow-up insert_parent re-points, and a position without
        // a resolving commit is not representable.
        if self.store.is_positioned(child) {
            let resolves_to_parent =
                self.store.resolve_to_commit(child) == self.store.resolve_to_commit(parent);
            return Ok(if resolves_to_parent { vec![0] } else { vec![] });
        }
        let numbers = match self.store.positioned_on(parent) {
            // Disconnecting from a reference removes the parent entries carrying its group.
            Some(on) => {
                let target_commit = self.resolved_commit(on)?;
                let group_entries = positions::entering(&self.store, parent);
                let child_commit = commit_entry(child)?;
                self.store
                    .parents(child_commit)
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(parent_number, target)| {
                        (target == target_commit
                            && group_entries.contains(&ParentEntry {
                                child: child_commit,
                                number: parent_number,
                            }))
                        .then_some(parent_number)
                    })
                    .collect::<Vec<_>>()
            }
            // Disconnecting from a commit removes its parent numbers; groups riding a removed parent entry lose
            // it from their entering parent entries below.
            None => self
                .store
                .parents(child)
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(parent_number, target)| {
                    (EditorIndex::from(target) == parent).then_some(parent_number)
                })
                .collect::<Vec<_>>(),
        };

        // Highest-first so earlier parent numbers keep their names; report the pre-removal parent numbers.
        let child_commit = commit_entry(child)?;
        for parent_number in numbers.iter().rev() {
            self.store
                .remove_parent(child_commit, *parent_number)
                .context("BUG: Failed to remove parent")?;
        }

        Ok(numbers)
    }
}
