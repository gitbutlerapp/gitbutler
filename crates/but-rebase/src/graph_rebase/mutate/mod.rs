//! Operations for mutating the editor: the verb surface (selection, replace/remove, and
//! the entry points), with the heavier choreography in its submodules —
//! [`disconnect`](self) surgery in `disconnect`, range/commit/reference insertion in
//! `insert`, and the parent-list primitives in `parents`.

mod disconnect;
mod insert;
mod parents;
#[cfg(test)]
mod tests;

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::RefMetadata;
use serde::{Deserialize, Serialize};

use crate::graph_rebase::anchor::Anchor;
use crate::graph_rebase::anchor::{Connect, Cut, Range};
use crate::graph_rebase::{CommitIndex, CommitSpec, Editor, EditorIndex, RefIndex};

/// Describes where relative to the target an insertion lands
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum InsertSide {
    /// Children of the target become children of the inserted entry.
    Above,
    /// Parents of the target become parents of the inserted entry.
    Below,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(InsertSide);

/// What happens to the parent entries a disconnect severs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconnect {
    /// Rewire the disconnected children onto the disconnected parents, healing the
    /// graph around the removed range.
    Heal,
    /// Leave children and parents apart; the caller reconnects them itself.
    Skip,
}

/// Operations for mutating the editor store
impl<M: RefMetadata> Editor<'_, M> {
    // ── Selection: id or name → index. ──

    /// The index of the commit `id` in the graph; an error when absent.
    pub fn select_commit(&self, id: gix::ObjectId) -> Result<CommitIndex> {
        match self.try_select_commit(id) {
            Some(index) => Ok(index),
            None => Err(anyhow!("Failed to find commit {id} in rebase editor")),
        }
    }

    /// The index of the commit `id` in the graph, when present.
    pub fn try_select_commit(&self, id: gix::ObjectId) -> Option<CommitIndex> {
        self.store
            .commits
            .commit_indices()
            .find(|&commit_idx| self.store.commit_id(commit_idx) == Some(id))
    }

    /// Select several commits at once, in input order — the batch form of
    /// [`Self::select_commit`], for operations that take a list of subjects.
    pub fn select_commits(
        &self,
        ids: impl IntoIterator<Item = gix::ObjectId>,
    ) -> Result<Vec<CommitIndex>> {
        ids.into_iter().map(|id| self.select_commit(id)).collect()
    }

    /// The index of the reference `name` in the graph; an error when absent.
    pub fn select_reference(&self, name: &gix::refs::FullNameRef) -> Result<RefIndex> {
        match self.try_select_reference(name) {
            Some(index) => Ok(index),
            None => Err(anyhow!("Failed to find reference {name} in rebase editor")),
        }
    }

    /// The index of the reference `name` in the graph, when present.
    pub fn try_select_reference(&self, name: &gix::refs::FullNameRef) -> Option<RefIndex> {
        for (ref_idx, refname, _) in self.store.references() {
            if name == refname.as_ref() {
                return Some(ref_idx);
            }
        }

        None
    }

    // ── Replace and remove: entries change or tombstone in place. ──

    /// Replace the commit at `commit` with `spec`, recording a commit mapping from the old
    /// to the new id (unless either side is untracked).
    pub fn replace_commit(&mut self, commit: CommitIndex, spec: CommitSpec) -> Result<()> {
        if let Some(from) = self.store.commits.commit_spec(commit)
            && !from.exclude_from_tracking
            && !spec.exclude_from_tracking
        {
            self.history.update_mapping(from.id, spec.id);
        }
        self.store.commits.set_commit(commit, spec);
        self.verified(Ok(()))
    }

    /// Drop the commit at `commit` in place: the entry tombstones, and the rebase replays
    /// history without it — children resolve through the tombstone to its parents. Unlike
    /// [`Self::remove_commit`], the surrounding parent entries are not rewired eagerly; the
    /// materialized outcome is identical (merges included, pinned by test), so choose by
    /// whether later same-session operations need to see the healed graph.
    pub fn drop_commit(&mut self, commit: CommitIndex) -> Result<()> {
        self.store.commits.tombstone_commit(commit);
        self.verified(Ok(()))
    }

    /// Remove a commit node: the graph heals around it (its children reconnect to its
    /// parents) and its entry tombstones. The commit leaves the rebuild entirely — used
    /// where its content already lives elsewhere (squashed into another commit) or is
    /// being discarded.
    ///
    /// `remove_commit` deletes a commit; [`Self::detach`] severs a link between two
    /// commits that both survive.
    pub fn remove_commit(&mut self, commit: CommitIndex) -> Result<()> {
        self.disconnect(Range::single(commit), Cut::All, Cut::All, Reconnect::Heal)?;
        self.drop_commit(commit)
    }

    /// Rename the reference at `reference` to `refname`, keeping its mutability.
    pub fn rename_reference(
        &mut self,
        reference: RefIndex,
        refname: gix::refs::FullName,
    ) -> Result<()> {
        self.ensure_mutable_ref(reference.into())?;
        self.store.set_reference(reference, refname, true);
        self.verified(Ok(()))
    }

    /// Delete the reference at `reference`: dependents heal past it, while its name and
    /// stored position are retained so stale indices keep resolving.
    pub fn remove_reference(&mut self, reference: RefIndex) -> Result<()> {
        self.ensure_mutable_ref(reference.into())?;
        // Deleting removes the reference from the physical stack: splice dependents
        // past it. Name and stored position are kept for retention reads.
        let was_live = self.store.is_reference(reference);
        self.store.tombstone_reference(reference);
        if was_live {
            self.store.splice(reference);
        }
        self.verified(Ok(()))
    }

    // ── Moving: disconnect and re-insert as one verb. ──

    /// Move the positioned reference `subject` to sit `side` of `target` — above or below
    /// another reference, or onto a commit's stack — without touching any commit.
    ///
    /// This is the typed single-reference form of [`Self::move_range`]: the reference
    /// leaves its group (members above close the gap, carried parent entries heal onto
    /// its old commit) and re-anchors at the target.
    pub fn move_reference(
        &mut self,
        subject: RefIndex,
        target: impl Into<Anchor>,
        side: InsertSide,
    ) -> Result<()> {
        if !self.store.is_positioned(subject) {
            bail!("Can only move a reference that holds a position");
        }
        self.move_range(
            Range::single(subject),
            Cut::All,
            target,
            side,
            Connect::Splice,
            Reconnect::Heal,
        )
    }

    /// Move `range` to sit `side` of `target`: the named `children` sever above it (the
    /// hole heals past the range unless `reconnect` says [`Reconnect::Skip`]), its base
    /// severs — the lowest-numbered parent entry of `range.parent`, so a merge's side
    /// parents travel with the range — and the range re-wires at the target per `connect`.
    ///
    /// The base is read from this graph, not from a workspace projection: ahead-of-base
    /// shapes collapse the base segment in projections, while the graph keeps every
    /// interposed reference, so only the graph's lowest-numbered entry names the true
    /// base. The child seam is the caller's to name: which child stands above the range
    /// (and where a shared reference group splits) is workspace knowledge the graph
    /// alone cannot decide.
    pub fn move_range(
        &mut self,
        range: Range,
        children: Cut,
        target: impl Into<Anchor>,
        side: InsertSide,
        connect: Connect,
        reconnect: Reconnect,
    ) -> Result<()> {
        let parents = self.base_of(range.parent)?;
        self.disconnect(range, children, parents, reconnect)?;
        self.insert_range(target, range, side, connect)
    }

    /// The cut severing `entry`'s base: its lowest-numbered parent entry, so a merge's
    /// side parents stay attached and travel with whatever the cut lifts out.
    pub fn base_of(&self, entry: impl Into<Anchor>) -> Result<Cut> {
        let base = self
            .direct_parents(entry)?
            .into_iter()
            .min_by_key(|(_, number)| *number);
        Ok(match base {
            Some((base, _)) => Cut::only([base]),
            // A root has no base to sever. `All` of an empty parent list cuts the same
            // zero entries as `Nothing` would, but `Nothing` declares the base is being
            // Kept — which `disconnect` rejects as contradicting a heal.
            None => Cut::All,
        })
    }

    // ── Bare additions: unconnected entries, for callers wiring by hand. ──

    /// Add the commit `spec` describes to the graph, unconnected. Almost always you want
    /// [`Self::insert_commit`] instead.
    pub fn add_commit(&mut self, spec: CommitSpec) -> Result<CommitIndex> {
        let new_idx = self.store.commits.add_commit(spec);
        Ok(new_idx)
    }

    /// Add a mutable reference named `refname` to the graph, unpositioned. Almost always
    /// you want [`Self::insert_reference`] instead.
    pub fn add_reference(&mut self, refname: gix::refs::FullName) -> Result<RefIndex> {
        let new_idx = self.store.add_reference(refname, true, false);
        Ok(new_idx)
    }

    // ── Internal: shared guards and resolution. ──

    /// The commit `entry` resolves to; an error when it resolves to nothing (an unborn ref).
    pub(crate) fn resolved_commit(&self, entry: impl Into<EditorIndex>) -> Result<CommitIndex> {
        self.store
            .resolve_to_commit(entry)
            .context("Reference target should resolve to a commit")
    }

    /// Bail when `entry` is an immutable reference (e.g. a remote-tracking ref):
    /// materialization would refuse the write, so the op fails up front instead of
    /// succeeding session-only.
    fn ensure_mutable_ref(&self, entry: EditorIndex) -> Result<()> {
        if let Some(record) = self.store.state_of(entry)
            && !record.mutable
        {
            bail!(
                "reference {} is immutable and cannot be moved, renamed, or deleted",
                record.refname
            );
        }
        Ok(())
    }

    /// Debug builds verify well-formedness at the exit of every public mutation except
    /// the bare [`Self::add_commit`] / [`Self::add_reference`] (their entries start
    /// unconnected), so an ill-formed graph is caught at the mutation that produced it
    /// rather than at the next rebase.
    /// Release builds rely on the `rebase()`/creation boundary checks.
    ///
    /// This is why mutations come as a `foo`/`foo_impl` pair: the public `foo` is only
    /// `self.verified(self.foo_impl(...))`, so no arm can return an unchecked graph.
    fn verified<T>(&self, out: Result<T>) -> Result<T> {
        #[cfg(debug_assertions)]
        if out.is_ok() {
            crate::graph_rebase::positions::assert_positions_total(&self.store)?;
        }
        out
    }
}

/// The ref-table index of `entry`; an error when it addresses a commit instead.
fn ref_entry(entry: EditorIndex) -> Result<RefIndex> {
    entry
        .as_ref()
        .context("operation targets a reference, but a commit was addressed")
}

/// The commit-half index of `entry`; an error when it addresses a reference instead.
pub(crate) fn commit_entry(entry: EditorIndex) -> Result<CommitIndex> {
    entry
        .as_commit()
        .context("operation targets a commit, but a reference was addressed")
}
