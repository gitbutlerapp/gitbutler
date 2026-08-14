//! Enumeration of linked worktrees and their archived state.

use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use gix::bstr::{BStr, BString};

use crate::Context;

/// A linked worktree whose checkout exists on disk, with its archived state.
///
/// This is identity only - anything `HEAD`-derived is resolved freshly by
/// [`Context::worktree_head()`] where a consumer actually needs it.
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

/// The `HEAD` of a linked worktree at resolution time, see [`Context::worktree_head()`].
#[derive(Debug, Clone)]
pub struct WorktreeHead {
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to.
    pub id: gix::ObjectId,
}

impl Context {
    /// Add active linked-worktree tips to `options` when worktree manipulation is enabled.
    ///
    /// Each tip's `HEAD` is resolved freshly via [`Self::worktree_head()`]; worktrees
    /// with nothing to seed are skipped.
    ///
    /// Like [`Self::active_worktrees()`], this must not be called while a database
    /// handle is borrowed.
    pub fn graph_options(
        &self,
        mut options: but_graph::init::Options,
    ) -> Result<but_graph::init::Options> {
        for worktree in self.active_worktrees()? {
            let Some(head) = self.worktree_head(worktree.name.as_ref())? else {
                continue;
            };
            options.worktree_tips.push(but_graph::init::WorktreeTip {
                name: worktree.name,
                ref_name: head.ref_name,
                id: head.id,
            });
        }
        Ok(options)
    }

    /// Resolve the `HEAD` of the linked worktree named `name` freshly from its
    /// repository, or `None` when there is no commit to see: the worktree vanished,
    /// its repository or `HEAD` cannot be read, its branch is unborn, or it has the
    /// workspace ref checked out - a ref GitButler fully manages already.
    ///
    /// This is the single home of linked-worktree `HEAD` semantics: consumers
    /// resolve a worktree's branch or commit here at their point of use instead of
    /// holding on to an eagerly captured snapshot.
    pub fn worktree_head(&self, name: &BStr) -> Result<Option<WorktreeHead>> {
        let repo = self.repo.get()?;
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

    /// List all usable linked worktrees with their archived state and resolved `HEAD`s.
    ///
    /// This is the single home of linked-worktree state: every reader - listings, and
    /// graph building when it seeds extra traversal heads - must go through here or
    /// [`Self::active_worktrees()`] so archived worktrees are excluded consistently.
    ///
    /// With the `worktreeManipulation` feature flag disabled this returns nothing and
    /// has no side effects.
    ///
    /// The first-ever read with the flag enabled *adopts*: every worktree already on
    /// disk (whether usable or not) is archived, assuming it predates GitButler's
    /// worktree support, and an explicit marker records that adoption ran even when no
    /// worktree exists yet. A worktree created after adoption is active until
    /// explicitly archived; rows are never pruned - stale rows are invisible as
    /// listings intersect with the worktrees on disk - so a worktree recreated under
    /// a previously archived name stays archived until explicitly unarchived.
    ///
    /// Worktrees whose checkout is gone from disk (prunable) are never returned.
    /// Entries are identity only - whether a worktree has a usable `HEAD` (readable,
    /// born, not the workspace ref) is resolved freshly by [`Self::worktree_head()`]
    /// wherever a consumer actually needs it.
    ///
    /// Errors when the context repository is itself a linked worktree: such a context
    /// stores its database in the worktree's private git dir, so adoption and archived
    /// state would silently diverge from the main worktree's database.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn worktrees_with_state(&self) -> Result<Vec<WorktreeEntry>> {
        if !self.settings.feature_flags.worktree_manipulation {
            return Ok(Vec::new());
        }
        let repo = self.repo.get()?;
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
        let (all_names, mut worktrees) = enumerate_worktrees(&repo)?;

        let mut db = self.db.get_cache_mut()?;
        let archived = adopt_and_read_archived(&mut db, &all_names)?;

        for wt in &mut worktrees {
            wt.archived = archived.contains(&wt.name);
        }
        Ok(worktrees)
    }

    /// Persist whether the linked worktree named `name` is archived.
    ///
    /// This runs the one-time adoption of [`Self::worktrees_with_state()`] first:
    /// adoption archives every worktree on disk, so letting it run afterwards would
    /// silently revert what was explicitly asked for here.
    ///
    /// Rows are never pruned, so this also succeeds for a worktree that isn't on
    /// disk (any more) - such a row simply stays invisible to listings.
    ///
    /// Must not be called while a database handle is borrowed.
    pub fn set_worktree_archived(&self, name: &gix::bstr::BStr, archived: bool) -> Result<()> {
        self.worktrees_with_state()?;
        let mut db = self.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: name.to_vec(),
            archived,
        })?;
        Ok(())
    }

    /// List all non-archived linked worktrees; every returned entry has
    /// `archived == false`.
    ///
    /// This is [`Self::worktrees_with_state()`] filtered down to active worktrees,
    /// including its adoption side-effect, flag gating, and linked-worktree error;
    /// the same caveats apply.
    pub fn active_worktrees(&self) -> Result<Vec<WorktreeEntry>> {
        Ok(self
            .worktrees_with_state()?
            .into_iter()
            .filter(|wt| !wt.archived)
            .collect())
    }
}

/// Enumerate the linked worktrees of `repo`, returning the names of ALL of them
/// (for adoption - a worktree that is unusable today must still be adopted today,
/// not when it becomes usable) along with the entries whose checkout still exists
/// on disk. The `archived` state is not yet known and left `false`.
///
/// This is purely filesystem-based and opens no worktree repositories - anything
/// `HEAD`-derived is [`Context::worktree_head()`]'s concern.
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
