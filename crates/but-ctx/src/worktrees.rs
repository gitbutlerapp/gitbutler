//! Enumeration of linked worktrees and their archived state.

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Result;
use gix::bstr::BString;

use crate::Context;

/// A non-archived linked worktree with its resolved `HEAD`.
#[derive(Debug, Clone)]
pub struct ActiveWorktree {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The worktree `HEAD` as a graph traversal tip.
    pub tip: but_graph::init::WorktreeTip,
}

impl Context {
    /// List all non-archived linked worktrees with their resolved `HEAD`s, reconciling
    /// the `worktree_meta` table with the worktrees on disk.
    ///
    /// The first-ever reconciliation of a project (empty table, at least one worktree)
    /// archives every existing worktree, assuming they predate GitButler's worktree
    /// support. Later reconciliations record unknown worktrees as active. Rows are
    /// never pruned - stale rows are invisible as listings intersect with the
    /// worktrees on disk, and pruning would reset the first-read marker.
    ///
    /// Worktrees that are broken (pruned checkout, unresolvable `HEAD`), the worktree
    /// of the current repository, and worktrees checked out on the workspace ref are
    /// skipped.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn active_worktrees(&self) -> Result<Vec<ActiveWorktree>> {
        let repo = self.repo.get()?;
        let worktrees = enumerate_worktrees(&repo)?;

        let mut db = self.db.get_cache_mut()?;
        let archived = reconcile(&mut db, &worktrees)?;

        Ok(worktrees
            .into_iter()
            .filter(|wt| !archived.contains(&wt.name))
            .collect())
    }
}

/// Enumerate all usable linked worktrees of `repo`, except the one `repo` itself
/// may be opened in.
fn enumerate_worktrees(repo: &gix::Repository) -> Result<Vec<ActiveWorktree>> {
    let current_dir = std::env::current_dir()?;
    let repo_real_path =
        gix::path::realpath_opts(repo.path(), &current_dir, gix::path::realpath::MAX_SYMLINKS)?;

    let mut out = Vec::new();
    for proxy in repo.worktrees()? {
        let name: BString = proxy.id().to_owned();
        let Ok(path) = proxy.base() else {
            // Missing administrative data - the worktree is prunable.
            continue;
        };
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
        out.push(ActiveWorktree {
            name,
            path,
            tip: but_graph::init::WorktreeTip {
                ref_name,
                id: commit.id,
            },
        });
    }
    Ok(out)
}

/// Bring the `worktree_meta` table up-to-date with the `worktrees` found on disk and
/// return the names of all archived worktrees.
fn reconcile(
    db: &mut but_db::DbHandle,
    worktrees: &[ActiveWorktree],
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

    let mut archived: std::collections::BTreeSet<BString> = known
        .iter()
        .filter_map(|(name, archived)| archived.then(|| name.clone()))
        .collect();
    for wt in worktrees {
        if known.contains_key(&wt.name) {
            continue;
        }
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: wt.name.to_vec(),
            archived: archive_unknown,
        })?;
        if archive_unknown {
            archived.insert(wt.name.clone());
        }
    }
    Ok(archived)
}
