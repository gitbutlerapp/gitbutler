/// Provides data that helps describe the effect of the move changes operaiton.
pub struct MoveChangesResult {
    /// A list of commits that were replaced as part of any rebases that were
    /// performed. Provided as a list of tuples where the first item in the
    /// tuple is the "before" and the second item in the tuple is the "after"
    /// id.
    ///
    /// If a commit was unaffected then it will not be included in this list.
    pub replaced_commits: Vec<(gix::ObjectId, gix::ObjectId)>,
}

pub(super) mod discard_worktree_changes;
pub(super) mod move_between_commits;

mod file;
pub(crate) mod hunk;
mod utils;
