//! Worktree management types and database operations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) mod db;
pub mod destroy;
pub(crate) mod git;
pub mod integrate;
pub mod list;
pub mod new;

/// Represents some extra metadata that can be associated with a worktree
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorktreeMeta {
    /// The canonicalized filesystem path to the worktree.
    pub path: PathBuf,
    /// The git reference this worktree was created from.
    pub created_from_ref: Option<gix::refs::FullName>,
    /// The base which we will use in a cherry-pick.
    pub base: gix::ObjectId,
}

/// A struct representing worktrees for the frontend
///
/// This gets exposed in public JSON outputs, be careful when removing
/// properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    /// The canonicalized filesystem path to the worktree.
    pub path: PathBuf,
    /// The git reference this worktree was created from.
    pub created_from_ref: Option<gix::refs::FullName>,
    /// The base which we will use in a cherry-pick.
    pub base: Option<gix::ObjectId>,
}
