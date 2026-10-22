//! What a checkout changes, decided from trees and the index before anything is touched.

use std::collections::BTreeMap;

use bstr::{BStr, BString};
use gix::{index::entry::Mode, merge::tree::TreatAsUnresolved};

use super::{Conflicts, Trees, transaction::Placed};
use crate::index::{ConflictStage, Edit, Stat};

/// What an index entry records about content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Content {
    pub id: gix::ObjectId,
    pub mode: Mode,
}

impl Content {
    pub fn of(entry: &gix::index::Entry) -> Self {
        Content {
            id: entry.id,
            mode: entry.mode,
        }
    }

    pub fn at(state: &gix::index::State, path: &BStr) -> Option<Self> {
        state.entry_by_path(path).map(Content::of)
    }

    /// Submodules are directories checkout ensures exist, but never fills.
    pub fn is_submodule(&self) -> bool {
        self.mode == Mode::COMMIT
    }
}

/// What checkout does at one worktree path.
#[derive(Debug, Clone, Copy)]
pub(super) enum Change {
    Write(Content),
    Delete,
}

/// One stage of an unresolved conflict, to record in the index.
struct ConflictEntry {
    path: BString,
    stage: ConflictStage,
    content: Content,
}

/// Everything a checkout changes in the worktree and the index.
pub(super) struct Plan {
    /// Paths where the worktree, which matches the baseline, differs from the target.
    changes: BTreeMap<BString, Change>,
    /// Paths the worktree already has as `new_head` has them. Only their index entries change.
    records: BTreeMap<BString, Content>,
    /// Index entries of paths `new_head` deletes.
    removals: Vec<BString>,
    /// The stages of unresolved conflicts.
    conflicts: Vec<ConflictEntry>,
}

impl Plan {
    pub fn new(
        repo: &gix::Repository,
        trees: &Trees,
        conflicts: Conflicts<'_>,
    ) -> anyhow::Result<Self> {
        let index = repo.index_or_empty()?;
        let changes = changes(&trees.baseline, &trees.target);
        Ok(Plan {
            records: already_in_worktree(trees, &index, &changes),
            removals: deleted_by_new_head(trees, &index),
            conflicts: conflict_stages(conflicts),
            changes,
        })
    }

    /// The paths checkout writes, with their content.
    pub fn writes(&self) -> impl Iterator<Item = (&BStr, Content)> {
        self.changes
            .iter()
            .filter_map(|(path, change)| match change {
                Change::Write(content) => Some((path.as_ref(), *content)),
                Change::Delete => None,
            })
    }

    /// The content checkout writes at `path`, if any.
    pub fn write_at(&self, path: &BStr) -> Option<Content> {
        match self.changes.get(path) {
            Some(Change::Write(content)) => Some(*content),
            Some(Change::Delete) | None => None,
        }
    }

    /// The paths checkout deletes.
    pub fn deletions(&self) -> impl Iterator<Item = &BStr> {
        self.changes
            .iter()
            .filter_map(|(path, change)| match change {
                Change::Delete => Some(path.as_ref()),
                Change::Write(_) => None,
            })
    }

    /// Update `.git/index` to what checkout did, writing it once if anything changed.
    pub fn record_in_index(&self, repo: &gix::Repository, placed: &Placed) -> anyhow::Result<()> {
        let mut index = repo.index_or_empty()?.into_owned_or_cloned();
        if self.edit(&mut index, placed) {
            index.write(Default::default())?;
        }
        Ok(())
    }

    /// Change `index` to what checkout records, with the stat of the files it `placed`, and
    /// return `true` if it changed.
    pub fn edit(&self, index: &mut gix::index::State, placed: &Placed) -> bool {
        let mut edit = Edit::new(index);
        self.apply(&mut edit, placed);
        edit.finish()
    }

    fn apply(&self, edit: &mut Edit<'_>, placed: &Placed) {
        // These files weren't written, so their stat may postdate the snapshot their ids come from.
        for (path, content) in &self.records {
            edit.upsert(path.as_ref(), content.id, content.mode, Stat::Unknown);
        }
        for path in self
            .deletions()
            .chain(self.removals.iter().map(AsRef::as_ref))
        {
            edit.remove(path);
        }
        for (path, content) in self.writes() {
            edit.upsert(path, content.id, content.mode, placed.stat(path));
        }
        for ConflictEntry {
            path,
            stage,
            content,
        } in &self.conflicts
        {
            edit.set_conflict_stage(path.as_ref(), *stage, content.mode, content.id);
        }
    }
}

/// Compare the `baseline`, which the worktree matches, with the `target`.
fn changes(baseline: &gix::index::State, target: &gix::index::State) -> BTreeMap<BString, Change> {
    let writes = target
        .entries()
        .iter()
        .map(|entry| (entry.path(target), Content::of(entry)))
        .filter(|(path, content)| Content::at(baseline, path) != Some(*content))
        .map(|(path, content)| (path.to_owned(), Change::Write(content)));
    let deletions = baseline
        .entries()
        .iter()
        .map(|entry| entry.path(baseline))
        .filter(|path| target.entry_by_path(path).is_none())
        .map(|path| (path.to_owned(), Change::Delete));
    writes.chain(deletions).collect()
}

/// Paths checkout doesn't write, whose index entries follow `new_head` anyway.
fn already_in_worktree(
    trees: &Trees,
    index: &gix::index::State,
    changes: &BTreeMap<BString, Change>,
) -> BTreeMap<BString, Content> {
    trees
        .target
        .entries()
        .iter()
        .map(|entry| (entry.path(&trees.target), Content::of(entry)))
        .filter(|(path, _)| !changes.contains_key(*path))
        .filter(|(path, content)| index_follows_new_head(trees, index, path, *content))
        .map(|(path, content)| (path.to_owned(), content))
        .collect()
}

/// Like Git, the index takes `new_head`'s entry for a file the worktree already has like
/// `new_head`, unless the user staged something there or the entry already matches.
fn index_follows_new_head(
    trees: &Trees,
    index: &gix::index::State,
    path: &BStr,
    content: Content,
) -> bool {
    let current = Content::at(index, path);
    let nothing_staged = current == Content::at(&trees.head, path);
    nothing_staged
        && Content::at(&trees.new_head, path) == Some(content)
        && current != Some(content)
}

/// Index entries of files `new_head` deletes, which Git removes, unless the user staged
/// changes to them. Those still in the worktree are deleted by checkout too, and those that
/// aren't, which the user deleted as well, aren't in the worktree snapshot.
fn deleted_by_new_head(trees: &Trees, index: &gix::index::State) -> Vec<BString> {
    index
        .entries()
        .iter()
        .filter(|entry| {
            let path = entry.path(index);
            Content::at(&trees.head, path) == Some(Content::of(entry))
                && trees.new_head.entry_by_path(path).is_none()
        })
        .map(|entry| entry.path(index).to_owned())
        .collect()
}

/// The stages of the unresolved conflicts in `conflicts`.
fn conflict_stages(conflicts: Conflicts<'_>) -> Vec<ConflictEntry> {
    conflicts
        .into_iter()
        .flat_map(|outcome| outcome.conflicts.iter())
        .filter(|conflict| conflict.is_unresolved(TreatAsUnresolved::git()))
        .flat_map(|conflict| {
            let path = conflict.ours.location();
            let stages = [
                ConflictStage::Base,
                ConflictStage::Ours,
                ConflictStage::Theirs,
            ];
            stages
                .into_iter()
                .zip(conflict.entries())
                .filter_map(move |(stage, entry)| {
                    let entry = entry?;
                    Some(ConflictEntry {
                        path: path.to_owned(),
                        stage,
                        content: Content {
                            id: entry.id,
                            mode: entry.mode.into(),
                        },
                    })
                })
        })
        .collect()
}
