//! Editing a Git index, see [`Edit`].

use std::collections::{BTreeMap, BTreeSet};

use bstr::{BStr, BString, ByteSlice};
use gix::index::entry::{Flags, Mode, Stage};

/// What the index should record about a file on disk.
#[derive(Debug, Clone, Copy)]
pub enum Stat {
    /// Trust this stat, so Git can skip re-hashing the file.
    ///
    /// Take it right after writing the file, or before reading it to compute its object id.
    Known(gix::index::entry::Stat),
    /// Make Git compare the file's content the next time it looks at it.
    Unknown,
}

/// One side of a conflict, as recorded in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStage {
    /// The common ancestor, stage 1.
    Base,
    /// Our side, stage 2.
    Ours,
    /// Their side, stage 3.
    Theirs,
}

#[derive(Debug)]
enum Pending {
    Remove,
    Upsert {
        id: gix::ObjectId,
        mode: Mode,
        stat: Stat,
    },
    Conflict(ConflictStages),
}

impl Pending {
    /// Turn this into a conflict, keeping its stages if it already is one.
    fn conflict_stages(&mut self) -> &mut ConflictStages {
        if !matches!(self, Pending::Conflict(_)) {
            *self = Pending::Conflict(ConflictStages::default());
        }
        match self {
            Pending::Conflict(stages) => stages,
            Pending::Remove | Pending::Upsert { .. } => unreachable!("just made a conflict"),
        }
    }
}

#[derive(Debug, Default)]
struct ConflictStages {
    base: Option<(Mode, gix::ObjectId)>,
    ours: Option<(Mode, gix::ObjectId)>,
    theirs: Option<(Mode, gix::ObjectId)>,
}

/// Edits to a Git index that keep it valid for Git.
///
/// After [`finish()`](Self::finish), each path has either one unconflicted entry or conflict stages,
/// no path is both a file and a directory, and no cached data is left that the edits made stale.
///
/// Edits are recorded and applied together by `finish()`, so [`state()`](Self::state) stays the
/// unchanged, sorted index until then. Later edits of a path replace earlier ones, and later
/// additions replace earlier ones that clash with them as file versus directory.
pub struct Edit<'index> {
    state: &'index mut gix::index::State,
    pending: BTreeMap<BString, Pending>,
}

impl<'index> Edit<'index> {
    /// Start editing `state`.
    pub fn new(state: &'index mut gix::index::State) -> Self {
        Edit {
            state,
            pending: Default::default(),
        }
    }

    /// The index as it was before any of the edits.
    pub fn state(&self) -> &gix::index::State {
        self.state
    }

    /// Whether `path` will have an entry, at any stage, once the edits are applied.
    pub fn contains(&self, path: &BStr) -> bool {
        match self.pending.get(path) {
            Some(Pending::Remove) => false,
            Some(Pending::Upsert { .. } | Pending::Conflict(_)) => true,
            None => !self.is_displaced(path) && self.state.entry_by_path(path).is_some(),
        }
    }

    /// Record `path` as unconflicted with `id` and `mode`.
    ///
    /// This removes its conflict stages, and any entries that clash with it as file versus directory,
    /// including earlier pending additions.
    pub fn upsert(&mut self, path: &BStr, id: gix::ObjectId, mode: Mode, stat: Stat) {
        self.drop_clashing_additions(path);
        self.pending
            .insert(path.to_owned(), Pending::Upsert { id, mode, stat });
    }

    /// Remove `path` at every stage.
    pub fn remove(&mut self, path: &BStr) {
        self.pending.insert(path.to_owned(), Pending::Remove);
    }

    /// Record `mode` and `id` as conflict `stage` of `path`.
    ///
    /// The path keeps only the conflict stages recorded this way, and loses its unconflicted entry
    /// and any entries that clash with it as file versus directory, including earlier pending additions.
    pub fn set_conflict_stage(
        &mut self,
        path: &BStr,
        stage: ConflictStage,
        mode: Mode,
        id: gix::ObjectId,
    ) {
        self.drop_clashing_additions(path);
        let stages = self
            .pending
            .entry(path.to_owned())
            .or_insert(Pending::Remove)
            .conflict_stages();
        let entry = Some((mode, id));
        match stage {
            ConflictStage::Base => stages.base = entry,
            ConflictStage::Ours => stages.ours = entry,
            ConflictStage::Theirs => stages.theirs = entry,
        }
    }

    /// Whether a pending addition at a directory leading to `path`, or inside `path`, will remove
    /// its entries.
    fn is_displaced(&self, path: &BStr) -> bool {
        parents(path).any(|dir| self.is_added(dir)) || self.additions_inside(path).next().is_some()
    }

    /// Forget pending additions that clash with `path` as file versus directory, as an index can't
    /// hold both and the later addition wins.
    fn drop_clashing_additions(&mut self, path: &BStr) {
        let clashing: Vec<BString> = parents(path)
            .filter(|dir| self.is_added(dir))
            .map(ToOwned::to_owned)
            .chain(self.additions_inside(path).cloned())
            .collect();
        for clashing_path in clashing {
            self.pending.remove(&clashing_path);
        }
    }

    fn is_added(&self, path: &BStr) -> bool {
        self.pending
            .get(path)
            .is_some_and(|edit| !matches!(edit, Pending::Remove))
    }

    /// The paths of pending additions inside the directory `path`.
    fn additions_inside(&self, path: &BStr) -> impl Iterator<Item = &BString> {
        let mut prefix = path.to_owned();
        prefix.push(b'/');
        self.pending
            .range(prefix.clone()..)
            .take_while(move |(added, _)| added.starts_with(&prefix))
            .filter(|(_, edit)| !matches!(edit, Pending::Remove))
            .map(|(added, _)| added)
    }

    /// Apply the edits, sort the index, and drop the tree cache and resolve-undo data, which
    /// would be stale otherwise.
    ///
    /// Return `true` if the index changed and needs to be written.
    pub fn finish(self) -> bool {
        let Edit { state, pending } = self;
        if pending.is_empty() {
            return false;
        }
        let mut changed = remove_replaced_entries(state, &pending);
        changed |= add_pending_entries(state, &pending);
        if changed {
            state.sort_entries();
            state.remove_tree();
            state.remove_resolve_undo();
        }
        changed
    }
}

/// Remove the entries of every path in `pending`, and the entries that clash as file versus
/// directory with a path that `pending` adds.
///
/// Return `true` if anything was removed. Removing a path the index doesn't have changes nothing,
/// like discard purging an untracked rename destination, so this counts what actually changed to
/// spare callers rewriting the index.
fn remove_replaced_entries(
    state: &mut gix::index::State,
    pending: &BTreeMap<BString, Pending>,
) -> bool {
    let is_added = |path: &BStr| {
        pending
            .get(path)
            .is_some_and(|edit| !matches!(edit, Pending::Remove))
    };
    let directories_of_added: BTreeSet<&BStr> = pending
        .iter()
        .filter(|(_, edit)| !matches!(edit, Pending::Remove))
        .flat_map(|(path, _)| parents(path.as_bstr()))
        .collect();
    let len_before = state.entries().len();
    state.remove_entries(|_, path, _| {
        pending.contains_key(path)
            || directories_of_added.contains(path)
            || parents(path).any(is_added)
    });
    state.entries().len() != len_before
}

/// Push the unconflicted entries and conflict stages that `pending` adds, unsorted.
///
/// Return `true` if anything was added.
fn add_pending_entries(
    state: &mut gix::index::State,
    pending: &BTreeMap<BString, Pending>,
) -> bool {
    let mut added = false;
    for (path, edit) in pending {
        match edit {
            Pending::Remove => {}
            Pending::Upsert { id, mode, stat } => {
                let stat = match stat {
                    Stat::Known(stat) => *stat,
                    Stat::Unknown => Default::default(),
                };
                state.dangerously_push_entry(stat, *id, Flags::empty(), *mode, path.as_bstr());
                added = true;
            }
            Pending::Conflict(stages) => {
                for (stage, entry) in [
                    (Stage::Base, stages.base),
                    (Stage::Ours, stages.ours),
                    (Stage::Theirs, stages.theirs),
                ] {
                    if let Some((mode, id)) = entry {
                        state.dangerously_push_entry(
                            Default::default(),
                            id,
                            Flags::from_stage(stage),
                            mode,
                            path.as_bstr(),
                        );
                        added = true;
                    }
                }
            }
        }
    }
    added
}

/// The directories leading to `path`, which an index must not hold as files at the same time.
fn parents(path: &BStr) -> impl Iterator<Item = &BStr> {
    path.iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'/')
        .map(move |(pos, _)| path[..pos].as_bstr())
}
