/// Failures specific to performing a graph rebase.
///
/// Operational failures retain their original source while graph-specific
/// failures remain directly matchable by callers.
#[derive(Debug, thiserror::Error)]
pub enum RebaseError {
    /// The editor graph violated an invariant required to perform the rebase.
    #[error("{message}")]
    InvalidGraph {
        /// The violated invariant.
        message: &'static str,
    },
    /// A commit could not be cherry-picked onto its new parents.
    #[error("Failed to cherry-pick commit {commit_id}")]
    CherryPick {
        /// The commit being picked.
        commit_id: gix::ObjectId,
        /// The underlying cherry-pick failure.
        #[source]
        source: anyhow::Error,
    },
    /// A commit that must remain conflict-free produced conflicts.
    #[error(
        "Commit {commit_id} was marked as not conflictable, but resulted in a conflicted state"
    )]
    NonConflictableCommitConflicted {
        /// The commit that produced conflicts.
        commit_id: gix::ObjectId,
    },
    /// The synthetic bases required for a merge commit could not be merged.
    #[error("{message}")]
    FailedToMergeBases {
        /// The merge commit being picked.
        commit_id: gix::ObjectId,
        /// Whether merging the original bases failed.
        base_merge_failed: bool,
        /// The original merge bases, when available.
        bases: Option<Vec<gix::ObjectId>>,
        /// Whether merging the new parents failed.
        onto_merge_failed: bool,
        /// The new parents, when available.
        ontos: Option<Vec<gix::ObjectId>>,
        /// The detailed human-readable failure description.
        message: String,
    },
    /// Looking up a reference to update failed.
    #[error("Failed to find reference {refname} while rebasing")]
    FindReference {
        /// The reference being looked up.
        refname: gix::refs::FullName,
        /// The underlying reference lookup failure.
        #[source]
        source: anyhow::Error,
    },
    /// Updating a symbolic reference is not supported by graph rebase.
    #[error("Attempted to update the symbolic reference {name}")]
    SymbolicReferenceUpdate {
        /// The symbolic reference target.
        name: gix::refs::FullName,
    },
}
