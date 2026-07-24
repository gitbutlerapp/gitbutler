//! Listing and metadata operations for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//!
//! Enumeration and archived-state reconciliation are centralized in `but-ctx` -
//! callers pass the result in as [`WorktreeSource`]s so this crate stays independent
//! of it. The `worktree_meta` table only stores *explicitly set* archived state
//! plus the one-time adoption marker; a worktree without a row is active.

use std::path::PathBuf;

use anyhow::Context as _;
use bstr::{BStr, BString};
use serde::Serialize;

/// A non-archived linked worktree, presented like a single-branch stack.
///
/// This is intentionally slimmer than a workspace stack - linked worktrees have no
/// push status or remote tracking information of their own, and their commits
/// against the target are not computed yet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStack {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    #[serde(with = "but_serde::object_id")]
    pub head: gix::ObjectId,
}

/// An archived linked worktree, listed with identity information only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// The worktree checkout directory.
    #[serde(with = "but_serde::path_lossy")]
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    pub ref_name: Option<gix::refs::FullName>,
}

/// All usable linked worktrees, separated by archived state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListing {
    /// Non-archived worktrees.
    pub active: Vec<WorktreeStack>,
    /// Archived worktrees, hidden from the workspace but still on disk.
    pub archived: Vec<ArchivedWorktree>,
}

/// A usable linked worktree as input to [`list_worktrees()`].
///
/// Callers typically map this from `but-ctx`'s reconciled worktree enumeration.
#[derive(Debug, Clone)]
pub struct WorktreeSource {
    /// Whether the worktree is archived.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub head: gix::ObjectId,
}

/// Produce a listing of all worktrees in `sources`, splitting them by archived state.
pub fn list_worktrees(sources: Vec<WorktreeSource>) -> WorktreeListing {
    let mut active = Vec::new();
    let mut archived = Vec::new();
    for source in sources {
        let WorktreeSource {
            archived: is_archived,
            path,
            name,
            ref_name,
            head,
        } = source;
        if is_archived {
            archived.push(ArchivedWorktree {
                name,
                path,
                ref_name,
            });
        } else {
            active.push(WorktreeStack {
                name,
                path,
                ref_name,
                head,
            });
        }
    }
    WorktreeListing { active, archived }
}

/// Open the linked worktree named `name` as a from-disk repository.
///
/// It shares `repo`'s object database and has no object memory, so objects written
/// through it land loose on disk and are immediately visible to in-memory
/// repositories built on the same database - which is what makes it usable as the
/// source repository of a worktree-sourced commit or amend.
pub fn open_worktree_repo(repo: &gix::Repository, name: &BStr) -> anyhow::Result<gix::Repository> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    proxy.into_repo().map_err(Into::into)
}

/// Persist the archived state of the worktree named `name`.
///
/// This is an upsert - a worktree without a row is simply active, so archiving
/// creates its row on demand.
pub fn set_worktree_archived(
    db: &mut but_db::DbHandle,
    name: &BStr,
    archived: bool,
) -> anyhow::Result<()> {
    db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
        name: name.to_vec(),
        archived,
    })?;
    Ok(())
}
