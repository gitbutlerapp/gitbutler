//! Mutations. Each opens the project, calls the matching `but-api` function and
//! returns; the UI reloads the workspace afterwards. Everything that rewrites the
//! workspace records undo history through `but-api`'s own oplog snapshots.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context as _, Result, bail};
use but_api::commit::json::ChangesSource;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

use super::{CommitId, FileChange, Selected, open};

/// Where a commit or a set of changes lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The top of this branch (a full ref name).
    Branch(String),
    /// Right above this commit, in its branch.
    AboveCommit(CommitId),
}

impl Destination {
    fn relative_to(&self) -> Result<(RelativeTo, InsertSide)> {
        Ok(match self {
            Destination::Branch(ref_name) => (
                RelativeTo::Reference(full_name(ref_name)?),
                InsertSide::Below,
            ),
            Destination::AboveCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Above),
        })
    }
}

fn full_name(ref_name: &str) -> Result<gix::refs::FullName> {
    gix::refs::FullName::try_from(ref_name).with_context(|| format!("Invalid ref {ref_name}"))
}

/// Commits an operation rewrote, old id to new id, so the UI can keep pointing at them.
pub type Rewritten = HashMap<CommitId, CommitId>;

fn rewritten(workspace: &but_api::WorkspaceState) -> Rewritten {
    workspace
        .replaced_commits
        .iter()
        .map(|(old, new)| (CommitId(*old), CommitId(*new)))
        .collect()
}

/// Whole files, or just some of their hunks.
fn selected_specs(selected: &[Selected]) -> Vec<DiffSpec> {
    selected
        .iter()
        .map(|part| {
            let mut spec = DiffSpec::from(&part.file.change);
            if let Some(hunks) = &part.hunks {
                spec.hunk_headers = hunks.iter().map(|hunk| hunk.0).collect();
            }
            spec
        })
        .collect()
}

/// Changes GitButler couldn't take, as "path (reason)", so the UI can say so.
pub type LeftOut = Vec<String>;

fn left_out<R: std::fmt::Debug>(rejected: &[(R, DiffSpec)]) -> LeftOut {
    rejected
        .iter()
        .map(|(reason, spec)| format!("{} ({reason:?})", spec.path))
        .collect()
}

/// A branch name as GitButler would store it, like `my branch` → `refs/heads/my-branch`.
fn branch_ref(name: &str) -> Result<gix::refs::FullName> {
    let name = but_core::branch::normalize_short_name(name.trim())?;
    full_name(&format!("refs/heads/{name}"))
}

fn specs(files: &[FileChange]) -> Vec<DiffSpec> {
    files
        .iter()
        .map(|file| DiffSpec::from(&file.change))
        .collect()
}

/// The runtime every GitButler call runs on. Its threads get big stacks: GitButler's
/// graph walks and async push futures are deep, especially in debug builds, and would
/// overflow the small stacks of the UI framework's own worker threads.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_stack_size(16 * 1024 * 1024)
            .enable_all()
            .thread_name("gitbutler")
            .build()
            .expect("tokio runtime starts")
    })
}

/// Run blocking GitButler work off the UI thread; await the result from anywhere.
pub fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> impl std::future::Future<Output = Result<T>> {
    let handle = runtime().spawn_blocking(work);
    async move { handle.await.context("GitButler task failed")? }
}

/// Commit the `selected` uncommitted changes to `destination`.
pub fn commit(
    path: &Path,
    destination: &Destination,
    selected: &[Selected],
    message: &str,
) -> Result<(CommitId, LeftOut)> {
    let mut ctx = open(path)?;
    let changes = selected_specs(selected);
    if changes.is_empty() {
        bail!("Nothing to commit");
    }
    let (relative_to, side) = destination.relative_to()?;
    let mut guard = ctx.exclusive_worktree_access();
    let outcome = but_api::commit::create::commit_create(
        &mut ctx,
        relative_to,
        side,
        changes,
        ChangesSource::Head,
        message.to_owned(),
        DryRun::No,
        guard.write_permission(),
    )?;
    let left_out = left_out(&outcome.rejected_specs);
    match outcome.new_commit {
        Some(id) => Ok((CommitId(id), left_out)),
        None => bail!("Nothing could be committed: {}", left_out.join(", ")),
    }
}

/// Create a new stack named `name` (or a generated name) and return its full ref name.
pub fn create_branch(path: &Path, name: Option<&str>) -> Result<String> {
    let mut ctx = open(path)?;
    let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => but_api::branch::branch_canned_name(&ctx)?,
    };
    let ref_name = branch_ref(&name)?;
    but_api::branch::branch_create(
        &mut ctx,
        Some(ref_name.clone()),
        but_api::branch::json::BranchCreatePlacement::Independent { order: None },
    )?;
    Ok(ref_name.to_string())
}

/// Stack a new branch on top of `below` (a full ref name).
pub fn create_dependent_branch(path: &Path, below: &str, name: &str) -> Result<String> {
    let mut ctx = open(path)?;
    let ref_name = branch_ref(name)?;
    but_api::branch::branch_create(
        &mut ctx,
        Some(ref_name.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(full_name(below)?),
            side: InsertSide::Above,
        },
    )?;
    Ok(ref_name.to_string())
}

/// Rename a branch and return its new full ref name.
pub fn rename_branch(path: &Path, ref_name: &str, new_name: &str) -> Result<String> {
    let mut ctx = open(path)?;
    let outcome =
        but_api::branch::branch_rename(&mut ctx, full_name(ref_name)?, new_name.trim().to_owned())?;
    Ok(outcome.new_ref.to_string())
}

pub fn remove_branch(path: &Path, ref_name: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::branch_remove(&mut ctx, full_name(ref_name)?)?;
    Ok(())
}

/// Fold uncommitted `files` into `commit`.
pub fn amend(path: &Path, commit: CommitId, files: &[FileChange]) -> Result<(Rewritten, LeftOut)> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::amend::commit_amend(
        &mut ctx,
        commit.0,
        specs(files),
        ChangesSource::Head,
        DryRun::No,
    )?;
    let left_out = left_out(&outcome.rejected_specs);
    if outcome.new_commit.is_none() {
        bail!("Nothing could be amended: {}", left_out.join(", "));
    }
    Ok((rewritten(&outcome.workspace), left_out))
}

pub fn move_commits(
    path: &Path,
    commits: &[CommitId],
    destination: &Destination,
) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let (relative_to, side) = destination.relative_to()?;
    let outcome = but_api::commit::move_commit::commit_move(
        &mut ctx,
        commits.iter().map(|id| id.0).collect(),
        relative_to,
        side,
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

/// Squash `commits` into `into`, keeping all messages.
pub fn squash(path: &Path, commits: &[CommitId], into: CommitId) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::squash::commit_squash(
        &mut ctx,
        commits.iter().map(|id| id.0).collect(),
        into.0,
        but_workspace::commit::squash_commits::MessageCombinationStrategy::KeepBoth,
        DryRun::No,
    )?;
    let mut rewrites = rewritten(&outcome.workspace);
    // The squashed commits live on in the result.
    for id in commits.iter().chain([&into]) {
        rewrites.insert(*id, CommitId(outcome.new_commit));
    }
    Ok(rewrites)
}

/// Turn `commits` back into uncommitted changes.
pub fn uncommit(path: &Path, commits: &[CommitId]) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        commits.iter().map(|id| id.0).collect(),
        None,
        DryRun::No,
    )?;
    Ok(())
}

/// Take `files` out of `commit` and leave them uncommitted.
pub fn uncommit_files(path: &Path, commit: CommitId, files: &[FileChange]) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::uncommit::commit_uncommit_changes(
        &mut ctx,
        commit.0,
        specs(files),
        None,
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

/// Move `files` that `from` changes into `to`.
pub fn move_files(
    path: &Path,
    from: CommitId,
    to: CommitId,
    files: &[FileChange],
) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::move_changes::commit_move_changes_between(
        &mut ctx,
        from.0,
        to.0,
        specs(files),
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

pub fn reword(path: &Path, commit: CommitId, message: &str) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome =
        but_api::commit::reword::commit_reword(&mut ctx, commit.0, message.into(), DryRun::No)?;
    let mut rewrites = rewritten(&outcome.workspace);
    rewrites.insert(commit, CommitId(outcome.new_commit));
    Ok(rewrites)
}

/// Drop `commit` and everything it changed.
pub fn discard_commit(path: &Path, commit: CommitId) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::commit::discard_commit::commit_discard(&mut ctx, vec![commit.0], DryRun::No)?;
    Ok(())
}

/// Throw away the `selected` uncommitted changes.
pub fn discard(path: &Path, selected: &[Selected]) -> Result<()> {
    let mut ctx = open(path)?;
    let rejected =
        but_api::legacy::workspace::discard_worktree_changes(&mut ctx, selected_specs(selected))?;
    if let Some(spec) = rejected.first() {
        bail!("Couldn't discard {}", spec.path);
    }
    Ok(())
}

/// Push `ref_name` and the branches below it in its stack.
pub fn push(path: &Path, ref_name: &str, force: bool) -> Result<()> {
    let ctx = open(path)?.into_sync();
    runtime().handle().block_on(
        but_api::legacy::workspace::workspace_branch_and_ancestors_push(
            ctx,
            force,
            false,
            ref_name.to_owned(),
            true,
            Vec::new(),
        ),
    )?;
    Ok(())
}

/// Rebase every stack onto the target as it is now. Doesn't fetch first; see [`fetch()`].
pub fn update_from_target(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let updates =
        but_api::workspace::rebase_lane_bottoms(info.stacks.iter().map(|stack| stack.lane()));
    let mut guard = ctx.exclusive_worktree_access();
    but_api::workspace::workspace_integrate_upstream_with_perm(
        &mut ctx,
        updates,
        DryRun::No,
        guard.write_permission(),
    )?;
    Ok(())
}

pub fn fetch(path: &Path) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::workspace::workspace_fetch_from_remotes(&mut ctx, Some("fetch".into()))
}

/// Restore what the last undo took back. Returns `false` when there's nothing to redo.
pub fn redo(path: &Path) -> Result<bool> {
    let mut ctx = open(path)?;
    let Some(snapshot) = but_api::legacy::oplog::get_redo_target_snapshot(&ctx)? else {
        return Ok(false);
    };
    but_api::legacy::oplog::restore_snapshot_with_kind(
        &mut ctx,
        but_api::legacy::oplog::RestoreKind::RestoreFromSnapshotViaRedo,
        snapshot.commit_id,
    )?;
    Ok(true)
}

/// Restore the workspace to before the last operation. Returns `false` when there's nothing to undo.
pub fn undo(path: &Path) -> Result<bool> {
    let mut ctx = open(path)?;
    let Some(snapshot) = but_api::legacy::oplog::get_undo_target_snapshot(&ctx)? else {
        return Ok(false);
    };
    but_api::legacy::oplog::restore_snapshot_with_kind(
        &mut ctx,
        but_api::legacy::oplog::RestoreKind::RestoreFromSnapshotViaUndo,
        snapshot.commit_id,
    )?;
    Ok(true)
}
