use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use but_core::sync::RepoExclusive;

use super::{FullName, SnapshotId, history, open, worktrees};

/// The git side of one user action, applied by [`apply`].
pub trait Intent: Send + 'static {
    type Done: Send + 'static;
    fn apply(self, run: &mut Run) -> Result<Self::Done>;

    fn kind(&self) -> OpKind {
        OpKind::Other
    }
}

/// The operations treated apart from the rest. Labels are only what the user reads.
#[derive(PartialEq)]
pub enum OpKind {
    Other,
    Fetch,
    /// A push of the branch with this full ref, with the ones below it, on its own or to open
    /// its pull request.
    Push(FullName),
    /// Deleting the linked worktree with this name.
    DeleteWorktree(bstr::BString),
    /// An update from the target.
    Update,
    /// Resolving conflicts, which are numbered anew once it's done.
    Resolve,
    MarkResolved,
    FinishEditing,
    CancelEditing,
    /// ⌘Z and ⌘⇧Z, of the step with this oplog entry, which keep the undo history themselves.
    Undo(SnapshotId),
    Redo(SnapshotId),
    /// Operations history's restore, the way back when a project can't be read.
    Restore,
    /// Merging a pull request or turning its auto-merge on or off.
    Forge,
}

impl OpKind {
    /// Whether it only talks to the remote or the forge and moves refs, leaving the worktree
    /// and the commits alone, so reloads the watcher starts meanwhile go ahead.
    pub fn remote_only(&self) -> bool {
        matches!(self, Self::Fetch | Self::Push(_) | Self::Forge)
    }

    /// Whether it runs beside the project's other operation: it only talks to the remote or
    /// deletes a folder, so nothing another one does depends on it, and it can take long
    /// enough to hold them up.
    pub fn beside(&self) -> bool {
        matches!(self, Self::Fetch | Self::Push(_) | Self::DeleteWorktree(_))
    }

    /// Whether it runs while a commit is checked out for editing: fetching, which leaves the
    /// worktree alone, and what resolves the commit and ends the edit.
    pub fn while_editing(&self) -> bool {
        matches!(
            self,
            Self::Fetch
                | Self::Resolve
                | Self::MarkResolved
                | Self::FinishEditing
                | Self::CancelEditing
        )
    }
}

/// One operation on one project.
pub struct Run {
    path: PathBuf,
    ctx: but_ctx::Context,
    locked: bool,
    recorded: Option<SnapshotId>,
    moved_worktree: bool,
    span: Option<(i64, i64)>,
}

/// What an operation did beyond its own result: the oplog entry it recorded, whether it moved
/// a linked worktree's `HEAD`, and when its work ran, in milliseconds since the epoch.
#[derive(Debug)]
pub struct Ran {
    pub recorded: Option<SnapshotId>,
    pub moved_worktree: bool,
    pub span: (i64, i64),
}

impl Run {
    /// The project, for what's read before the lock is taken and after it's let go.
    pub(super) fn ctx(&self) -> &but_ctx::Context {
        &self.ctx
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Refuse while `.git/index.lock` exists. Left by a Git that crashed or was killed, as an
    /// agent's can be, it stops an operation partway, after it has written files.
    pub(super) fn refuse_index_locked(&self) -> Result<()> {
        let index_lock = self.ctx.repo.get()?.git_dir().join("index.lock");
        anyhow::ensure!(
            !index_lock.exists(),
            "Another Git process is using the repository, or one that stopped left \
             .git/index.lock behind. Delete it if no Git command is running."
        );
        Ok(())
    }

    /// Run `work` under GitButler's lock, noting what it recorded and moved under it too, so
    /// an agent's `but` can't land in between and be counted as this operation's. Below it go
    /// only `Transaction` methods and `_with_perm` calls: one that takes the lock again
    /// deadlocks, and panics under `BUT_WS_LOCK_DEBUG=1` in debug builds.
    pub(super) fn locked<R>(
        &mut self,
        work: impl FnOnce(&mut but_ctx::Context, &mut RepoExclusive) -> Result<R>,
    ) -> Result<R> {
        let mut guard = self.ctx.exclusive_worktree_access();
        self.watched(|ctx| work(ctx, guard.write_permission()))
    }

    /// Run `work`, noting what it recorded and moved. Called directly only for work that takes
    /// GitButler's lock itself, which would deadlock under [`Self::locked`].
    pub(super) fn watched<R>(
        &mut self,
        work: impl FnOnce(&mut but_ctx::Context) -> Result<R>,
    ) -> Result<R> {
        anyhow::ensure!(
            !std::mem::replace(&mut self.locked, true),
            "BUG: an operation locks its project once"
        );
        self.refuse_index_locked()?;
        let oplog = history::oplog_head(&self.ctx);
        let heads = worktrees::heads(&*self.ctx.repo.get()?);
        // Just the work: what Git logs after it, as an agent's commit waiting on the lock does,
        // isn't this operation's.
        let started = now_millis();
        let value = work(&mut self.ctx)?;
        self.span = Some((started, now_millis()));
        self.recorded = history::oplog_head(&self.ctx).filter(|after| Some(*after) != oplog);
        // The oplog records only the main checkout: what moved a linked worktree, even one
        // stacked on a branch here, can't be undone.
        self.moved_worktree = worktrees::heads(&*self.ctx.repo.get()?).moved_since(&heads);
        Ok(value)
    }
}

pub fn apply<I: Intent>(path: &Path, intent: I) -> Result<(I::Done, Ran)> {
    let mut run = Run {
        path: path.to_owned(),
        ctx: open(path)?,
        locked: false,
        recorded: None,
        moved_worktree: false,
        span: None,
    };
    let started = now_millis();
    let done = intent.apply(&mut run)?;
    let ran = Ran {
        recorded: run.recorded,
        moved_worktree: run.moved_worktree,
        span: run.span.unwrap_or((started, now_millis())),
    };
    Ok((done, ran))
}

/// Now, in milliseconds since the Unix epoch, as file times are.
pub fn now_millis() -> i64 {
    (SystemTime::now().duration_since(UNIX_EPOCH)).map_or(0, |t| t.as_millis() as i64)
}
