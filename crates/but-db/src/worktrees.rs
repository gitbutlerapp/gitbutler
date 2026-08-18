//! Enumeration of linked worktrees and their archived state.
//!
//! This is the single home of linked-worktree state: every reader - listings, and
//! graph building when it seeds extra traversal heads - must go through
//! [`worktrees_with_state()`] so archived worktrees are excluded consistently, and
//! through [`worktree_head()`] for anything `HEAD`-derived.

use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use gix::bstr::{BStr, BString};

use crate::DbHandle;

/// Whether `ref_name` is a workspace ref, kept in sync with
/// `but_core::is_workspace_ref_name()` by hand: depending on but-core here would
/// pull libgit2 into every crate that reaches but-db, for two string comparisons.
fn is_workspace_ref(ref_name: &gix::refs::FullName) -> bool {
    let name = ref_name.as_bstr();
    name == "refs/heads/gitbutler/workspace" || name == "refs/heads/gitbutler/integration"
}

/// A linked worktree whose checkout exists on disk, with its archived state.
///
/// This is identity only - anything `HEAD`-derived is resolved freshly by
/// [`worktree_head()`] where a consumer actually needs it.
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    /// Whether the worktree is hidden from listings and graph traversal.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: PathBuf,
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`,
    /// which survives `git worktree move`.
    pub name: BString,
}

/// The `HEAD` of a linked worktree at resolution time, see [`worktree_head()`].
#[derive(Debug, Clone)]
pub struct WorktreeHead {
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub id: gix::ObjectId,
}

/// Resolve the `HEAD` of the linked worktree named `name` freshly from its
/// repository, or `None` when there is no commit to see: the worktree vanished,
/// its repository or `HEAD` cannot be read, its branch is unborn, or it has the
/// workspace ref checked out - a ref GitButler fully manages already.
///
/// This is the single home of linked-worktree `HEAD` semantics: consumers
/// resolve a worktree's branch or commit here at their point of use instead of
/// holding on to an eagerly captured snapshot.
pub fn worktree_head(repo: &gix::Repository, name: &BStr) -> Result<Option<WorktreeHead>> {
    let Some(proxy) = repo.worktree_proxy_by_id(name) else {
        return Ok(None);
    };
    let wt_repo = match proxy.into_repo_with_possibly_inaccessible_worktree() {
        Ok(wt_repo) => wt_repo,
        Err(err) => {
            // Unlike the other `None` states, this is never expected.
            tracing::warn!(%name, ?err, "Skipping linked worktree whose repository cannot be opened");
            return Ok(None);
        }
    };
    let mut head = match wt_repo.head() {
        Ok(head) => head,
        Err(err) => {
            tracing::warn!(%name, ?err, "Skipping linked worktree with an unreadable HEAD");
            return Ok(None);
        }
    };
    let ref_name = head.referent_name().map(ToOwned::to_owned);
    if ref_name.as_ref().is_some_and(is_workspace_ref) {
        return Ok(None);
    }
    match head.peel_to_commit() {
        Ok(commit) => Ok(Some(WorktreeHead {
            ref_name,
            id: commit.id,
        })),
        // A worktree on an unborn branch has nothing to see yet.
        Err(gix::head::peel::to_commit::Error::PeelToObject(
            gix::head::peel::to_object::Error::Unborn { .. },
        )) => Ok(None),
        Err(err) => {
            tracing::warn!(%name, ?err, "Skipping linked worktree whose HEAD cannot be peeled to a commit");
            Ok(None)
        }
    }
}

/// List all usable linked worktrees of `repo` with their archived state.
///
/// The first-ever read *adopts*: every worktree already on disk (whether usable or
/// not) is archived, assuming it predates GitButler's worktree support, and an
/// explicit marker records that adoption ran even when no worktree exists yet. A
/// worktree created after adoption is active until explicitly archived; rows are
/// never pruned - stale rows are invisible as listings intersect with the worktrees
/// on disk - so a worktree recreated under a previously archived name stays
/// archived until explicitly unarchived.
///
/// Worktrees whose checkout is gone from disk (prunable) are never returned.
/// Entries are identity only - whether a worktree has a usable `HEAD` (readable,
/// born, not the workspace ref) is resolved freshly by [`worktree_head()`]
/// wherever a consumer actually needs it.
///
/// Errors when `repo` is itself a linked worktree: such a repository stores its
/// database in the worktree's private git dir, so adoption and archived state
/// would silently diverge from the main worktree's database.
pub fn worktrees_with_state(
    repo: &gix::Repository,
    db: &mut DbHandle,
) -> Result<Vec<WorktreeEntry>> {
    // The `commondir` redirect only exists in linked-worktree git dirs; unlike
    // `Kind::LinkedWorkTree`, which is a path heuristic requiring a literal
    // `.git` component, this also catches worktrees of bare repositories.
    if repo.git_dir() != repo.common_dir() {
        anyhow::bail!(
            "worktree state must be read from the main worktree - \
             a linked-worktree context has its own database, letting adoption \
             and archived state diverge"
        );
    }
    let (all_names, mut worktrees) = enumerate_worktrees(repo)?;

    let archived = adopt_and_read_archived(db, &all_names)?;

    for wt in &mut worktrees {
        wt.archived = archived.contains(&wt.name);
    }
    Ok(worktrees)
}

/// Enumerate the linked worktrees of `repo`, returning the names of ALL of them
/// (for adoption - a worktree that is unusable today must still be adopted today,
/// not when it becomes usable) along with the entries whose checkout still exists
/// on disk. The `archived` state is not yet known and left `false`.
///
/// This is purely filesystem-based and opens no worktree repositories - anything
/// `HEAD`-derived is [`worktree_head()`]'s concern.
///
/// `repo` must be the main worktree, so none of the linked worktrees enumerated
/// here can be the repository's own.
fn enumerate_worktrees(repo: &gix::Repository) -> Result<(Vec<BString>, Vec<WorktreeEntry>)> {
    let mut all_names = Vec::new();
    let mut out = Vec::new();
    for proxy in repo.worktrees()? {
        let name: BString = proxy.id().to_owned();
        all_names.push(name.clone());
        let path = match proxy.base() {
            Ok(path) => path,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // Missing administrative data - the worktree is prunable.
                continue;
            }
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose checkout location cannot be read");
                continue;
            }
        };
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                // The `gitdir` file points at something that is not a directory - prunable.
                continue;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // The checkout was deleted without `git worktree remove` - prunable.
                continue;
            }
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose checkout cannot be inspected");
                continue;
            }
        }
        out.push(WorktreeEntry {
            archived: false,
            path,
            name,
        });
    }
    Ok((all_names, out))
}

/// Return the names of all archived worktrees, first running the one-time adoption
/// if it never ran: all `names` currently on disk are archived and the adoption
/// marker is written, in one transaction.
///
/// The marker is explicit so nothing is inferred from the table content: in
/// particular a project's first worktree, created after adoption already ran with
/// zero worktrees on disk, starts out active.
fn adopt_and_read_archived(db: &mut DbHandle, names: &[BString]) -> Result<BTreeSet<BString>> {
    if !db.worktree_meta().adoption_ran()? {
        // An immediate transaction avoids the un-retried `SQLITE_BUSY_SNAPSHOT` a
        // deferred read-then-write would fail with when racing another writer, and
        // the marker is re-checked under the write lock as several processes may
        // adopt concurrently right after the feature flag is enabled.
        let mut trans = db.immediate_transaction()?;
        if !trans.worktree_meta().adoption_ran()? {
            trans.worktree_meta_mut().mark_adopted()?;
            for name in names {
                trans.worktree_meta_mut().upsert(crate::WorktreeMeta {
                    name: name.to_vec(),
                    archived: true,
                })?;
            }
        }
        trans.commit()?;
    }
    Ok(db
        .worktree_meta()
        .list()?
        .into_iter()
        .filter(|row| row.archived)
        .map(|row| BString::from(row.name))
        .collect())
}
