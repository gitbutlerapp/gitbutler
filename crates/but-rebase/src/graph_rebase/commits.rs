//! The store's commit half: a [`but_graph::CommitGraph`] mounted for editing, plus the
//! lockstep columns editing needs — replay settings, the derived children index, and
//! stable parent-entry ids. One row per commit, all indexed by the same
//! [`CommitIndex`]; appends flow through `push_rows`, which asserts the columns never
//! drift apart. `adopt` mounts the graph, `into_commit_graph` hands the same type back
//! out as the editor's final product.
//!
//! This module has no reference knowledge at all: no ref table, no positions, no
//! carries. That is a checked property, not a habit — it imports nothing from
//! `store`'s ref side, `positions`, or `ref_ops`, so commit surgery *cannot*
//! touch references by construction. Everything GitButler-specific about references
//! rides in the editor's other store, keyed to these rows by [`CommitIndex`] and
//! [`ParentEntryId`].

use but_core::commit::SignCommit;

use crate::graph_rebase::{
    CommitSpec,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// A row in the commit half — a commit or its tombstone. Nodes are the only entities that
/// carry parent entries: parent arrays connect nodes, never references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommitIndex(pub(crate) usize);

impl std::fmt::Display for CommitIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.0)
    }
}

/// One incoming child parent entry of a commit, named positionally as `(source node, parent number)` —
/// the coordinate the children index and carry intents are expressed in. Carry statements
/// resolve these coordinates to a stable [`ParentEntryId`] when authored, so a parent entry removed and
/// re-created at the same coordinates is a different parent entry that no statement follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ParentEntry {
    /// The commit whose parent list holds the entry.
    pub child: CommitIndex,
    /// The parent number: the entry's position in `child`'s parent list.
    pub number: usize,
}

/// A stable parent-entry identity: allocated once per parent-list entry — at ingest or when a
/// mutation creates the parent entry — and immune to parent-number renumbering. Re-pointing a parent entry
/// (`replace_parent`, `redirect_children`) keeps its id, so statements follow the parent entry to its
/// new target; deleting a parent retires the id for good. Because carry statements name parent entries
/// by id rather than by position, renumbering a parent array cannot invalidate them, and no
/// rename maintenance is needed to keep them pointing at the right parent entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ParentEntryId(u64);

/// Everything a [`CommitSpec`] carries except the commit id — the id lives on the graph commit itself,
/// so the options live beside it. Stale for tombstones (never read; a revival overwrites).
#[derive(Debug, Clone)]
pub(crate) struct CommitSettings {
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    pub pick_mode: PickMode,
    pub sign_commit: SignCommit,
    pub exclude_from_tracking: bool,
    pub conflictable: bool,
    pub tree_merge_mode: TreeMergeMode,
    pub mutable: bool,
}

impl CommitSettings {
    fn split(spec: CommitSpec) -> (gix::ObjectId, Self) {
        let CommitSpec {
            id,
            preserved_parents,
            pick_mode,
            sign_commit,
            exclude_from_tracking,
            conflictable,
            tree_merge_mode,
            mutable,
        } = spec;
        (
            id,
            Self {
                preserved_parents,
                pick_mode,
                sign_commit,
                exclude_from_tracking,
                conflictable,
                tree_merge_mode,
                mutable,
            },
        )
    }

    fn spec(&self, id: gix::ObjectId) -> CommitSpec {
        CommitSpec {
            id,
            preserved_parents: self.preserved_parents.clone(),
            pick_mode: self.pick_mode,
            sign_commit: self.sign_commit,
            exclude_from_tracking: self.exclude_from_tracking,
            conflictable: self.conflictable,
            tree_merge_mode: self.tree_merge_mode,
            mutable: self.mutable,
        }
    }
}

impl Default for CommitSettings {
    fn default() -> Self {
        let (_, settings) =
            Self::split(CommitSpec::new(gix::ObjectId::null(gix::hash::Kind::Sha1)));
        settings
    }
}

/// A [`but_graph::CommitGraph`] mounted for editing: commits carry ordered parent
/// arrays, with replay settings, a derived children index, and stable parent-entry ids
/// in lockstep columns. Nothing is ever deleted (a removed commit is tombstoned in
/// place), so ids stay stable for the session's lifetime.
#[derive(Debug, Clone, Default)]
pub(crate) struct Commits {
    /// The substrate: commit ids are the commit (tombstoning flags a commit in place,
    /// its index survives every rewrite); the parent arrays are the ordered structure.
    graph: but_graph::CommitGraph,
    /// Each commit's options.
    settings: Vec<CommitSettings>,
    /// The derived children index: `children[p]` holds every `(child, parent number)`
    /// parent entry naming commit `p`, sorted. Maintained through [`Self::update_parents`] — the
    /// single seam every parent mutation flows through — so [`Self::children_of`] is a
    /// lookup, not a graph scan.
    children: Vec<Vec<ParentEntry>>,
    /// Each commit's parent-array parent entry ids — an inner parallel array, one id per parent
    /// entry: `parent_entry_ids[child][parent_number]` is the stable identity of that parent entry.
    /// Maintained by the named parent mutators, never by renumbering.
    parent_entry_ids: Vec<Vec<ParentEntryId>>,
    /// The parent entry-id allocator — monotonically increasing, never reused.
    next_parent_entry_id: u64,
}

impl Commits {
    /// Mount `graph` for editing. Full commit records (flags, refs, generation)
    /// survive, which handing the graph back out after a rebase depends on. Every parent
    /// entry must already point at a node in the graph — the editor's standing requirement.
    pub(crate) fn adopt(graph: but_graph::CommitGraph) -> Self {
        let settings = vec![CommitSettings::default(); graph.commit_count()];
        let mut children: Vec<Vec<ParentEntry>> = vec![Vec::new(); graph.commit_count()];
        for i in 0..graph.commit_count() {
            for (parent_number, parent) in graph.parent_indices(i).into_iter().enumerate() {
                children[parent].push(ParentEntry {
                    child: CommitIndex(i),
                    number: parent_number,
                });
            }
        }
        let mut next_parent_entry_id = 0u64;
        let parent_entry_ids = (0..graph.commit_count())
            .map(|i| {
                (0..graph.parent_indices(i).len())
                    .map(|_| {
                        let id = ParentEntryId(next_parent_entry_id);
                        next_parent_entry_id += 1;
                        id
                    })
                    .collect()
            })
            .collect();
        Self {
            graph,
            settings,
            children,
            parent_entry_ids,
            next_parent_entry_id,
        }
    }

    fn alloc_entry_id(&mut self) -> ParentEntryId {
        let id = ParentEntryId(self.next_parent_entry_id);
        self.next_parent_entry_id += 1;
        id
    }

    /// The stable identity of the live parent entry at `(child, parent_number)`, if it exists.
    pub(crate) fn entry_id_at(
        &self,
        child: CommitIndex,
        parent_number: usize,
    ) -> Option<ParentEntryId> {
        self.parent_entry_ids
            .get(child.0)?
            .get(parent_number)
            .copied()
    }

    /// The mounted graph, read-only — the write-through seam projects it after a rebase.
    pub(crate) fn graph(&self) -> &but_graph::CommitGraph {
        &self.graph
    }

    /// Surrender the mounted graph — the materialized commit graph, the editor's final
    /// product.
    pub(crate) fn into_graph(self) -> but_graph::CommitGraph {
        self.graph
    }

    /// Add the commit `spec` describes to the commit half and return its stable id.
    pub(crate) fn add_commit(&mut self, spec: CommitSpec) -> CommitIndex {
        let (id, settings) = CommitSettings::split(spec);
        let i = self.graph.add_commit(id);
        self.push_rows(settings, i)
    }

    /// Add an entry born tombstoned: a placeholder that holds no commit. Only unit-test
    /// graph builders construct these; real removal tombstones an existing commit.
    #[cfg(test)]
    pub(crate) fn add_tombstone(&mut self) -> CommitIndex {
        let i = self.graph.add_tombstone();
        self.push_rows(CommitSettings::default(), i)
    }

    fn push_rows(&mut self, settings: CommitSettings, i: usize) -> CommitIndex {
        self.settings.push(settings);
        self.children.push(Vec::new());
        self.parent_entry_ids.push(Vec::new());
        debug_assert_eq!(
            self.settings.len(),
            self.graph.commit_count(),
            "settings table fell out of step with the commit graph"
        );
        CommitIndex(i)
    }

    /// Overwrite the commit at `entry` per `spec` — id and settings both; revives a
    /// tombstone.
    pub(crate) fn set_commit(&mut self, entry: CommitIndex, spec: CommitSpec) {
        let (id, settings) = CommitSettings::split(spec);
        self.graph.revive_commit(entry.0, id);
        self.settings[entry.0] = settings;
    }

    /// Tombstone the node at `entry`: it stops holding a commit (settings go stale, not
    /// cleared).
    pub(crate) fn tombstone_commit(&mut self, entry: CommitIndex) {
        self.graph.tombstone_commit(entry.0);
    }

    /// The commit id of the commit at `entry` — `None` for tombstones. The cheap way to
    /// read just the id; use [`Self::commit_spec`] for the full spec.
    pub(crate) fn commit_id(&self, entry: CommitIndex) -> Option<gix::ObjectId> {
        self.graph.commit_id(entry.0)
    }

    /// The spec of the commit at `entry`, assembled from the graph commit and its
    /// settings column; `None` for a tombstone.
    pub(crate) fn commit_spec(&self, entry: CommitIndex) -> Option<CommitSpec> {
        self.graph
            .commit_id(entry.0)
            .map(|id| self.settings[entry.0].spec(id))
    }

    /// Rewrite the commit id of the commit at `entry` in place — the rebase write: the node id,
    /// its parent array, its settings, and every position naming it all survive unchanged.
    pub(crate) fn set_commit_id(&mut self, entry: CommitIndex, id: gix::ObjectId) {
        debug_assert!(
            self.graph.commit_id(entry.0).is_some(),
            "tombstones have no commit id"
        );
        self.graph.set_commit_id(entry.0, id);
    }

    /// Overwrite the preserved parents of the commit at `entry` (see
    /// [`CommitSpec::preserved_parents`]).
    pub(crate) fn set_preserved_parents(
        &mut self,
        entry: CommitIndex,
        parents: Option<Vec<gix::ObjectId>>,
    ) {
        debug_assert!(
            self.graph.commit_id(entry.0).is_some(),
            "tombstones carry no spec to read"
        );
        self.settings[entry.0].preserved_parents = parents;
    }

    /// All commit-half ids (commits and tombstones), ascending.
    pub(crate) fn commit_indices(&self) -> impl Iterator<Item = CommitIndex> + '_ {
        (0..self.graph.commit_count()).map(CommitIndex)
    }

    /// The nodes that no other node lists as a parent — the childless tips, ascending.
    pub(crate) fn tips(&self) -> impl Iterator<Item = CommitIndex> + '_ {
        (0..self.graph.commit_count())
            .filter(|&i| self.children[i].is_empty())
            .map(CommitIndex)
    }

    /// The ordered parents of `entry` — parent number position is the parent order.
    pub(crate) fn parents(&self, entry: CommitIndex) -> Vec<CommitIndex> {
        self.graph
            .parent_indices(entry.0)
            .into_iter()
            .map(CommitIndex)
            .collect()
    }

    /// How many parents `entry` has.
    pub(crate) fn parent_count(&self, entry: CommitIndex) -> usize {
        self.graph.parent_indices(entry.0).len()
    }

    /// Every parent entry that names `entry` as a parent, as sorted `(child, parent number)`
    /// pairs — answered from the maintained children index, not a graph scan.
    pub(crate) fn children_of(&self, entry: CommitIndex) -> &[ParentEntry] {
        &self.children[entry.0]
    }

    /// Append `parent` as `child`'s last parent; returns its parent number.
    pub(crate) fn push_parent(&mut self, child: CommitIndex, parent: CommitIndex) -> usize {
        let id = self.alloc_entry_id();
        self.parent_entry_ids[child.0].push(id);
        self.update_parents(child, |parents| {
            parents.push(parent);
            parents.len() - 1
        })
    }

    /// Insert `parent` at `parent number` of `child` (clamped to the array end); later
    /// parent numbers shift up, their statements untouched — a statement names an
    /// [`ParentEntryId`], not a position. Returns the parent number actually used.
    pub(crate) fn insert_parent(
        &mut self,
        child: CommitIndex,
        parent_number: usize,
        parent: CommitIndex,
    ) -> usize {
        let len = self.parent_count(child);
        let parent_number = parent_number.min(len);
        let id = self.alloc_entry_id();
        self.parent_entry_ids[child.0].insert(parent_number, id);
        self.update_parents(child, |parents| parents.insert(parent_number, parent));
        parent_number
    }

    /// Remove `child`'s parent at `parent number`, returning it with its retired entry id;
    /// later parent numbers shift down, their statements untouched. The caller owns
    /// dropping any statements naming the retired id — the commit half knows no statements.
    pub(crate) fn remove_parent(
        &mut self,
        child: CommitIndex,
        parent_number: usize,
    ) -> Option<(CommitIndex, ParentEntryId)> {
        let len = self.parent_count(child);
        if parent_number >= len {
            return None;
        }
        let target = self.update_parents(child, |parents| parents.remove(parent_number));
        let removed = self.parent_entry_ids[child.0].remove(parent_number);
        Some((target, removed))
    }

    /// Re-point `child`'s parent at `parent number` onto `new_parent`. The parent entry keeps its
    /// [`ParentEntryId`], so groups stated on it follow the parent entry to its new target.
    pub(crate) fn replace_parent(
        &mut self,
        child: CommitIndex,
        parent_number: usize,
        new_parent: CommitIndex,
    ) {
        self.update_parents(child, |parents| match parents.get_mut(parent_number) {
            Some(entry) => *entry = new_parent,
            None => debug_assert!(
                false,
                "replace_parent: {child} has no parent_number {parent_number}"
            ),
        });
    }

    /// Move `from`'s whole parent array onto `to` (which must have none); the parent entries keep
    /// their identities, so statements follow without any rewrite.
    pub(crate) fn transplant_parents(&mut self, from: CommitIndex, to: CommitIndex) {
        debug_assert_eq!(
            self.parent_count(to),
            0,
            "transplant target {to} already has parents"
        );
        let parents = self.update_parents(from, std::mem::take);
        let ids = std::mem::take(&mut self.parent_entry_ids[from.0]);
        self.parent_entry_ids[to.0] = ids;
        self.update_parents(to, |to_parents| *to_parents = parents);
    }

    /// Re-target every parent-array entry naming `from` onto `to`, parent numbers preserved —
    /// the parent entries keep their ids, so statements naming them stay valid untouched.
    pub(crate) fn redirect_children(&mut self, from: CommitIndex, to: CommitIndex) {
        // The index is sorted by child, so consecutive parent entries of one child dedup away.
        let mut children: Vec<CommitIndex> = self
            .children_of(from)
            .iter()
            .map(|&ParentEntry { child, .. }| child)
            .collect();
        children.dedup();
        for child in children {
            self.update_parents(child, |parents| {
                for parent in parents.iter_mut() {
                    if *parent == from {
                        *parent = to;
                    }
                }
            });
        }
    }

    /// Empty `child`'s parent array, returning each parent with its retired parent entry id.
    /// Statements naming the drained parent entries are the caller's to re-state onto their
    /// new carrier — the commit half knows no statements.
    pub(crate) fn drain_parents(
        &mut self,
        child: CommitIndex,
    ) -> Vec<(CommitIndex, ParentEntryId)> {
        let parents = self.update_parents(child, std::mem::take);
        let ids = std::mem::take(&mut self.parent_entry_ids[child.0]);
        parents.into_iter().zip(ids).collect()
    }

    /// Rewrite `child`'s parent array through `f` — the single seam every parent mutation
    /// flows through into the graph's parent number write. Parents are [`CommitIndex`] by type:
    /// references in a parent entry are unrepresentable.
    fn update_parents<R>(
        &mut self,
        child: CommitIndex,
        f: impl FnOnce(&mut Vec<CommitIndex>) -> R,
    ) -> R {
        let old = self.graph.parent_indices(child.0);
        let mut parents: Vec<CommitIndex> = old.iter().copied().map(CommitIndex).collect();
        let result = f(&mut parents);
        let targets: Vec<usize> = parents.into_iter().map(|parent| parent.0).collect();
        for (parent_number, &parent) in old.iter().enumerate() {
            let entries = &mut self.children[parent];
            if let Ok(at) = entries.binary_search(&ParentEntry {
                child,
                number: parent_number,
            }) {
                entries.remove(at);
            }
        }
        for (parent_number, &parent) in targets.iter().enumerate() {
            let entries = &mut self.children[parent];
            match entries.binary_search(&ParentEntry {
                child,
                number: parent_number,
            }) {
                Err(at) => entries.insert(
                    at,
                    ParentEntry {
                        child,
                        number: parent_number,
                    },
                ),
                Ok(_) => debug_assert!(
                    false,
                    "children index already names ({child}, {parent_number})"
                ),
            }
        }
        // Preserved parents pin the commit's onto-commits only while its parent entries are
        // untouched (they carry raw parents the walk didn't materialize). Once a
        // mutation rewrites the parent array, the live parent entries are the truth — a stale
        // preserved list would make the rebase silently ignore the reparenting.
        if targets.as_slice() != old && self.settings[child.0].preserved_parents.is_some() {
            self.settings[child.0].preserved_parents = None;
        }
        self.graph.set_parents(child.0, targets);
        result
    }
}
