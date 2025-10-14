//! Worktree management types and database operations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Database operations for worktrees.
pub(crate) mod db;
pub(crate) mod gc;
pub(crate) mod git;
pub mod list;
pub mod new;

/// The source from which a worktree was created.
/// This gets exposed in public JSON outputs, be careful when removing
/// properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum WorktreeSource {
    /// The worktree was created from a branch. This is the given name of the
    /// branch
    Branch(gix::refs::PartialName),
}

/// A worktree entry representing a Git worktree.
/// This gets exposed in public JSON outputs, be careful when removing
/// properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    /// The canonicalized filesystem path to the worktree.
    pub path: PathBuf,
    /// The git reference this worktree was created from. This is a fully
    /// qualified reference
    pub reference: gix::refs::FullName,
    /// The base commit (ObjectId) from which this worktree was created.
    pub base: gix::ObjectId,
    /// The source from which this worktree was created.
    pub source: WorktreeSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub enum WorktreeHealthStatus {
    /// The worktree is in a healthy state
    Normal,
    /// The worktree has a different branch checked out than expected
    /// This is not strictly an issue & could be an intened user state
    BranchMissing,
    /// The branch we expect to be checked out does not exist
    /// This is not strictly an issue & could be an intened user state
    BranchNotCheckedOut,
    /// The actual worktree doesn't exist - should GC
    WorktreeMissing,
    /// No cooresponding branch name in workspace - should GC
    WorkspaceBranchMissing,
}
