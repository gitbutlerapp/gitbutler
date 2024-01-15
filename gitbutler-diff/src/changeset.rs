use crate::Hunk;

/// Changesets are collections of hunks whereby
/// quite a few operations can be performed on a
/// file.
#[derive(Debug, Clone, Default)]
pub struct Changeset {
    hunks: Vec<Hunk>,
}

/// Denotes that a hunk could not be added to a changeset
/// due to a conflict in the source range.
///
/// This is technically an [`std::error::Error`] type but is used in some
/// non-`Err` contexts.
#[derive(Debug, thiserror::Error)]
#[error("hunk conflicts with existing hunk (new hunk: {:?}, existing hunk: {:?})", .new.source_span(), .existing.source_span())]
pub struct HunkConflict {
    /// The hunk that could not be added.
    pub new: Hunk,
    /// The existing hunk that conflicts with the new hunk.
    pub existing: Hunk,
}

impl Changeset {
    /// Returns an iterator over the hunks in the changeset.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Hunk> {
        self.hunks.iter()
    }

    /// Returns a mutable iterator over the hunks in the changeset.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Hunk> {
        self.hunks.iter_mut()
    }

    fn sort(&mut self) {
        self.hunks.sort_by_key(|hunk| hunk.source_span().start());
    }
}

impl Extend<Hunk> for Changeset {
    fn extend<T: IntoIterator<Item = Hunk>>(&mut self, iter: T) {
        self.hunks.extend(iter);
    }
}

impl FromIterator<Hunk> for Changeset {
    fn from_iter<T: IntoIterator<Item = Hunk>>(iter: T) -> Self {
        Self {
            hunks: Vec::from_iter(iter),
        }
    }
}
