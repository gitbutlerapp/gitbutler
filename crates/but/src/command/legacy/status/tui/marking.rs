use std::collections::HashMap;

use crate::{
    CliId,
    id::{ShortId, UncommittedCliId},
};

#[derive(Default, Debug, Clone)]
pub(super) struct Marks {
    marks: HashMap<ShortId, Markable>,
}

impl Marks {
    pub(super) fn toggle(&mut self, markable: Markable) {
        if self.marks.contains_key(markable.short_id()) {
            self.remove(&markable);
        } else {
            self.insert(markable);
        }
    }

    pub(super) fn insert(&mut self, markable: Markable) {
        self.marks.insert(markable.short_id().to_owned(), markable);
    }

    pub(super) fn remove(&mut self, markable: &Markable) {
        self.marks.remove(markable.short_id());
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
        self.marks.contains_key(markable.short_id())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Markable> {
        self.marks.values()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Markable {
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
    Uncommitted(UncommittedCliId),
}

impl Markable {
    pub(super) fn try_from_cli_id(cli_id: &CliId) -> Option<Self> {
        match cli_id {
            CliId::Commit { commit_id, id } => Some(Self::Commit {
                commit_id: *commit_id,
                id: id.clone(),
            }),
            CliId::Uncommitted(uncommitted) => Some(Self::Uncommitted(uncommitted.clone())),
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => None,
        }
    }

    fn short_id(&self) -> &ShortId {
        match self {
            Markable::Commit { id, .. } => id,
            Markable::Uncommitted(uncommitted_cli_id) => &uncommitted_cli_id.id,
        }
    }

    pub(super) fn into_cli_id(self) -> CliId {
        match self {
            Markable::Commit { commit_id, id } => CliId::Commit { commit_id, id },
            Markable::Uncommitted(uncommitted_cli_id) => CliId::Uncommitted(uncommitted_cli_id),
        }
    }
}
