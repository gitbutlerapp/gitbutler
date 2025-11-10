//! Worktree management types and database operations.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bstr::{BString, ByteSlice};
use serde::{Deserialize, Serialize};

pub(crate) mod db;
pub mod destroy;
pub(crate) mod git;
pub mod integrate;
pub mod list;
pub mod new;

/// A worktree name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorktreeId(#[serde(with = "gitbutler_serde::bstring_lossy")] BString);

impl WorktreeId {
    /// Create a new worktree ID using a random UUID.
    pub fn new() -> Self {
        Self(BString::from(uuid::Uuid::new_v4().to_string()))
    }

    /// Create a worktree ID from a string.
    pub fn from_bstr(s: impl Into<BString>) -> Self {
        Self(s.into())
    }

    /// Extract the worktree ID from a path.
    ///
    /// Takes the basename of the path as the worktree name.
    pub fn from_path(path: &Path) -> Result<Self> {
        let basename = path
            .file_name()
            .context("Invalid worktree path - no filename")?;

        Ok(Self(BString::from(basename.to_string_lossy().as_ref())))
    }

    /// Get the worktree name as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }

    /// Get the inner BString.
    pub fn as_bstr(&self) -> &bstr::BStr {
        self.0.as_ref()
    }
}

impl Default for WorktreeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents some extra metadata that can be associated with a worktree
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorktreeMeta {
    /// The worktree identifier.
    pub id: WorktreeId,
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
    /// The worktree identifier.
    pub id: WorktreeId,
    /// The canonicalized filesystem path to the worktree.
    pub path: PathBuf,
    /// The git reference this worktree was created from.
    #[serde(with = "gitbutler_serde::fullname_opt")]
    pub created_from_ref: Option<gix::refs::FullName>,
    /// The base which we will use in a cherry-pick.
    #[serde(with = "gitbutler_serde::object_id_opt")]
    pub base: Option<gix::ObjectId>,
}
