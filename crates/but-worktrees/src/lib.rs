//! Worktree management types and database operations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Database operations for worktrees.
pub mod db;
pub mod git;
pub mod new;

/// The source from which a worktree was created.
/// This gets exposed in public JSON outputs, be careful when removing
/// properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum WorktreeSource {
    /// The worktree was created from a branch. This is the given name of the
    /// branch
    Branch(String),
}

/// A worktree entry representing a Git worktree.
/// This gets exposed in public JSON outputs, be careful when removing
/// properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    /// The filesystem path to the worktree.
    pub path: PathBuf,
    /// The git reference this worktree was created from. This is a fully
    /// qualified reference
    pub reference: String,
    /// The base commit (ObjectId) from which this worktree was created.
    pub base: gix::ObjectId,
    /// The source from which this worktree was created.
    pub source: WorktreeSource,
}
