use std::collections::BTreeMap;

use but_core::{DiffSpec, tree::create_tree::RejectionReason};
use but_rebase::graph_rebase::{Editor, ToSelector};

/// Outcome after creating a commit.
pub struct CommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after moving changes between commits.
pub struct MoveChangesResult {
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after rewording a commit.
pub struct CommitRewordResult {
    /// The ID of the newly created commit with the updated message.
    pub new_commit: gix::ObjectId,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome of moving a commit.
pub struct CommitMoveResult {
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Outcome after inserting a blank commit.
pub struct CommitInsertBlankResult {
    /// The ID of the newly inserted blank commit.
    pub new_commit: gix::ObjectId,
    /// Commits that were replaced by this operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// Specifies a location relative to which a commit operation should occur.
/// This is the fully-owned cousin of [but_rebase::graph_rebase::mutate::RelativeTo]
/// as the [`but_api_macros::but_api`] macro doesn't support contained references or referenced parameters
/// beyond a few well-known ones.
#[derive(Debug, Clone)]
pub enum RelativeTo {
    /// Relative to a commit.
    Commit(gix::ObjectId),
    /// Relative to a reference.
    Reference(gix::refs::FullName),
}

impl From<super::json::RelativeTo> for RelativeTo {
    fn from(value: super::json::RelativeTo) -> Self {
        match value {
            super::json::RelativeTo::Commit(commit) => Self::Commit(commit),
            super::json::RelativeTo::Reference(reference)
            | super::json::RelativeTo::ReferenceBytes(reference) => Self::Reference(reference),
        }
    }
}

impl ToSelector for RelativeTo {
    fn to_selector(&self, editor: &Editor) -> anyhow::Result<but_rebase::graph_rebase::Selector> {
        match self {
            Self::Commit(commit) => editor.select_commit(*commit),
            Self::Reference(reference) => editor.select_reference(reference.as_ref()),
        }
    }
}
