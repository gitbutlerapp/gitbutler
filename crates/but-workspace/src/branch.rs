#![allow(unused_variables)]
//! What follows is a description of all entities we need to implement Workspaces,
//! a set of *two or more* branches that are merged together.
//!
//! ## Operations
//!
//! * **add branch from workspace**
//!    - Add a branch to a *workspace commit* so it also includes the tip of the branch.
//!    - Alternatively, checkout the branch if it is the first branch and a *target branch* is known.
//! * **remove branch from workspace**
//!    - Remove a branch from a *workspace commit*
//!    - Alternatively, checkout the *target branch*
//! * **switch**
//!    - Switch between branches of any kind, similar to `git switch`, but with automatic worktree stashing.
//! * **stash changes**
//!    - Implicit, causes worktree changes to be raised into their own tree and commit on top of `HEAD`.
//! * **apply stashed changes**
//!    - Lower previously stashed changes by re-applying them to a possibly changed tree, and checking them
//!      out to the working tree.
//!
//! ## Metadata
//!
//! Metadata is all data that doesn't refer to Git objects (i.e. anything with a SHA1), and can be associated with
//! a full reference name.
//!
//! ## Target Branch
//!
//! The branch that every branch in a workspace wants to get integrated with,
//! either by merging or by being rebased onto it.
//!
//! The target branch, despite being checked out, is not a preferred destination for new commits
//! - these should go into a branch, but there is nothing here that would enforce this.
//!
//! ## Workspace Tip
//!
//! Is the elaborate name of the commit that currently represents what's visible in the working tree,
//! i.e. the commit that `HEAD` points to. Workspace changes are expected to be on top of this commit.
//!
//! `HEAD` can point to Git commits, and to *Workspace Commits*, and the user switches `HEAD` locations
//! at will with any tool of choice.
//!
//! ## Workspace Commits
//!
//! Whenever there are more than one branch to show, a *workspace commit* is created to keep track of the branches
//! that are contained in it, making it *always a merge commit*.
//!
//! ### Workspace References
//!
//! Whenever a workspace is officially created by means of a *workspace commit*, a `refs/heads/gitbutler/workspace/<name>`
//! is also created to point to it.
//! These are removed once the corresponding *workspace commit* is going out of scope.
//!
//! *This also makes workspaces enumerable*.
//!
//! ## Worktree Stashes via Stash Commits
//!
//! Whenever there are worktree changes present before switching `HEAD` to another location,
//! we will leave them if they don't interfere with the new `HEAD^{tree}` that we are about switch to.
//! Those that do interfere we pick up and place as special *stash commit* on top of the commit
//! that can accommodate them and let a *stash reference* point to it.
//!
//! ### Stash References
//!
//! The *stash reference* is created in the *gitbutler-stashes* [Git namespace](gix::refs::file::Store::namespace) to
//! fully isolate them from the original reference name. This also means that we should disallow renaming branches with
//! an associated *stash reference* and make it possible to list orphaned stashes as well, along with recovery options.
//!
//! For instance, if a references is `refs/heads/gitbutler/workspace/one`, then the stash reference is in
//! `<namespace: gitbutler-stashes>/refs/heads/gitbutler/workspace/one`. The reason for this is that they will not
//! be garbage-collected that way.
//!
//! *This also makes stashes enumerable*. It's notable that it's entirely possible for stashes to become *orphaned*,
//! i.e. their workspace commit (that the stash commit sits on top of) doesn't have a reference *with the same name*
//! pointing to it anymore. What's great is that these can easily be recovered, along with the stash
//! as it's trivial to find the *workspace commit* as only parent of the *stash commit*.
//!
//! ### Raising a Stash
//!
//! This is the process of knowing what to put in a stash in the first place before changing the worktree.
//! The idea is to *minimize* the stash and leave everything that isn't going to be touched by the final checkout in place.
//! That way what's in the stash is only the files that need to be merged back, minimizing the chance for conflicts.
//!
//! ### Apply Stashed Changes/Lowering a Stash
//!
//! Stashed changes are cherry-picked onto the target tree, but *merge conflicts* have to be kept and checked out
//! to manifest them, similar to what happens when a Git stash is popped.
//!
//! If the destination of an unapply operation already has a stash, both source and destination stash will be merged
//! with merge-conflicts manifesting in the tree.
//!
//! ### Shortcomings
//!
//! * **changed indices prevent stash creation**
//!     - If changes in the index would be ignored, data stored only there would be lost.
//!     - conflicts would be lost, and should probably prevent the stash in any case.
//! * **complexities with multiple worktree stashes**
//!     - The user can leave a stash in each worktree that they switch away from.
//!     - When switching back to a commit with stash we now have to deal with two stashes, the one that was already
//!       there and the one we newly created before switching. Depending on the operation we may have to merge both
//!       stashes, which can conflict.
//!
//! ## Sketches
//!
//!
//! ```text
//! ┌───┐     ███████████████████████ Apply ████████│ No Worktree Changes  │████████  Unapply █████████████████████████████████
//! │   │                                           └──────────────────────┘
//! │   │
//! │   │
//! │   │
//! │ T │
//! │ a │
//! │ r │
//! │ g │      Target                                 Target        HEAD                           Target
//! │ e │         │                                      │            │                               │
//! │ t │         ▼                                      ▼            ▼                               ▼
//! │   │        ┌─┐          ┌─┐                       ┌─┐          ┌─┐                             ┌─┐          ┌─┐
//! │   │ HEAD ─▶└─┘          └─┘◀── Feature            └─┘          └─┘◀── Feature           HEAD ─▶└─┘          └─┘◀── Feature
//! │   │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │        ┌─┐           │                        ┌─┐           │                              ┌─┐           │
//! │   │        └─┘───────────┘                        └─┘───────────┘                              └─┘───────────┘
//! │ N │
//! │ o │
//! │   │                                                     ┌──┐
//! │ T │                                             HEAD ──▶│WS│ ◀── ws/1
//! │ a │                                                     └──┘
//! │ r │       main                                            │                                  main
//! │ g │         │                                      ┌──────┴─────┐                              │
//! │ e │         ▼                                      │            │                              ▼
//! │ t │        ┌─┐          ┌─┐                       ┌─┐          ┌─┐                            ┌─┐          ┌─┐
//! │   │ HEAD──▶└─┘          └─┘◀── Feature    main ──▶└─┘          └─┘◀── Feature          HEAD──▶└─┘          └─┘◀── Feature
//! │   │         │            │                         │            │                              │            │
//! │   │         │            │                         │            │                              │            │
//! │   │         │            │                         │            │                              │            │
//! │   │        ┌─┐           │                        ┌─┐           │                             ┌─┐           │
//! └───┘        └─┘───────────┘                        └─┘───────────┘                             └─┘───────────┘
//!
//!
//!
//!
//!
//!                                                 ┌──────────────────────┐
//! ┌───┐     ███████████████████████ Apply ████████│ No Worktree Changes  │████████ Apply ████████████████████████████████████
//! │   │                                           └──────────────────────┘
//! │   │
//! │   │                                                                                                   ┌──┐
//! │   │                                                                                            HEAD ─▶│WS│◀─ ws/1
//! │ T │                                                                                                   └──┘
//! │ a │                                                 ┌──HEAD                                             │
//! │ r │                                                 │                                                   │
//! │ g │      Target    F1    F2                   Target│   F1    F2                          Target     ┌──┴──┐
//! │ e │         │       │     │                      │  │    │     │                             │       │     │
//! │ t │         ▼       ▼     ▼                      ▼  │    ▼     ▼                             ▼       │     │
//! │   │        ┌─┐     ┌─┐   ┌─┐                    ┌─┐ │   ┌─┐   ┌─┐                           ┌─┐     ┌─┐   ┌─┐
//! │   │ HEAD ─▶└─┘     └─┘   └─┘                    └─┘ └──▶└─┘   └─┘                           └─┘  ┌─▶└─┘   └─┘◀── F2
//! │   │         │       │     │                      │       │     │                             │   │   │     │
//! │   │         │       │     │                      │       │     │                             │   │   │     │
//! │   │         │       │     │                      │       │     │                             │  F1   │     │
//! │   │        ┌─┐      │     │                     ┌─┐      │     │                            ┌─┐      │     │
//! │   │        └─┘──────┴─────┘                     └─┘──────┴─────┘                            └─┘──────┴─────┘
//! │ N │
//! │ o │
//! │   │                                                ┌──┐                                            ┌──┐
//! │ T │                                         HEAD ─▶│WS│◀─ ws/1                             HEAD ──▶│WS│◀── ws/1
//! │ a │                                                └──┘                                            └──┘
//! │ r │                F1    F2                          │        F2                                     ▲
//! │ g │                 │     │                      ┌───┴───┐     │                             ┌───────┼─────┐
//! │ e │                 ▼     ▼                      │       │     ▼                             │       │     │
//! │ t │        ┌─┐     ┌─┐   ┌─┐                    ┌─┐     ┌─┐   ┌─┐                           ┌─┐     ┌─┐   ┌─┐
//! │   │ HEAD ─▶└─┘     └─┘   └─┘                    └─┘  ┌─▶└─┘   └─┘                           └─┘  ┌─▶└─┘   └─┘◀─── F2
//! │   │         │       │     │                      │   │   │     │                             │   │   │     │
//! │   │         │       │     │                      │  F1   │     │                             │  F1   │     │
//! │   │         │       │     │                      │       │     │                             │       │     │
//! │   │        ┌─┐      │     │                     ┌─┐      │     │                            ┌─┐      │     │
//! └───┘        └─┘──────┴─────┘                     └─┘──────┴─────┘                            └─┘──────┴─────┘
//!
//!
//!
//!
//!                                                 ┌──────────────────────┐
//! ┌───┐     ███████████████████████ Apply ████████│Worktree Changes (WTC)│████████  Unapply █████████████████████████████████
//! │   │                                           └──────────────────────┘
//! │   │
//! │   │
//! │   │
//! │ T │
//! │ a │                                               ┌─┐                                                       ┌─┐
//! │ r │                                      [WTC] ──▶└─┘                                                       └─┘◀──[WTC2]
//! │ g │     Target (WTC)                               │          HEAD                          Target (WTC)     │
//! │ e │         │                                      │            │                               │            │
//! │ t │         ▼                                      │            ▼                               ▼            │
//! │   │        ┌─┐          ┌─┐                       ┌─┐          ┌─┐                             ┌─┐          ┌─┐
//! │   │ HEAD ─▶└─┘          └─┘◀── Feature  Target ──▶└─┘          └─┘◀──Feature (WTC2)     HEAD ─▶└─┘          └─┘◀── Feature
//! │   │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │        ┌─┐           │                        ┌─┐           │                              ┌─┐           │
//! │   │        └─┘───────────┘                        └─┘───────────┘                              └─┘───────────┘
//! │ N │
//! │ o │
//! │   │                                                     ┌──┐
//! │ T │                                       ┌─┐   ws/1 ──▶│WS│◀──HEAD (WTC2)
//! │ a │                               [WTC]──▶└─┘           └──┘
//! │ r │     main (WTC)                         │              │                                main (WTC + WTC2)
//! │ g │         │                              └───────┬──────┴─────┐                              │
//! │ e │         ▼                                      │            │                              ▼
//! │ t │        ┌─┐          ┌─┐                       ┌─┐          ┌─┐                            ┌─┐          ┌─┐
//! │   │ HEAD──▶└─┘          └─┘◀── Feature    main ──▶└─┘          └─┘◀── Feature          HEAD──▶└─┘          └─┘◀── Feature
//! │   │         │            │                         │            │                              │            │
//! │   │         │            │                         │            │                              │            │
//! │   │         │            │                         │            │                              │            │
//! │   │        ┌─┐           │                        ┌─┐           │                             ┌─┐           │
//! └───┘        └─┘───────────┘                        └─┘───────────┘                             └─┘───────────┘
//!
//!
//!                                                                                              main (WTC)      ┌─┐
//!                                                                                                  │           └─┘◀──[WTC2]
//!                                                                                                  │            │
//!                                                ┌───────────────────────┐                         ▼            │
//!                                                │          OR           │                        ┌─┐          ┌─┐
//!                                                │     depending on      │                 HEAD──▶└─┘          └─┘◀── Feature
//!                                                │  change-association   │                         │            │
//!                                                │          OR           │                         │            │
//!                                                └───────────────────────┘                        ┌─┐           │
//!                                                                                                 └─┘───────────┘
//! ```
use anyhow::{bail, Context};
use gitbutler_stack::VirtualBranchesState;
use gix::prelude::ObjectIdExt;

/// The result of [`add_branch_to_workspace`].
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// The new location of the workspace tip if the existing one was moved, or if a new one was created.
    /// This is equivalent to where `HEAD` should be.
    pub workspace_tip: gix::ObjectId,
    /// What to do with `workspace_tip`.
    pub operation: WorkspaceTipOperation,
    /// If we rebased something, this is the result of that operation.
    /// If `None`, this means `branch_id` was checked out.
    pub rebase_output: Option<but_rebase::RebaseOutput>,
}

/// What to do with the `workspace_tip` of an [`ApplyOutcome`].
#[derive(Debug, Copy, Clone)]
pub enum WorkspaceTipOperation {
    /// Checkout the given tip.
    Checkout,
}

/// Apply the given `branch_tip` so that it is part of the given `workspace_tip`,
/// which may also be initialized with `HEAD` and for all intends and purposes is equivalent to what `HEAD` points to.
/// After the operation, `branch_tip` should be part of the returned `workspace_tip`
/// If not available, a new tip will be provided which may be the commit the `branch` is currently pointing to,
/// indicating the workspace tip is now just the tip of an ordinary branch.
/// `target_tip`, if present, is the tip of the branch to integrate all workspace branches with ultimatley.
///
/// Note that at this point, the worktree will not have been touched, nor will references have been updated.
/// However, the `vb` will be updated where appropriate to match new ref positions in case the `branch` had to be
/// rebased.
pub fn add_branch_to_workspace(
    repo: &gix::Repository,
    branch_tip: gix::ObjectId,
    workspace_tip: gix::ObjectId,
    target_tip: Option<gix::ObjectId>,
    vb: &mut VirtualBranchesState,
) -> anyhow::Result<ApplyOutcome> {
    if crate::WorkspaceCommit::from_id(branch_tip.attach(repo))?.is_managed() {
        bail!("Cannot bring a workspace into another one")
    }

    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let merge_base = repo
        .merge_base_with_graph(workspace_tip, branch_tip, &mut graph)
        .context("Branch and workspace must have a merge-base")?;
    if merge_base == branch_tip || merge_base == workspace_tip {
        return Ok(ApplyOutcome {
            workspace_tip: branch_tip,
            operation: WorkspaceTipOperation::Checkout,
            rebase_output: None,
        });
    }
    todo!()
}

/// Like [`add_branch_to_workspace`], but will also update the involved references, and change `HEAD` to point to possibly newly
/// created `workspace_tip`.
/// `workspace_tip` can also be a detached `HEAD`, that's valid.
// TODO: maybe rather work with refs and add dry-run?
pub fn add_branch_to_workspace_and_update_refs(
    repo: &gix::Repository,
    branch_tip: gix::refs::PartialName,
    workspace_tip: gix::refs::FullName,
    target_tip: Option<gix::refs::PartialName>,
    vb: &mut VirtualBranchesState,
) -> anyhow::Result<ApplyOutcome> {
    todo!()
}

/// The inverse of [`add_branch_to_workspace`] where `workspace_tip` is the current `HEAD` and `branch_tip` is the tip to *not* include in the
/// `workspace_tip` anymore.
pub fn remove_branch_from_workspace(
    repo: &gix::Repository,
    branch_tip: gix::ObjectId,
    workspace_tip: gix::ObjectId,
    target_tip: Option<gix::ObjectId>,
    vb: &mut VirtualBranchesState,
) -> anyhow::Result<ApplyOutcome> {
    todo!()
}
