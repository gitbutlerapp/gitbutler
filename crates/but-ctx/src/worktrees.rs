//! Enumeration of linked worktrees and their archived state.

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Result;
use gix::bstr::BString;

use crate::Context;

/// A usable linked worktree with its archived state and resolved `HEAD`.
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    /// Whether the worktree is hidden from listings and graph traversal.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The worktree `HEAD` as a graph traversal tip, which also carries the
    /// stable worktree name.
    pub tip: but_graph::init::WorktreeTip,
}

impl WorktreeEntry {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub fn name(&self) -> &BString {
        &self.tip.name
    }
}

/// A non-archived linked worktree with its resolved `HEAD`.
#[derive(Debug, Clone)]
pub struct ActiveWorktree {
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The worktree `HEAD` as a graph traversal tip, which also carries the
    /// stable worktree name.
    pub tip: but_graph::init::WorktreeTip,
}

impl ActiveWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub fn name(&self) -> &BString {
        &self.tip.name
    }
}

impl Context {
    /// List all usable linked worktrees with their archived state and resolved `HEAD`s,
    /// reconciling the `worktree_meta` table with the worktrees on disk.
    ///
    /// The first-ever reconciliation of a project archives every worktree existing on
    /// disk at that moment (whether usable or not), assuming they predate GitButler's
    /// worktree support; a sentinel row marks adoption even when no worktree exists yet.
    /// Later reconciliations record unknown worktrees as active. Rows are never pruned -
    /// stale rows are invisible as listings intersect with the worktrees on disk - so a
    /// worktree recreated under a previously archived name stays archived until
    /// explicitly unarchived.
    ///
    /// Worktrees that are broken (pruned checkout, unresolvable `HEAD`), the worktree
    /// of the current repository, and worktrees checked out on the workspace ref are
    /// reconciled but not returned.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn worktrees_with_state(&self) -> Result<Vec<WorktreeEntry>> {
        let repo = self.repo.get()?;
        let (all_names, mut worktrees) = enumerate_worktrees(&repo)?;

        let mut db = self.db.get_cache_mut()?;
        let archived = reconcile(&mut db, &all_names)?;

        for wt in &mut worktrees {
            wt.archived = archived.contains(wt.name());
        }
        Ok(worktrees)
    }

    /// List all non-archived linked worktrees with their resolved `HEAD`s.
    ///
    /// This is [`Self::worktrees_with_state()`] filtered down to active worktrees,
    /// including its reconciliation side-effect; the same caveats apply.
    pub fn active_worktrees(&self) -> Result<Vec<ActiveWorktree>> {
        Ok(self
            .worktrees_with_state()?
            .into_iter()
            .filter(|wt| !wt.archived)
            .map(|wt| ActiveWorktree {
                path: wt.path,
                tip: wt.tip,
            })
            .collect())
    }
}

/// Enumerate the linked worktrees of `repo`, returning the names of ALL of them
/// (for reconciliation - a worktree that is unusable today must still be adopted
/// today, not when it becomes usable) along with the usable entries, excluding the
/// worktree `repo` itself may be opened in. The `archived` state is not yet known
/// and left `false`.
#[allow(clippy::type_complexity)]
fn enumerate_worktrees(repo: &gix::Repository) -> Result<(Vec<BString>, Vec<WorktreeEntry>)> {
    let proxies = repo.worktrees()?;
    if proxies.is_empty() {
        // Skip the current-dir dependent realpath machinery entirely.
        return Ok((Vec::new(), Vec::new()));
    }
    let current_dir = std::env::current_dir()?;
    let repo_real_path =
        gix::path::realpath_opts(repo.path(), &current_dir, gix::path::realpath::MAX_SYMLINKS)?;

    let mut all_names = Vec::new();
    let mut out = Vec::new();
    for proxy in proxies {
        let name: BString = proxy.id().to_owned();
        all_names.push(name.clone());
        let Ok(path) = proxy.base() else {
            // Missing administrative data - the worktree is prunable.
            continue;
        };
        if !path.is_dir() {
            // The checkout was deleted without `git worktree remove` - prunable.
            continue;
        }
        let Ok(wt_repo) = proxy.into_repo_with_possibly_inaccessible_worktree() else {
            continue;
        };
        let wt_repo_real_path = gix::path::realpath_opts(
            wt_repo.path(),
            &current_dir,
            gix::path::realpath::MAX_SYMLINKS,
        )?;
        if wt_repo_real_path == repo_real_path {
            continue;
        }
        let Ok(mut head) = wt_repo.head() else {
            continue;
        };
        let ref_name = head.referent_name().map(ToOwned::to_owned);
        if ref_name
            .as_ref()
            .is_some_and(|name| but_core::is_workspace_ref_name(name.as_ref()))
        {
            // The workspace ref is fully managed by GitButler already.
            continue;
        }
        let Ok(commit) = head.peel_to_commit() else {
            // Unborn or broken `HEAD`.
            continue;
        };
        out.push(WorktreeEntry {
            archived: false,
            path,
            tip: but_graph::init::WorktreeTip {
                name,
                ref_name,
                id: commit.id,
            },
        });
    }
    Ok((all_names, out))
}

/// A `worktree_meta` row name that marks a project as adopted even when it had no
/// worktrees at first reconciliation. The empty string can never be a real worktree
/// name, and listings intersect with worktrees on disk, so it stays invisible.
const ADOPTION_SENTINEL: &[u8] = b"";

/// Bring the `worktree_meta` table up-to-date with the `names` of all worktrees found
/// on disk and return the names of all archived worktrees.
fn reconcile(
    db: &mut but_db::DbHandle,
    names: &[BString],
) -> Result<std::collections::BTreeSet<BString>> {
    let known: BTreeMap<BString, bool> = db
        .worktree_meta()
        .list()?
        .into_iter()
        .map(|row| (BString::from(row.name), row.archived))
        .collect();
    // An empty table means worktree support was never used on this project;
    // consider all existing worktrees old and archive them.
    let archive_unknown = known.is_empty();
    if archive_unknown {
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: ADOPTION_SENTINEL.to_vec(),
            archived: true,
        })?;
    }

    let mut archived: std::collections::BTreeSet<BString> = known
        .iter()
        .filter_map(|(name, archived)| archived.then(|| name.clone()))
        .collect();
    for name in names {
        if known.contains_key(name) {
            continue;
        }
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: name.to_vec(),
            archived: archive_unknown,
        })?;
        if archive_unknown {
            archived.insert(name.clone());
        }
    }
    Ok(archived)
}
