use crate::{
    CliId,
    id::{ShortId, UncommittedCliId},
};

#[derive(Default, Debug, Clone, PartialEq)]
pub(super) struct Marks {
    marks: Vec<Markable>,
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
        self.marks.push(markable);
    }

    pub(super) fn remove(&mut self, markable: &Markable) {
        self.marks.retain(|item| item != markable);
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
        let mut marked_uncommitted = false;
        for mark in &self.marks {
            match mark {
                Markable::Commit { .. } => marked_commits = true,
                Markable::Uncommitted(..) => marked_uncommitted = true,
            }
        }
        MarkClasses {
            marked_commits,
            marked_uncommitted,
        }
    }
}

impl<'a> IntoIterator for &'a Marks {
    type Item = <&'a Vec<Markable> as IntoIterator>::Item;
    type IntoIter = <&'a Vec<Markable> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.marks.iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Markable {
    Uncommitted(UncommittedCliId),
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
            CliId::Uncommitted(uncommitted) => {
                if uncommitted
                    .hunk_assignments
                    .iter()
                    .any(|hunk| hunk.stack_id.is_some())
                {
                    return None;
                }
                Some(Self::Uncommitted(uncommitted.clone()))
            }
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Branch { .. }
            | CliId::Unassigned { .. }
            | CliId::Stack { .. } => None,
        }
    }

    pub fn into_cli_id(self) -> CliId {
        match self {
            Markable::Uncommitted(uncommitted_cli_id) => CliId::Uncommitted(uncommitted_cli_id),
            Markable::Commit { commit_id, id } => CliId::Commit { commit_id, id },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MarkClasses {
    pub marked_commits: bool,
    pub marked_uncommitted: bool,
}
