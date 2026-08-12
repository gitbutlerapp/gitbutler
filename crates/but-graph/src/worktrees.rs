//! The linked worktrees of a repository along with their archived state.
//!
//! This is the single home of linked-worktree state: every reader - listings, and the
//! graph traversal that seeds worktree `HEAD`s as extra tips - goes through here so
//! archived worktrees are excluded consistently.
//!
//! The `worktree_meta` table only stores *explicitly set* archived state plus the
//! one-time adoption marker; a worktree without a row is active.

use std::collections::BTreeSet;

use anyhow::Result;
use bstr::BString;

/// A usable linked worktree with its archived state and resolved `HEAD`.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Whether the worktree is hidden from listings and graph traversal.
    pub archived: bool,
    /// The worktree checkout directory.
    pub path: std::path::PathBuf,
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`,
    /// which survives `git worktree move`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub head: gix::ObjectId,
}

/// List all usable linked worktrees of `repo` with their archived state, recording their
/// state in `db`.
///
/// The first-ever call *adopts*: every worktree already on disk (whether usable or not) is
/// archived, assuming it predates GitButler's worktree support, and an explicit marker
/// records that adoption ran even when no worktree exists yet. A worktree created after
/// adoption is active until explicitly archived; rows are never pruned - stale rows are
/// invisible as listings intersect with the worktrees on disk - so a worktree recreated
/// under a previously archived name stays archived until explicitly unarchived.
///
/// Worktrees that are broken (pruned checkout, unresolvable `HEAD`) and worktrees checked
/// out on the workspace ref are never returned, see [`but_core::worktree::linked()`].
///
/// Errors when `repo` is itself a linked worktree: it has its own private git dir, so
/// reading state from there would let adoption and archived state diverge from the main
/// worktree's database. `db` must be that main worktree's database.
pub fn with_state(repo: &gix::Repository, db: &mut but_db::DbHandle) -> Result<Vec<Worktree>> {
    // The `commondir` redirect only exists in linked-worktree git dirs; unlike
    // `Kind::LinkedWorkTree`, which is a path heuristic requiring a literal
    // `.git` component, this also catches worktrees of bare repositories.
    if repo.git_dir() != repo.common_dir() {
        anyhow::bail!(
            "worktree state must be read from the main worktree - \
             a linked worktree has its own database, letting adoption \
             and archived state diverge"
        );
    }
    let (all_names, linked) = but_core::worktree::linked(repo)?;
    let archived = adopt_and_read_archived(db, &all_names)?;
    Ok(linked
        .into_iter()
        .map(|worktree| Worktree {
            archived: archived.contains(&worktree.name),
            path: worktree.path,
            name: worktree.name,
            ref_name: worktree.ref_name,
            head: worktree.head,
        })
        .collect())
}

/// Like [`with_state()`], but keeping only the worktrees that aren't archived; every
/// returned entry has `archived == false`.
pub fn active(repo: &gix::Repository, db: &mut but_db::DbHandle) -> Result<Vec<Worktree>> {
    Ok(with_state(repo, db)?
        .into_iter()
        .filter(|worktree| !worktree.archived)
        .collect())
}

/// Persist whether the linked worktree named `name` is archived.
///
/// This runs the one-time adoption of [`with_state()`] first: adoption archives every
/// worktree on disk, so letting it run afterwards would silently revert what was
/// explicitly asked for here.
///
/// Rows are never pruned, so this also succeeds for a worktree that isn't on disk (any
/// more) - such a row simply stays invisible to listings.
pub fn set_archived(
    repo: &gix::Repository,
    db: &mut but_db::DbHandle,
    name: &bstr::BStr,
    archived: bool,
) -> Result<()> {
    with_state(repo, db)?;
    db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
        name: name.to_vec(),
        archived,
    })?;
    Ok(())
}

/// Return the names of all archived worktrees, first running the one-time adoption
/// if it never ran: all `names` currently on disk are archived and the adoption
/// marker is written, in one transaction.
///
/// The marker is explicit so nothing is inferred from the table content: in
/// particular a project's first worktree, created after adoption already ran with
/// zero worktrees on disk, starts out active.
fn adopt_and_read_archived(
    db: &mut but_db::DbHandle,
    names: &[BString],
) -> Result<BTreeSet<BString>> {
    if !db.worktree_meta().adoption_ran()? {
        // An immediate transaction avoids the un-retried `SQLITE_BUSY_SNAPSHOT` a
        // deferred read-then-write would fail with when racing another writer, and
        // the marker is re-checked under the write lock as several processes may
        // adopt concurrently right after the feature flag is enabled.
        let mut trans = db.immediate_transaction()?;
        if !trans.worktree_meta().adoption_ran()? {
            trans.worktree_meta_mut().mark_adopted()?;
            for name in names {
                trans.worktree_meta_mut().upsert(but_db::WorktreeMeta {
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
