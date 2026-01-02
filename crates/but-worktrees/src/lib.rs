//! Worktree management types and database operations.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use bstr::BString;
use serde::{Deserialize, Serialize};

pub(crate) mod db;
pub mod destroy;
pub(crate) mod git;
pub mod integrate;
pub mod list;
pub mod new;

/// A worktree name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorktreeId(BString);

impl WorktreeId {
    /// Create a new worktree ID using a random UUID.
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
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

        Ok(Self(gix::path::os_str_into_bstr(basename)?.to_owned()))
    }

    fn as_bstr(&self) -> &bstr::BStr {
        self.0.as_ref()
    }

    /// Useful for lossless calls to Git.
    pub(crate) fn to_os_str(&self) -> OsString {
        gix::path::from_bstr(self.as_bstr()).into_owned().into()
    }
}

impl std::fmt::Display for WorktreeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_bstr().fmt(f)
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
    pub created_from_ref: Option<gix::refs::FullName>,
    /// The base which we will use in a cherry-pick.
    pub base: Option<gix::ObjectId>,
}
