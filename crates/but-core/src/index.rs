//! Editing a Git index, see [`Edit`].

use std::collections::BTreeMap;

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

impl From<ConflictStage> for Stage {
    fn from(stage: ConflictStage) -> Self {
        match stage {
            ConflictStage::Base => Stage::Base,
            ConflictStage::Ours => Stage::Ours,
            ConflictStage::Theirs => Stage::Theirs,
        }
    }
}

impl Pending {
    /// Whether this adds an entry at `stage`.
    fn adds_at(&self, stage: Stage) -> bool {
        match self {
            Pending::Remove => false,
            Pending::Upsert { .. } => stage == Stage::Unconflicted,
            Pending::Conflict(stages) => stages.slot(stage).is_some_and(|entry| entry.is_some()),
        }
    }

    /// Stop adding an entry at `stage`, and return `true` if nothing is left to add.
    fn drop_stage(&mut self, stage: Stage) -> bool {
        match self {
            Pending::Remove => false,
            Pending::Upsert { .. } => stage == Stage::Unconflicted,
            Pending::Conflict(stages) => {
                if let Some(entry) = stages.slot_mut(stage) {
                    *entry = None;
                }
                stages.base.is_none() && stages.ours.is_none() && stages.theirs.is_none()
            }
        }
    }

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

impl ConflictStages {
    fn slot(&self, stage: Stage) -> Option<&Option<(Mode, gix::ObjectId)>> {
        match stage {
            Stage::Unconflicted => None,
            Stage::Base => Some(&self.base),
            Stage::Ours => Some(&self.ours),
            Stage::Theirs => Some(&self.theirs),
        }
    }

    fn slot_mut(&mut self, stage: Stage) -> Option<&mut Option<(Mode, gix::ObjectId)>> {
        match stage {
            Stage::Unconflicted => None,
            Stage::Base => Some(&mut self.base),
            Stage::Ours => Some(&mut self.ours),
            Stage::Theirs => Some(&mut self.theirs),
        }
    }
}

/// Edits to a Git index that keep it valid for Git.
///
/// After [`finish()`](Self::finish), each path has either one unconflicted entry or conflict stages,
/// no path is both a file and a directory at the same stage, and no cached data is left that the
/// edits made stale.
///
/// Edits are recorded and applied together by `finish()`, so [`state()`](Self::state) stays the
/// unchanged, sorted index until then. Later edits of a path replace earlier ones, and later
/// additions replace earlier ones that clash with them as file versus directory at the same stage.
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
            None => self
                .state
                .entry_range(path)
                .and_then(|range| self.state.entries().get(range))
                .is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        clashing_additions(&self.pending, path, entry.flags.stage())
                            .next()
                            .is_none()
                    })
                }),
        }
    }

    /// Record `path` as unconflicted with `id` and `mode`.
    ///
    /// This removes its conflict stages, and any unconflicted entries that clash with it as file
    /// versus directory, including earlier pending additions.
    pub fn upsert(&mut self, path: &BStr, id: gix::ObjectId, mode: Mode, stat: Stat) {
        self.drop_clashing_additions(path, Stage::Unconflicted);
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
    /// and any entries at `stage` that clash with it as file versus directory, including earlier
    /// pending additions.
    pub fn set_conflict_stage(
        &mut self,
        path: &BStr,
        stage: ConflictStage,
        mode: Mode,
        id: gix::ObjectId,
    ) {
        self.drop_clashing_additions(path, stage.into());
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

    /// Drop the stage of pending additions at `stage` that clash with `path` as file versus
    /// directory, as an index can't hold both at the same stage and the later addition wins.
    ///
    /// A path left with nothing to add is still removed, so its original entries don't come back.
    fn drop_clashing_additions(&mut self, path: &BStr, stage: Stage) {
        let clashing: Vec<BString> = clashing_additions(&self.pending, path, stage)
            .cloned()
            .collect();
        for clashing_path in clashing {
            if let Some(edit) = self.pending.get_mut(&clashing_path)
                && edit.drop_stage(stage)
            {
                *edit = Pending::Remove;
            }
        }
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
/// directory with a path that `pending` adds at the same stage.
///
/// Return `true` if anything was removed. Removing a path the index doesn't have changes nothing,
/// like discard purging an untracked rename destination, so this counts what actually changed to
/// spare callers rewriting the index.
fn remove_replaced_entries(
    state: &mut gix::index::State,
    pending: &BTreeMap<BString, Pending>,
) -> bool {
    let len_before = state.entries().len();
    state.remove_entries(|_, path, entry| {
        pending.contains_key(path)
            || clashing_additions(pending, path, entry.flags.stage())
                .next()
                .is_some()
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

/// The pending additions at `stage` that clash with `path` as file versus directory: one at a
/// directory leading to `path`, or one inside `path`.
///
/// Like Git, only entries at the same stage clash, so both sides of a file/directory conflict fit.
fn clashing_additions<'a>(
    pending: &'a BTreeMap<BString, Pending>,
    path: &'a BStr,
    stage: Stage,
) -> impl Iterator<Item = &'a BString> + 'a {
    let mut prefix = path.to_owned();
    prefix.push(b'/');
    let inside = pending
        .range(prefix.clone()..)
        .take_while(move |(added, _)| added.starts_with(&prefix));
    parents(path)
        .filter_map(move |dir| pending.get_key_value(dir))
        .chain(inside)
        .filter(move |(_, edit)| edit.adds_at(stage))
        .map(|(added, _)| added)
}

/// The directories leading to `path`, which an index must not hold as files at the same stage.
fn parents(path: &BStr) -> impl Iterator<Item = &BStr> {
    path.iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'/')
        .map(move |(pos, _)| path[..pos].as_bstr())
}
