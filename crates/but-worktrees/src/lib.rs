//! Worktree management types and database operations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Database operations for worktrees.
pub mod db;

/// The source from which a worktree was created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum WorktreeSource {
    /// The worktree was created from a branch reference.
    Branch(String),
}

/// A worktree entry representing a Git worktree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worktree {
    /// Unique identifier for this worktree.
    pub id: Uuid,
    /// The base commit (ObjectId) from which this worktree was created.
    pub base: gix::ObjectId,
    /// The filesystem path to the worktree.
    pub path: PathBuf,
    /// The source from which this worktree was created.
    pub source: WorktreeSource,
}
