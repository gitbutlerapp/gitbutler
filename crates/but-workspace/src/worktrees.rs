//! Listing and metadata operations for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration and archived-state reconciliation happen in `but-ctx` - callers pass
//! the result in as [`WorktreeSource`]s so this crate stays independent of it.

use std::path::PathBuf;

use anyhow::Context as _;
use bstr::{BStr, BString};
use gix::prelude::ObjectIdExt as _;
use serde::Serialize;

/// A non-archived linked worktree, presented like a single-branch stack.
///
/// This is intentionally slimmer than a workspace stack - linked worktrees have no
/// push status or remote tracking information of their own.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStack {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub name: BString,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_lossy_opt")
    )]
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub head: gix::ObjectId,
    /// The merge base between [`Self::head`] and the target, if both exist.
    #[serde(with = "but_serde::object_id_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id_opt")
    )]
    pub base: Option<gix::ObjectId>,
    /// The commits reachable from the first parent of [`Self::head`] but not from the
    /// target, or empty when no target is known.
    pub commits: Vec<crate::ui::Commit>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorktreeStack);

/// An archived linked worktree, listed with identity information only.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ArchivedWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub name: BString,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    #[serde(with = "but_serde::fullname_lossy_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_lossy_opt")
    )]
    pub ref_name: Option<gix::refs::FullName>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ArchivedWorktree);

/// All usable linked worktrees, separated by archived state.
#[derive(Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListing {
    /// Non-archived worktrees, with their commits against the target.
    pub active: Vec<WorktreeStack>,
    /// Archived worktrees, hidden from the workspace but still on disk.
    pub archived: Vec<ArchivedWorktree>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorktreeListing);

/// A usable linked worktree as input to [`list_worktrees()`].
///
/// Callers typically map this from `but-ctx`'s reconciled worktree enumeration.
#[derive(Debug, Clone)]
pub struct WorktreeSource {
    /// Whether the worktree is archived.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The worktree `HEAD` with the stable worktree name.
    pub tip: but_graph::init::WorktreeTip,
}

/// Produce a listing of all worktrees in `sources`, splitting them by archived state.
///
/// Active worktrees get their commits computed as a first-parent walk of their `HEAD`,
/// hidden behind `target_id`, plus the merge base with the target.
/// Without a `target_id` there is no lower bound to stop that walk at - it could run
/// to the root of history - so the commit list degrades to empty and the base to `None`.
/// Merge-base failures (e.g. disjoint histories) also degrade the base to `None`.
pub fn list_worktrees(
    repo: &gix::Repository,
    sources: Vec<WorktreeSource>,
    target_id: Option<gix::ObjectId>,
) -> anyhow::Result<WorktreeListing> {
    let mut active = Vec::new();
    let mut archived = Vec::new();
    for source in sources {
        let WorktreeSource {
            archived: is_archived,
            path,
            tip,
        } = source;
        if is_archived {
            archived.push(ArchivedWorktree {
                name: tip.name,
                path,
                ref_name: tip.ref_name,
            });
            continue;
        }
        let head = tip.id;
        let (base, commits) = match target_id {
            Some(target_id) => {
                let base = repo
                    .merge_base(head, target_id)
                    .ok()
                    .map(|base| base.detach());
                let commits = crate::local_commits_for_branch(head.attach(repo), target_id)?;
                (base, commits)
            }
            None => (None, Vec::new()),
        };
        active.push(WorktreeStack {
            name: tip.name,
            path,
            ref_name: tip.ref_name,
            head,
            base,
            commits,
        });
    }
    Ok(WorktreeListing { active, archived })
}

/// Persist the archived state of the worktree named `name`.
///
/// This is an upsert - it also adopts worktrees the reconciliation hasn't seen yet.
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

/// Open the linked worktree named `name` as a from-disk repository sharing
/// `repo`'s object database and ref store.
///
/// This fails early for unknown names and for pruned or inaccessible checkouts.
/// Objects written through the returned repository land in the shared object
/// database, so they are immediately visible to other repository handles.
pub fn open_worktree_repo(repo: &gix::Repository, name: &BStr) -> anyhow::Result<gix::Repository> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    proxy
        .into_repo()
        .with_context(|| format!("Worktree {name} is not accessible"))
}

/// Compute the uncommitted changes in the linked worktree named `name`.
///
/// The worktree repository is opened from disk, which fails early for pruned or
/// inaccessible checkouts. Unlike the main-workspace variant, no hunk assignments
/// or dependencies are computed - those are workspace concepts.
pub fn worktree_changes_by_name(
    repo: &gix::Repository,
    name: &BStr,
) -> anyhow::Result<but_core::ui::WorktreeChanges> {
    let wt_repo = open_worktree_repo(repo, name)?;
    Ok(but_core::diff::worktree_changes(&wt_repo)?.into())
}
