use crate::branch::OnWorkspaceMergeConflict;
use but_core::ref_metadata::StackId;
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use std::borrow::Cow;

/// Returned by [unapply()](function::unapply()).
pub struct Outcome<'workspace> {
    /// The newly created workspace, if owned, or the one that was passed in if borrowed, to show how the workspace looks like now.
    ///
    /// If borrowed, the graph already didn't contain the desired branch and nothing had to be unapplied. Note that metadata changes
    /// might not be included in this case, as they aren't the source of truth.
    pub workspace: Cow<'workspace, but_graph::projection::Workspace>,
    /// The unapply operation ended in checking out the last remaining stack in the workspace, whose tip name is listed here.
    /// This will only happen if the commit to check out is named.
    ///
    /// If a remote tracking branch is given to apply, it will actually apply its local tracking branch, which is created on demand as well.
    /// Further, if there is no target or if the current branch isn't the target branch, then the current branch and the given one
    /// will be applied.
    pub checked_out: Option<gix::refs::FullName>,
    /// `true` if we created the given workspace ref as it didn't exist yet.
    pub workspace_ref_created: bool,
    /// If not `None`, an actual merge was attempted, but depending on [the settings](OnWorkspaceMergeConflict),
    /// this was persisted or not.
    pub workspace_merge: Option<crate::commit::merge::Outcome>,
    /// The ids of all stacks that were conflicting and thus didn't get applied, and tip ref names can be derived from that.
    pub conflicting_stack_ids: Vec<StackId>,
}

/// How to treat the workspace merge commit when [unapplying](function::unapply()) it is not technically required anymore.
///
/// This happens when the amount of stacks goes from `2` to `1`.
/// It also happens when a stack is unapplied and the workspace commit only has a single commit merged into it.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum WorkspaceMergeCommit {
    /// Never remove the workspace merge commit. This allows workspaces with 1 stack, or empty workspaces.
    /// can connect directly with the *one* workspace base.
    /// This also ensures that there is a workspace merge commit, even if it is none-sensical.
    #[default]
    Keep,
    /// Remove a workspace merge commit by pointing the workspace reference elsewhere, or
    /// by [deleting](WorkspaceReference) the workspace reference.
    /// Removal happens if the workspace merge commit would only connect to a commit owned by a single stack,
    /// or if there is no stack left at all and the workspace is empty.
    /// Note that workspace commits also don't even have to be present if there are one or more virtual stacks,
    /// as these don't have commits on their own.
    // TODO: make this the default when this is the default in apply()
    RemoveIfPossible,
}

/// Decide what to do with the workspace reference (typically `gitbutler/workspace`) when it's not needed anymore.
///
/// It's not needed anymore when switching away from it to another branch, which may happen only if the workspace commit
/// isn't needed anymore and is [configured](WorkspaceMergeCommit) to be forgotten in that case.
/// *Additionally*, it may only happen if it's a managed reference, i.e. in `refs/heads/gitbutler/`, and if there is only
/// one *named* stack left which then as well may be checked out directly.
#[derive(Default, Debug, Clone)]
pub enum WorkspaceReference {
    /// No matter what, keep the reference for its metadata, *and* keep it checked out.
    #[default]
    KeepCheckedOut,
    /// Keep the reference for its metadata, but allow switching to the last remaining stack.
    KeepButAllowSwitchingToRemainingStack,
    /// Delete the workspace metadata and the workspace reference after switching to the last remaining stack.
    DeleteAfterSwitchingToRemainingStack,
}

/// Options for [branch::unapply()](function::unapply()).
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// How the branch should be brought into the workspace.
    pub workspace_merge_commit: WorkspaceMergeCommit,
    /// Decide how to deal with conflicts when updating the workspace merge commit after removing a stack.
    ///
    /// Note that it should be incredibly unlikely, but can we prove it's impossible?
    pub on_workspace_conflict: OnWorkspaceMergeConflict,
    /// What to do with the workspace reference after unapplying.
    pub workspace_reference: WorkspaceReference,
    /// How the worktree checkout should behave when uncommitted changes are present in the worktree that it would
    /// want to modify to accommodate the new workspace commit, with the unapplied stack removed.
    pub uncommitted_changes: UncommitedWorktreeChanges,
}

pub(crate) mod function {
    use super::{Options, Outcome};
    use but_core::RefMetadata;
    use gix::refs::FullNameRef;

    /// TODO
    pub fn unapply<'ws>(
        branch: &FullNameRef,
        workspace: &'ws but_graph::projection::Workspace,
        repo: &gix::Repository,
        meta: &mut impl RefMetadata,
        opts: Options,
    ) -> anyhow::Result<Outcome<'ws>> {
        todo!()
    }
}
