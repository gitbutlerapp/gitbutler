use crate::{
    CliId,
    id::{ShortId, UncommittedHunkOrFile},
};

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Marks {
    marks: Vec<Markable>,
}

impl Marks {
    pub fn toggle(&mut self, markable: Markable) {
        if self.marks.contains(&markable) {
            self.remove(&markable);
        } else {
            self.insert(markable);
        }
    }

    pub fn insert(&mut self, markable: Markable) {
        self.marks.push(markable);
    }

    pub fn remove(&mut self, markable: &Markable) {
        self.marks.retain(|item| item != markable);
    }

    pub fn clear(&mut self) {
        self.marks.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn contains(&self, markable: &Markable) -> bool {
        self.marks.contains(markable)
    }

    /// Like [`Markable::try_from_cli_id`] followed by [`Marks::contains`], but without cloning
    /// the cli id's payload. Uncommitted hunks carry their entire diff, so cloning them just for
    /// a marked-check is prohibitively expensive for large diffs.
    pub fn contains_cli_id(&self, cli_id: &CliId) -> bool {
        self.marks.iter().any(|mark| mark.matches_cli_id(cli_id))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Markable> {
        self.into_iter()
    }

    pub fn classify(&self) -> MarkClasses {
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
pub enum Markable {
    Uncommitted(UncommittedHunkOrFile),
    Commit {
        commit_id: gix::ObjectId,
        id: ShortId,
    },
}

impl Markable {
    pub fn try_from_cli_id(cli_id: &CliId) -> Option<Self> {
        match cli_id {
            CliId::Commit { commit_id, id } => Some(Self::Commit {
                commit_id: *commit_id,
                id: id.clone(),
            }),
            CliId::UncommittedHunkOrFile(uncommitted) => {
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
            | CliId::Uncommitted { .. }
            | CliId::Stack { .. } => None,
        }
    }

    /// Returns `true` if this mark refers to `cli_id`.
    ///
    /// Equivalent to `Markable::try_from_cli_id(cli_id) == Some(self)` without the clone.
    pub fn matches_cli_id(&self, cli_id: &CliId) -> bool {
        match (self, cli_id) {
            (
                Markable::Commit { commit_id, id },
                CliId::Commit {
                    commit_id: other_commit_id,
                    id: other_id,
                },
            ) => commit_id == other_commit_id && id == other_id,
            (Markable::Uncommitted(marked), CliId::UncommittedHunkOrFile(uncommitted)) => {
                if uncommitted
                    .hunk_assignments
                    .iter()
                    .any(|hunk| hunk.stack_id.is_some())
                {
                    return false;
                }
                marked == uncommitted
            }
            _ => false,
        }
    }

    pub fn into_cli_id(self) -> CliId {
        match self {
            Markable::Uncommitted(uncommitted_cli_id) => {
                CliId::UncommittedHunkOrFile(uncommitted_cli_id)
            }
            Markable::Commit { commit_id, id } => CliId::Commit { commit_id, id },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MarkClasses {
    pub marked_commits: bool,
    pub marked_uncommitted: bool,
}
