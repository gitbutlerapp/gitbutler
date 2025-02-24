//! A way to represent the graph in a simplified (but more usable) form.
//!
//! This is the current default way of GitButler to perceive its world, but most inexpensively generated to stay
//! close to the source of truth, [The Graph](crate::Graph).
//!
//! These types are not for direct consumption, but should be processed further for consumption by the user.

/// Types related to the stack representation for graphs.
///
/// Note that these are always a simplification, degenerating information, while maintaining a link back to the graph.
mod stack;

pub use stack::{Stack, StackCommit, StackCommitDebugFlags, StackCommitFlags, StackSegment};

pub(crate) mod workspace;
pub use workspace::{TargetCommit, TargetRef, Workspace, WorkspaceKind};

/// utilities for workspace-related commits.
pub mod commit {
    use bstr::{BStr, ByteSlice};

    const GITBUTLER_INTEGRATION_COMMIT_TITLE: &str = "GitButler Integration Commit";
    const GITBUTLER_WORKSPACE_COMMIT_TITLE: &str = "GitButler Workspace Commit";

    /// Return `true` if this `commit_message` indicates a workspace commit managed by GitButler.
    /// If `false`, this is the tip of the stack itself which will be put underneath a *managed* workspace commit
    /// once another branch is added to the workspace.
    pub fn is_managed_workspace_by_message(commit_message: &BStr) -> bool {
        let message = gix::objs::commit::MessageRef::from_bytes(commit_message);
        let title = message.title.trim().as_bstr();
        title == GITBUTLER_INTEGRATION_COMMIT_TITLE || title == GITBUTLER_WORKSPACE_COMMIT_TITLE
    }
}
