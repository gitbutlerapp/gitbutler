use std::collections::HashSet;

use crate::{CliId, id::ShortId};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub(super) struct Marks {
    marks: HashSet<Markable>,
}

impl Marks {
    pub(super) fn toggle(&mut self, markable: Markable) {
        if self.marks.contains(&markable) {
            self.remove(&markable);
        } else {
            self.insert(markable);
        }
    }

    pub(super) fn insert(&mut self, markable: Markable) {
        self.marks.insert(markable);
    }

    pub(super) fn remove(&mut self, markable: &Markable) {
        self.marks.remove(markable);
    }

    pub(super) fn clear(&mut self) {
        self.marks.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.marks.len()
    }

    pub(super) fn contains(&self, markable: &Markable) -> bool {
        self.marks.contains(markable)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Markable> {
        self.into_iter()
    }

    pub(super) fn classify(&self) -> MarkClasses {
        let mut marked_commits = false;
        for mark in &self.marks {
            match mark {
                Markable::Commit { .. } => marked_commits = true,
            }
        }
        MarkClasses { marked_commits }
    }
}

impl<'a> IntoIterator for &'a Marks {
    type Item = <&'a HashSet<Markable> as IntoIterator>::Item;
    type IntoIter = <&'a HashSet<Markable> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.marks.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Markable {
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
}

impl Markable {
    pub(super) fn try_from_cli_id(cli_id: &CliId) -> Option<Self> {
        match cli_id {
            CliId::Commit { commit_id, id } => Some(Self::Commit {
                commit_id: *commit_id,
                id: id.clone(),
            }),
            CliId::Uncommitted(..)
            | CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MarkClasses {
    pub marked_commits: bool,
}
