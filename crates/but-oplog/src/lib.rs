//! The operations log, short: Oplog, is a sequence of restore points, which each restore point implemented as snapshots.
//! Snapshots contain enough information to restore what we are interested in.
//!
//! ### Snapshots
//!
//! For simplicity, snapshots of "what we are interested in" is all state that contributes to the GitButler experience, such as:
//!
//! * where `HEAD` points to
//! * the position of all references in the Workspace
//!     - the positions of stash references
//! * metadata stored in the database
//!
//! Note the absence of uncommitted and untracked files, as it's something we'd not want to track for the user - they have to do this
//! themselves and voluntarily via stashes. We only manipulate data that is stored in Git or that is owned and maintained by GitButler.
//!
//! It's controlled by the caller which of these (or parts) are stored and to what extent just as an optimisation.
//!
//! ### The Log
//!
//! Snapshots are merely Git trees that contain information, and they can be arranged using commits to bring them in order, and to
//! attach metadata about the operation that created them.
//!
//! This is where the Log part comes into play, as it associates commits with these trees, and the parent-child relationships of these commits
//! form the log.
//!
//! #### Undo/Redo
//!
//! Undo can be implemented by keeping track of which position in the Log we are currently in using its Head, and by restoring all relevant state
//! to the snapshot it's pointing to. To support a redo, and if this is the most recent entry in the Log as pointed to by its tip, we'd have to
//! create another snapshot to capture that state so there is something to go back to.
//!
//! From that point on, one can move freely through the log backwards and forwards. Worktree changes complicate this, because it's unclear to me to what
//! extent we should even handle them - my take is to not keep them in oplog snapshots at all and force the user to stash them away.
//!
//! When creating a new snapshot while the head isn't at the tip of the log, for simplicity we "forget" the snapshots between the head and the tip, and
//! move the tip to the new snapshot instead.
//!
//! ### Mode of operation: Side Effect
//!
//! When applying a mutation to the repository, the oplog runs as a side effect. The *effect* itself changes state, and is expected to either fully succeed,
//! or leave no trace of ever running.
//!
//! This means that snapshots have to be recorded in such a way that they can't be observed until they are committed, which happens only when the
//! *effect* succeeds. On failure, there is no entry in the oplog.
//!
//! ### Status of the implementation
//!
//! Right now it's merely a letter of intent with an API sketch that is sufficient to 'record but not persist unless command is successful'.
//! Note that the `gitbutler-oplog` is still the backbone of this implementation, but modified to the extent necessary.
//! Legacy commands will always see their restore point created as their failure might leave partial changes that might still be undoable
//! with the restore point.
//!
//! Non-legacy commands can use the [`UnmaterializedOplogSnapshot`] utility to insert the snapshot into the log on successful effects.
#![forbid(unsafe_code, missing_docs)]

/// This is just a sketch for an in-memory snapshot that isn't observable through the on-disk repository.
/// It will be committed only if the main effect of a function was successfully applied.
/// This works only if that effect is known to only apply in full, or not at all (at least in 99.9% of the cases).
///
/// NOTE: if this utility type should really take a `Context` as parameter, it should be in `but-api`.
pub struct UnmaterializedOplogSnapshot {
    /// The tree containing all snapshot information.
    #[cfg(feature = "legacy")]
    tree_id: gix::ObjectId,
    /// Details to pass when committing the snapshot.
    #[cfg(feature = "legacy")]
    details: gitbutler_oplog::entry::SnapshotDetails,
}

/// legacy types for easy of use, all provided by `gitbutler-oplog`.
#[cfg(feature = "legacy")]
pub mod legacy {
    pub use gitbutler_oplog::entry::{OperationKind, SnapshotDetails, Trailer};
}

#[cfg(feature = "legacy")]
mod oplog_snapshot {
    use but_oxidize::{ObjectIdExt, OidExt};
    use gitbutler_oplog::OplogExt;

    use crate::UnmaterializedOplogSnapshot;

    /// Lifecycle
    impl UnmaterializedOplogSnapshot {
        /// Create a new instance from `details`, which is a snapshot that isn't committed to the oplog yet.
        /// This fails if the snapshot creation fails.
        pub fn from_details(
            ctx: &but_ctx::Context,
            details: gitbutler_oplog::entry::SnapshotDetails,
        ) -> anyhow::Result<Self> {
            // TODO: these guards are probably something to remove as they don't belong into a plumbing crate, neither does Context.
            let guard = ctx.shared_worktree_access();
            let tree_id = ctx.prepare_snapshot(guard.read_permission())?.to_gix();
            Ok(Self { tree_id, details })
        }
    }

    impl UnmaterializedOplogSnapshot {
        /// Call this method only if the main effect succeeded so the snapshot should be added to the operation log.
        pub fn commit(self, ctx: &but_ctx::Context) -> anyhow::Result<()> {
            let mut guard = ctx.exclusive_worktree_access();
            let _commit_id = ctx.commit_snapshot(
                self.tree_id.to_git2(),
                self.details,
                guard.write_permission(),
            )?;
            Ok(())
        }
    }
}
