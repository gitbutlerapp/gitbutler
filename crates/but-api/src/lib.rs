//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
#![cfg_attr(not(feature = "napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "napi", deny(unsafe_code))]
#![deny(missing_docs)]

use std::collections::BTreeMap;

#[cfg(not(feature = "graph-workspace"))]
use but_workspace::RefInfo;

#[cfg(feature = "legacy")]
pub mod legacy;

/// Functions for GitHub authentication.
pub mod github;

/// Functions for GitLab authentication.
pub mod gitlab;

/// Functions for Bitbucket authentication.
pub mod bitbucket;

/// Functions that take a branch as input.
pub mod branch;

/// Functions that operate on the workspace.
pub mod workspace;

/// Listing target-branch history relative to the workspace.
pub mod target_commits;

/// Land a branch directly onto the target ref (the "avoid pull requests" workflow).
///
/// Gated behind `legacy` for now because the real-remote push relies on the legacy `gitbutler-git`
/// push helper; the boundary itself is modern (ref input, [`WorkspaceState`] output).
#[cfg(feature = "legacy")]
pub mod land;

/// Ephemeral comments anchored to lines in diffs, shared between the GUI and the CLI.
pub mod comments;

/// Functions that operate commits
pub mod commit;

/// Functions that operate on linked git worktrees (experimental).
pub mod worktrees;

/// Resolve the conflicts of a conflicted commit with an LLM.
pub mod resolve;

/// Functions that show what changed in various Git entities, like trees, commits and the worktree.
pub mod diff;

/// Types meant to be serialised to JSON, without degenerating information despite the need to be UTF-8 encodable.
/// EXPERIMENTAL
pub mod json;

/// Functions releated to platform detection and information.
pub mod platform;

pub mod open;

pub mod panic_capture;

/// The types for watcher events
#[cfg(feature = "export-schema")]
pub mod watcher;

/// The tag vocabulary clients cache API results under.
pub mod tags;

/// Functions for workspace state.
pub mod workspace_state;

/// Represents the workspace for the frontend
///
/// This describes the post-operation workspace view that mutations should
/// return regardless of whether they executed for real or as a dry-run.
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    /// Commits that were replaced by the operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
    /// The workspace presented for the frontend. See [`RefInfo`] for more
    /// detail.
    #[cfg(not(feature = "graph-workspace"))]
    pub head_info: RefInfo,
    /// The workspace presented for the frontend, as the rendered graph
    /// projection. See
    /// [`DetailedGraphWorkspace`](but_workspace::ui::workspace::DetailedGraphWorkspace)
    /// for more detail.
    #[cfg(feature = "graph-workspace")]
    pub graph_workspace: but_workspace::ui::workspace::DetailedGraphWorkspace,
    /// True if a checkout occurred, and a conflict occurred during that
    /// checkout.
    pub checkout_conflict_occurred: bool,
}
