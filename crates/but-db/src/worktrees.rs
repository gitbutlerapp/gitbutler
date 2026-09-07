//! Enumeration of linked worktrees and their archived state.
//!
//! This is the single home of linked-worktree state: every reader - listings, and
//! graph building when it seeds extra traversal heads - must go through
//! [`worktrees_with_state()`] so archived worktrees are excluded consistently, and
//! through [`worktree_head()`] for anything `HEAD`-derived.

use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use gix::bstr::{BStr, BString};

use crate::{
    ConnectionMut,
    connection::ConnectionMutInner,
    table::worktree_meta::{WorktreeMetaHandle, WorktreeMetaHandleMut},
};

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
    if ref_name
        .as_ref()
        .is_some_and(|name| but_core::is_workspace_ref_name(name.as_ref()))
    {
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
/// worktree created after adoption is active until explicitly archived.
///
/// Every read also forgets the rows of worktrees git no longer knows - removed,
/// pruned, or their administrative directory deleted by hand - so a worktree
/// created later under such a name starts out active. A worktree whose checkout
/// is gone but whose administrative directory git still keeps (prunable, possibly
/// locked or on unmounted media) keeps its row until git itself prunes it.
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
    db: &mut ConnectionMut<'_, '_>,
) -> Result<Vec<WorktreeEntry>> {
    if repo.kind() == gix::repository::Kind::LinkedWorkTree {
        anyhow::bail!(
            "worktree state must be read from the main worktree - \
             a linked-worktree context has its own database, letting adoption \
             and archived state diverge"
        );
    }
    let (all_names, mut worktrees) = enumerate_worktrees(repo)?;

    let archived = reconcile_archived(db, &all_names)?;

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
        if proxy.is_prunable() {
            continue;
        }
        let path = match proxy.base() {
            Ok(path) => path,
            Err(err) => {
                tracing::warn!(%name, ?err, "Skipping linked worktree whose checkout location cannot be read");
                continue;
            }
        };
        match std::fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                // The `gitdir` file points at something that is not a directory.
                continue;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // A locked worktree may be unavailable without being prunable.
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

/// Return the names of all archived worktrees among `names`, the worktrees git
/// knows, first running the one-time adoption if it never ran: all `names` are
/// archived and the adoption marker is written, in one transaction. Rows whose
/// name is not in `names` are forgotten.
///
/// The marker is explicit so nothing is inferred from the table content: in
/// particular a project's first worktree, created after adoption already ran with
/// zero worktrees on disk, starts out active.
fn reconcile_archived(
    db: &mut ConnectionMut<'_, '_>,
    names: &[BString],
) -> Result<BTreeSet<BString>> {
    let rows = db.worktree_meta().list()?;
    let needs_update = !db.worktree_meta().adoption_ran()?
        || rows
            .iter()
            .any(|row| !names.iter().any(|name| name == &row.name));
    if !needs_update {
        return Ok(rows
            .into_iter()
            .filter(|row| row.archived)
            .map(|row| row.name.into())
            .collect());
    }

    match &mut db.inner {
        ConnectionMutInner::Database(db) => {
            // Lock before rechecking adoption, avoiding a deferred read-to-write upgrade race.
            let tx = db.immediate_transaction()?;
            let archived = reconcile_archived_in_connection(tx.inner(), names)?;
            tx.commit()?;
            Ok(archived)
        }
        ConnectionMutInner::Transaction(tx) => {
            // Graph refresh during a mutation must use its existing transaction, never BEGIN again.
            let sp = tx.inner_mut().savepoint()?;
            let archived = reconcile_archived_in_connection(&sp, names)?;
            sp.commit()?;
            Ok(archived)
        }
    }
}

fn reconcile_archived_in_connection(
    conn: &rusqlite::Connection,
    names: &[BString],
) -> Result<BTreeSet<BString>> {
    let read = WorktreeMetaHandle { conn };
    let mut write = WorktreeMetaHandleMut { conn };
    if !read.adoption_ran()? {
        write.mark_adopted()?;
        for name in names {
            write.upsert(crate::WorktreeMeta {
                name: name.to_vec(),
                archived: true,
            })?;
        }
    }
    let mut archived = BTreeSet::new();
    for row in read.list()? {
        if !names.iter().any(|name| name == &row.name) {
            write.delete(&row.name)?;
        } else if row.archived {
            archived.insert(row.name.into());
        }
    }
    Ok(archived)
}
