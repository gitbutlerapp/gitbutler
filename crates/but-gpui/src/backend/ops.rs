//! Mutations of commits, branches and uncommitted changes. Each opens the project, calls
//! the matching `but-api` function and returns; the UI reloads the workspace afterwards.
//! Undo history comes from `but-api`'s own oplog snapshots, except that [`discard`],
//! [`split_off`] and [`move_to_new_branch`] record their own.

use gitbutler_repo::hooks::{ErrorData, HookResult, MessageHookResult};
use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use but_api::commit::json::ChangesSource;
use but_core::tree::create_tree::RejectionReason;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

use super::{ChangeKind, CommitId, Selected, checkout, open, written};

/// Where a commit or a set of changes lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The top of this branch (a full ref name).
    Branch(String),
    /// Right above this commit, in its branch.
    AboveCommit(CommitId),
    /// Right below this commit, in its branch.
    BelowCommit(CommitId),
}

impl Destination {
    fn relative_to(&self) -> Result<(RelativeTo, InsertSide)> {
        Ok(match self {
            Destination::Branch(ref_name) => (
                RelativeTo::Reference(full_name(ref_name)?),
                InsertSide::Below,
            ),
            Destination::AboveCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Above),
            Destination::BelowCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Below),
        })
    }
}

pub(super) fn full_name(ref_name: &str) -> Result<gix::refs::FullName> {
    gix::refs::FullName::try_from(ref_name).with_context(|| format!("Invalid ref {ref_name}"))
}

/// Commits an operation rewrote, old id to new id, so the UI can keep pointing at them.
pub type Rewritten = HashMap<CommitId, CommitId>;

pub(super) fn rewritten(workspace: &but_api::WorkspaceState) -> Rewritten {
    workspace
        .replaced_commits
        .iter()
        .map(|(old, new)| (CommitId(*old), CommitId(*new)))
        .collect()
}

/// The `selected` files a merge left conflicted, as the index has them now: what the UI
/// asks for may predate the merge.
fn unmerged<'a>(repo: &gix::Repository, selected: &'a [Selected]) -> Result<Vec<&'a str>> {
    let index = repo.index_or_empty()?;
    Ok((selected.iter())
        .map(|part| part.file.path.as_str())
        .filter(|path| {
            (index.entry_index_by_path(path.as_bytes().as_bstr())).is_ok_and(|ix| {
                index.entries()[ix].stage() != gix::index::entry::Stage::Unconflicted
            })
        })
        .collect())
}

/// Refuse conflicted files of the main checkout, or of the linked `worktree`, which would go
/// in markers and all.
fn refuse_conflicted(
    ctx: &but_ctx::Context,
    worktree: Option<&str>,
    selected: &[Selected],
) -> Result<()> {
    let repo = checkout(ctx, worktree)?;
    if let Some(path) = unmerged(&repo, selected)?.first() {
        bail!("Mark {path} resolved first: it still has conflicts");
    }
    refuse_changed(&repo, selected)
}

/// Refuse some of an uncommitted file written to since its diff was read, as by an agent:
/// its hunks and lines are placed by line numbers that an edit above or among them moves,
/// and the engine would take whatever lies there now.
fn refuse_changed(repo: &gix::Repository, selected: &[Selected]) -> Result<()> {
    let changed = (selected.iter()).find(|part| {
        part.hunks.is_some() && written(repo, part.file.path.as_str().into()) != part.file.modified
    });
    if let Some(part) = changed {
        bail!(
            "{} changed since the diff was shown; try again",
            part.file.path
        );
    }
    Ok(())
}

/// Where changes are read from: the main checkout, or the linked `worktree`.
fn changes_source(worktree: Option<&str>) -> ChangesSource {
    worktree.map_or(ChangesSource::Head, |name| {
        ChangesSource::Worktree(name.to_owned())
    })
}

/// [`selected_specs`] to take out of a commit or the worktree, refusing some of a file that
/// can only be taken whole, as the rest would go along.
fn taken_specs(selected: &[Selected]) -> Result<Vec<DiffSpec>> {
    if let Some(part) =
        (selected.iter()).find(|part| part.file.whole_only() && part.hunks.is_some())
    {
        let why = match part.file.kind {
            ChangeKind::Added => "it's added here",
            ChangeKind::Deleted => "it's deleted here",
            _ => "it's not a plain file on one side",
        };
        bail!("Only all of {} can be taken out, as {why}", part.file.path);
    }
    Ok(selected_specs(selected))
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

fn left_out(rejected: &[(RejectionReason, DiffSpec)]) -> LeftOut {
    rejected
        .iter()
        .map(|(reason, spec)| format!("{}: {}", spec.path, why(reason)))
        .collect()
}

/// Why GitButler couldn't take a change, said for the person who asked.
fn why(reason: &RejectionReason) -> &'static str {
    use RejectionReason as R;
    match reason {
        R::NoEffectiveChanges => "nothing in it would change",
        // The changes it builds on can be in a later commit of the same branch.
        R::CherryPickMergeConflict => {
            "it builds on changes in a later commit or another branch; put it where those are, \
             or move them first"
        }
        R::WorkspaceMergeConflict => {
            "it builds on changes in another branch; commit there, or move those first"
        }
        R::WorkspaceMergeConflictOfUnrelatedFile => "it would conflict with another branch",
        R::WorktreeFileMissingForObjectConversion => "the file is gone from disk",
        R::FileToLargeOrBinary => "it's binary or too large to split",
        R::PathNotFoundInBaseTree => "the file isn't where the branch starts from",
        R::UnsupportedDirectoryEntry | R::UnsupportedTreeEntry => "it isn't a file Git can commit",
        R::MissingDiffSpecAssociation => "it changed since the diff was shown; try again",
    }
}

/// A branch name as GitButler would store it, like `my branch` → `refs/heads/my-branch`.
fn branch_ref(name: &str) -> Result<gix::refs::FullName> {
    let name = but_core::branch::normalize_short_name(name.trim())?;
    full_name(&format!("refs/heads/{name}"))
}

/// Run the pre-commit hook on what `changes` would commit; its failure refuses the commit.
fn pre_commit(ctx: &but_ctx::Context, changes: &[DiffSpec]) -> Result<()> {
    let result = but_api::legacy::repo::pre_commit_hook_diffspecs(ctx, changes.to_vec())?;
    if let HookResult::Failure(ErrorData { error }) = result {
        bail!("The pre-commit hook refused the commit:\n{error}");
    }
    Ok(())
}

/// Run the post-commit hook, and say what it printed if it failed: the commit stands.
fn post_commit(ctx: &but_ctx::Context) -> Option<String> {
    match but_api::legacy::repo::post_commit_hook(ctx) {
        Ok(HookResult::Failure(ErrorData { error })) => Some(error),
        Ok(_) => None,
        Err(err) => Some(format!("{err:#}")),
    }
}

/// GitButler's branches and commits build on a commit; core's own error for a repository
/// without one names its internals.
fn refuse_unborn(ctx: &but_ctx::Context) -> Result<()> {
    if ctx.repo.get()?.head()?.is_unborn() {
        bail!("The repository has no commits yet. Make the first one with Git.");
    }
    Ok(())
}

/// Commit the `selected` uncommitted changes to `destination`. With `run_hooks`, the
/// repository's hooks run in Git's order. pre-commit and commit-msg run first, and either
/// refuses the commit by failing; commit-msg can also rewrite the message. post-commit runs
/// once the commit is made, which stands even if the hook fails, and what it printed then
/// comes back as the third value. Hooks run outside GitButler's lock, so one that runs
/// `but` doesn't wait on it.
pub fn commit(
    path: &Path,
    destination: &Destination,
    selected: &[Selected],
    worktree: Option<&str>,
    message: &str,
    run_hooks: bool,
) -> Result<(CommitId, LeftOut, Option<String>)> {
    let mut ctx = open(path)?;
    refuse_unborn(&ctx)?;
    refuse_conflicted(&ctx, worktree, selected)?;
    // The hooks run in the main checkout, not where a linked worktree's changes are.
    let run_hooks = run_hooks && worktree.is_none();
    // No changes make an empty commit.
    let changes = selected_specs(selected);
    let mut message = message.to_owned();
    if run_hooks {
        pre_commit(&ctx, &changes)?;
        match but_api::legacy::repo::message_hook(&ctx, message.clone())? {
            MessageHookResult::Failure(ErrorData { error }) => {
                bail!("The commit-msg hook refused the commit:\n{error}")
            }
            MessageHookResult::Message(rewritten) => message = rewritten.message,
            MessageHookResult::Success | MessageHookResult::NotConfigured => {}
        }
    }
    let (relative_to, side) = destination.relative_to()?;
    let outcome = {
        let mut guard = ctx.exclusive_worktree_access();
        but_api::commit::create::commit_create(
            &mut ctx,
            relative_to,
            side,
            changes,
            changes_source(worktree),
            message,
            DryRun::No,
            guard.write_permission(),
        )?
    };
    let left_out = left_out(&outcome.rejected_specs);
    let Some(id) = outcome.new_commit else {
        bail!("Nothing could be committed.\n{}", left_out.join("\n"));
    };
    let hook_failed = run_hooks.then(|| post_commit(&ctx)).flatten();
    Ok((CommitId(id), left_out, hook_failed))
}

/// Create a new stack named `name` (or a generated name) and return its full ref name.
/// With one branch checked out rather than the workspace, there are no stacks beside it:
/// the new branch goes on top of it and is checked out, as `git checkout -b` would.
pub fn create_branch(path: &Path, name: Option<&str>) -> Result<String> {
    use but_api::branch::json::BranchCreatePlacement as Placement;
    let mut ctx = open(path)?;
    refuse_unborn(&ctx)?;
    let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => but_api::branch::branch_canned_name(&ctx)?,
    };
    let ref_name = branch_ref(&name)?;
    let checked_out = (ctx.repo.get()?.head_name()?).filter(|head| {
        head.as_bstr() != but_core::WORKSPACE_REF_NAME
            && head.as_ref().category() == Some(gix::refs::Category::LocalBranch)
    });
    let placement = match checked_out {
        Some(head) => Placement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(head),
            side: InsertSide::Above,
        },
        None => Placement::Independent { order: None },
    };
    but_api::branch::branch_create(&mut ctx, Some(ref_name.clone()), placement)?;
    Ok(ref_name.to_string())
}

/// Make branch `name` at the target, or one with a made-up name, and switch to it on its own,
/// leaving the workspace, as Lite's New Branch and Switch to It does. Returns its full name.
pub fn switch_to_new_branch(path: &Path, name: Option<&str>) -> Result<String> {
    let mut ctx = open(path)?;
    let name = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => but_api::branch::branch_canned_name(&ctx)?,
    };
    let ref_name = branch_ref(&name)?;
    but_api::branch::branch_checkout_new(&mut ctx, Some(name))?;
    Ok(ref_name.to_string())
}

/// Switch to `ref_name` (a full ref name) on its own, leaving the workspace; one only a
/// remote has gets its local branch first. Returns the local branch's full name.
pub fn switch_to_branch(path: &Path, ref_name: &str) -> Result<String> {
    let mut ctx = open(path)?;
    let name = full_name(ref_name)?;
    let local = match name.category() {
        Some(gix::refs::Category::RemoteBranch) => {
            but_workspace::branch::local_tracking_branch(&*ctx.repo.get()?, name.as_ref())?
        }
        _ => name.clone(),
    };
    but_api::branch::branch_checkout(&mut ctx, name)?;
    Ok(local.to_string())
}

/// Stack a new branch on top of `below` (a full ref name).
pub fn create_dependent_branch(path: &Path, below: &str, name: &str) -> Result<String> {
    let relative_to = but_api::commit::json::RelativeTo::Reference(full_name(below)?);
    create_branch_at(path, relative_to, InsertSide::Above, name)
}

/// Make a new branch whose newest commit is `commit`, taking it and the commits below it
/// in its branch; or with `below`, one that ends just under `commit`.
pub fn create_branch_at_commit(
    path: &Path,
    commit: CommitId,
    below: bool,
    name: &str,
) -> Result<String> {
    let side = if below {
        InsertSide::Below
    } else {
        InsertSide::Above
    };
    let relative_to = but_api::commit::json::RelativeTo::Commit(commit.0);
    create_branch_at(path, relative_to, side, name)
}

fn create_branch_at(
    path: &Path,
    relative_to: but_api::commit::json::RelativeTo,
    side: InsertSide,
    name: &str,
) -> Result<String> {
    let mut ctx = open(path)?;
    let ref_name = branch_ref(name)?;
    but_api::branch::branch_create(
        &mut ctx,
        Some(ref_name.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent { relative_to, side },
    )?;
    Ok(ref_name.to_string())
}

/// Stack `branch` (a full ref name) on top of `onto`, with the branches above it.
pub fn stack_branch(path: &Path, branch: &str, onto: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::move_branch(
        &mut ctx,
        full_name(branch)?.as_ref(),
        RelativeTo::Reference(full_name(onto)?),
        InsertSide::Above,
        DryRun::No,
    )?;
    Ok(())
}

/// Move `branch` right below `commit`, the last commit of the branch it then sits under.
/// Below that branch's reference instead, `branch` would take over its commits.
pub fn move_branch_below(path: &Path, branch: &str, commit: CommitId) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::move_branch(
        &mut ctx,
        full_name(branch)?.as_ref(),
        RelativeTo::Commit(commit.0),
        InsertSide::Below,
        DryRun::No,
    )?;
    Ok(())
}

/// Take `branch` out of its stack into a stack of its own.
pub fn tear_off_branch(path: &Path, branch: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::tear_off_branch(&mut ctx, full_name(branch)?.as_ref(), DryRun::No)?;
    Ok(())
}

/// Take the stack holding `branch` out of the workspace. Its branches stay in the repository.
pub fn unapply_stack(path: &Path, branch: &str) -> Result<()> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let stack_id = info
        .stacks
        .iter()
        .find(|stack| stack.segments.iter().any(|s| holds(s, branch)))
        .and_then(|stack| stack.id)
        .context("That branch isn't in the workspace any more")?;
    but_api::legacy::virtual_branches::unapply_stack(&mut ctx, stack_id)
}

pub(super) fn holds(segment: &but_workspace::ref_info::Segment, branch: &str) -> bool {
    segment
        .ref_name()
        .is_some_and(|name| name.as_bstr() == branch)
}

/// An empty commit at `at`, to move changes into later.
pub fn insert_empty_commit(path: &Path, at: &Destination) -> Result<CommitId> {
    let mut ctx = open(path)?;
    let (relative_to, side) = at.relative_to()?;
    let outcome = but_api::commit::insert_blank::commit_insert_blank(
        &mut ctx,
        relative_to,
        side,
        DryRun::No,
    )?;
    Ok(CommitId(outcome.new_commit))
}

/// Put copies of `commits`, listed newest first, at `to` in one go, keeping the originals
/// where they are. The oldest goes in first and each newer one on top, as in their history.
pub fn cherry_pick(path: &Path, commits: &[CommitId], to: &Destination) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let (relative_to, side) = to.relative_to()?;
    let outcome = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        commits.iter().rev().map(|commit| commit.0).collect(),
        relative_to,
        side,
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

/// Rename a branch and return its new full ref name.
pub fn rename_branch(path: &Path, ref_name: &str, new_name: &str) -> Result<String> {
    let mut ctx = open(path)?;
    let outcome =
        but_api::branch::branch_rename(&mut ctx, full_name(ref_name)?, new_name.trim().to_owned())?;
    Ok(outcome.new_ref.to_string())
}

/// Name the main checkout's unnamed branch whose newest commit is `top`, putting a `HEAD`
/// detached there on it, and return its full ref name.
pub fn name_branch(path: &Path, top: CommitId, name: &str) -> Result<String> {
    let mut ctx = open(path)?;
    let new_ref = branch_ref(name)?;
    let outcome =
        but_api::branch::branch_name_segment(&mut ctx, ChangesSource::Head, top.0, Some(new_ref))?;
    Ok(outcome.new_ref.to_string())
}

pub fn remove_branch(path: &Path, ref_name: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::branch_remove(&mut ctx, full_name(ref_name)?)?;
    Ok(())
}

/// Add the `selected` changes of the main checkout, or of the linked `worktree`, to
/// `commit`. With `run_hooks`, pre-commit runs first and can refuse, and post-commit runs
/// after, as the desktop app does; what a failed post-commit printed comes back as the
/// third value.
pub fn amend(
    path: &Path,
    commit: CommitId,
    selected: &[Selected],
    worktree: Option<&str>,
    run_hooks: bool,
) -> Result<(Rewritten, LeftOut, Option<String>)> {
    let mut ctx = open(path)?;
    refuse_conflicted(&ctx, worktree, selected)?;
    let run_hooks = run_hooks && worktree.is_none();
    let changes = selected_specs(selected);
    if run_hooks {
        pre_commit(&ctx, &changes)?;
    }
    let outcome = but_api::commit::amend::commit_amend(
        &mut ctx,
        commit.0,
        changes,
        changes_source(worktree),
        DryRun::No,
    )?;
    let left_out = left_out(&outcome.rejected_specs);
    if outcome.new_commit.is_none() {
        bail!("Nothing could be amended.\n{}", left_out.join("\n"));
    }
    let hook_failed = run_hooks.then(|| post_commit(&ctx)).flatten();
    Ok((rewritten(&outcome.workspace), left_out, hook_failed))
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
pub fn uncommit_files(path: &Path, commit: CommitId, files: &[Selected]) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::uncommit::commit_uncommit_changes(
        &mut ctx,
        commit.0,
        taken_specs(files)?,
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
    files: &[Selected],
) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::move_changes::commit_move_changes_between(
        &mut ctx,
        from.0,
        to.0,
        taken_specs(files)?,
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

/// Take `files` (or some of their hunks) out of `commit` as if it had never made them.
pub fn discard_from_commit(path: &Path, commit: CommitId, files: &[Selected]) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::discard_commit::commit_discard_changes(
        &mut ctx,
        commit.0,
        taken_specs(files)?,
        DryRun::No,
    )?;
    Ok(rewritten(&outcome.workspace))
}

/// Move `files` (or some of their hunks) out of `commit` into a new commit right above it,
/// in one step that one undo takes back. Returns the rewrites and the new commit.
pub fn split_off(
    path: &Path,
    commit: CommitId,
    files: &[Selected],
) -> Result<(Rewritten, CommitId)> {
    use but_oplog::legacy::{OperationKind, SnapshotDetails};
    let mut ctx = open(path)?;
    // The lock first: the metadata read before it could be written back over another
    // process's change.
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let changes = taken_specs(files)?;
    let (new, workspace) = but_transaction::with_transaction_with_perm(
        &mut ctx,
        &mut meta,
        guard.write_permission(),
        SnapshotDetails::new(OperationKind::MoveCommitFile),
        DryRun::No,
        |mut tx| {
            let blank = tx.insert_blank_commit(RelativeTo::Commit(commit.0), InsertSide::Above)?;
            let moved = tx.move_committed_changes_between(commit.0, blank.id, changes)?;
            Ok(but_transaction::Commit(CommitId(moved.id)))
        },
    )?;
    Ok((rewritten(&workspace), new))
}

/// Move `commits` into a new branch of their own, beside the other stacks, as one operation
/// to undo. Returns what moved, and the new branch's full name.
pub fn move_to_new_branch(path: &Path, commits: &[CommitId]) -> Result<(Rewritten, String)> {
    use but_oplog::legacy::{OperationKind, SnapshotDetails};
    let mut ctx = open(path)?;
    let name = branch_ref(&but_api::branch::branch_canned_name(&ctx)?)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let workspace = but_transaction::with_transaction_with_perm(
        &mut ctx,
        &mut meta,
        guard.write_permission(),
        SnapshotDetails::new(OperationKind::MoveCommit),
        DryRun::No,
        |mut tx| {
            tx.create_reference(name.as_ref(), None, None)?;
            tx.move_commits(
                commits.iter().map(|commit| commit.0),
                RelativeTo::Reference(name.clone()),
                InsertSide::Below,
            )
        },
    )?;
    Ok((rewritten(&workspace), name.to_string()))
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

/// Throw away the `selected` uncommitted changes of the main checkout, or of the linked
/// `worktree` with that name.
pub fn discard(path: &Path, worktree: Option<&str>, selected: &[Selected]) -> Result<()> {
    use but_oplog::legacy::{OperationKind, SnapshotDetails};
    let mut ctx = open(path)?;
    // One lock throughout, so what's conflicted can't change under the discard.
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let repo = &checkout(&ctx, worktree)?;
    // Discarding goes by status, which leaves conflicted files out: take them as they are,
    // then throw that away. Only whole: discarding part of one would take the rest of it
    // as resolved, markers and all.
    let conflicted = unmerged(repo, selected)?;
    if let Some(part) = (selected.iter())
        .find(|part| part.hunks.is_some() && conflicted.contains(&part.file.path.as_str()))
    {
        bail!(
            "{} has conflicts: discard all of it, or mark it resolved first",
            part.file.path
        );
    }
    refuse_changed(repo, selected)?;
    let conflicted: Vec<bstr::BString> = conflicted.into_iter().map(Into::into).collect();
    let specs = taken_specs(selected)?;
    // The snapshot is recorded before anything changes, as the but-api discard does: undo
    // then brings back whatever goes, conflicts included, even when some of it couldn't be
    // discarded. The oplog can't restore a linked worktree, so its discard has none.
    if worktree.is_none() {
        let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
            &ctx,
            SnapshotDetails::new(OperationKind::DiscardChanges),
            perm.read_permission(),
            DryRun::No,
        );
        if let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(&ctx, perm)) {
            tracing::warn!(?err, "couldn't record the discard for undo");
        }
    }
    let context_lines = ctx.settings.context_lines;
    let specs = current_hunks(repo, selected, specs, context_lines)?;
    // Conflicted files go last: resolving one takes its conflict away, so a failure after
    // that would lose it, and a linked worktree has no snapshot to bring it back.
    let (conflicted_specs, rest): (Vec<_>, Vec<_>) =
        (specs.into_iter()).partition(|spec| conflicted.contains(&spec.path));
    let mut rejected = but_workspace::discard_workspace_changes(repo, rest, context_lines)?;
    if !conflicted.is_empty() {
        but_workspace::resolve_worktree_conflicts(repo, conflicted)?;
        rejected.extend(but_workspace::discard_workspace_changes(
            repo,
            conflicted_specs,
            context_lines,
        )?);
    }
    if !rejected.is_empty() {
        // A file with nothing left to discard is as good as discarded.
        let left = but_core::diff::worktree_changes(repo)?.changes;
        if let Some(spec) = (rejected.iter()).find(|spec| left.iter().any(|c| c.path == spec.path))
        {
            bail!("Couldn't discard {}", spec.path);
        }
    }
    Ok(())
}

/// `specs`, the `selected` changes in that order, with their hunks as the worktree has them
/// now, picked lines set against the hunk around them. Hunks go by line numbers, so a file
/// whose hunks aren't where its diff had them, as when committing part of it since moved its
/// base without writing it, asks to try again on the fresh diff.
fn current_hunks(
    repo: &gix::Repository,
    selected: &[Selected],
    specs: Vec<DiffSpec>,
    context_lines: u32,
) -> Result<Vec<DiffSpec>> {
    let changes = but_core::diff::worktree_changes(repo)?.changes;
    let mut current = Vec::new();
    for (part, mut spec) in selected.iter().zip(specs) {
        if !spec.hunk_headers.is_empty() {
            let stale =
                || anyhow::anyhow!("{} changed since the diff was shown; try again", spec.path);
            let change = (changes.iter())
                .find(|change| change.path == spec.path)
                .filter(|change| super::file_change((*change).clone()).base() == part.file.base())
                .ok_or_else(stale)?;
            let hunks = match change.unified_patch(repo, context_lines)? {
                Some(but_core::UnifiedPatch::Patch { hunks, .. }) => hunks,
                _ => Vec::new(),
            };
            spec.hunk_headers = (spec.hunk_headers.iter())
                .map(|wanted| {
                    // Picked lines name one side only, as a commit takes them: a discard takes
                    // them out of the hunk around them, against its other side.
                    let within = |start: u32, lines: u32, outer: u32, outer_lines: u32| {
                        start >= outer && start + lines <= outer + outer_lines
                    };
                    let found = if wanted.old_lines == 0 {
                        (hunks.iter())
                            .find(|h| {
                                within(wanted.new_start, wanted.new_lines, h.new_start, h.new_lines)
                            })
                            .map(|hunk| but_core::HunkHeader {
                                old_start: hunk.old_start,
                                old_lines: hunk.old_lines,
                                ..*wanted
                            })
                    } else if wanted.new_lines == 0 {
                        (hunks.iter())
                            .find(|h| {
                                within(wanted.old_start, wanted.old_lines, h.old_start, h.old_lines)
                            })
                            .map(|hunk| but_core::HunkHeader {
                                new_start: hunk.new_start,
                                new_lines: hunk.new_lines,
                                ..*wanted
                            })
                    } else {
                        (hunks.iter())
                            .find(|h| {
                                h.old_start == wanted.old_start && h.old_lines == wanted.old_lines
                            })
                            .map(Into::into)
                    };
                    found.ok_or_else(stale)
                })
                .collect::<Result<_>>()?;
        }
        current.push(spec);
    }
    Ok(current)
}

/// Take the conflicted uncommitted `files` as they are on disk, markers or not.
pub fn mark_resolved(path: &Path, files: Vec<String>) -> Result<()> {
    let mut ctx = open(path)?;
    let paths = files.into_iter().map(Into::into).collect();
    // Undo can't restore a commit checked out for editing, so it isn't recorded.
    if super::edit::editing(&ctx)?.is_some() {
        let _guard = ctx.exclusive_worktree_access();
        return but_workspace::resolve_worktree_conflicts(&*ctx.repo.get()?, paths);
    }
    but_api::workspace::resolve_worktree_conflicts(&mut ctx, paths)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::process::Command;

    use super::*;
    use crate::backend::{DiffBody, DiffSource, FileDiff, Kept, Workspace, load_diff};

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    fn uncommitted(path: &Path) -> Vec<crate::backend::FileChange> {
        let shown = crate::backend::Workspace::default();
        let none = Default::default();
        crate::backend::reload_uncommitted(path, shown, true, &none)
            .unwrap()
            .uncommitted
    }

    /// A repository on `main` with one commit of `file` holding `content`. It signs nothing
    /// and reads hooks from its own `.git/hooks`, whatever the global config says.
    fn repo_with(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        git(path, &["init", "-q", "-b", "main"]);
        git(path, &["config", "user.name", "T"]);
        git(path, &["config", "user.email", "t@example.com"]);
        git(path, &["config", "commit.gpgsign", "false"]);
        git(path, &["config", "gitbutler.signCommits", "false"]);
        let hooks = path.join(".git/hooks");
        git(path, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        std::fs::write(path.join("file"), content).unwrap();
        git(path, &["add", "file"]);
        git(path, &["commit", "-q", "-m", "first"]);
        dir
    }

    /// A repository mid-merge, with `file` changed on both sides and left conflicted.
    fn conflicted_repo() -> (tempfile::TempDir, CommitId) {
        let dir = repo_with("base\n");
        let path = dir.path();
        let commit = |content: &str| {
            std::fs::write(path.join("file"), content).unwrap();
            git(path, &["commit", "-q", "-am", content.trim()]);
        };
        git(path, &["checkout", "-q", "-b", "other"]);
        commit("theirs\n");
        git(path, &["checkout", "-q", "main"]);
        commit("ours\n");
        let merge = Command::new("git")
            .current_dir(path)
            .args(["merge", "-q", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success(), "both sides changed the line");
        let head = git(path, &["rev-parse", "HEAD"]);
        let head = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        (dir, head)
    }

    #[test]
    fn a_conflicted_file_is_listed_and_kept_out_until_marked_resolved() {
        let (dir, head) = conflicted_repo();
        let path = dir.path();
        let files = uncommitted(path);
        assert_eq!(
            files.len(),
            1,
            "status alone leaves the file out: {files:?}"
        );
        assert!(files[0].conflicted, "its index entry is unmerged");

        let whole = Selected::whole(&files[0]);
        let refused = amend(path, head, std::slice::from_ref(&whole), None, false).unwrap_err();
        assert!(
            refused.to_string().contains("resolved first"),
            "the markers would go in: {refused:#}"
        );

        mark_resolved(path, vec!["file".into()]).unwrap();
        let files = uncommitted(path);
        assert_eq!(files.len(), 1, "the markers are still a change: {files:?}");
        assert!(!files[0].conflicted, "the index takes the file as it is");
    }

    #[test]
    fn discarding_a_conflicted_file_takes_it_back_to_head() {
        let (dir, _) = conflicted_repo();
        let path = dir.path();
        let files = uncommitted(path);
        let part = Selected {
            file: files[0].clone(),
            hunks: Some(Vec::new()),
        };
        let refused = discard(path, None, &[part]).unwrap_err();
        assert!(
            refused.to_string().contains("has conflicts"),
            "part of it would leave the rest resolved, markers and all: {refused:#}"
        );
        let whole = Selected::whole(&files[0]);
        discard(path, None, &[whole]).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            "ours\n"
        );
        let files = uncommitted(path);
        assert!(files.is_empty(), "nothing's left to commit: {files:?}");
    }

    #[test]
    fn an_added_file_is_discarded_whole_or_not_at_all() {
        let dir = repo_with("one\n");
        let path = dir.path();
        std::fs::write(path.join("new"), "one\ntwo\n").unwrap();

        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let DiffBody::Hunks { hunks, .. } = &diff[0].body else {
            panic!("a text diff");
        };
        let line = diff[0].part(Some(
            hunks[0].picked(&std::collections::BTreeSet::from([1])),
        ));
        let refused = discard(path, None, &[line]).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("Only all of new can be taken out"),
            "a line of it would take the rest along: {refused:#}"
        );
        assert!(path.join("new").exists(), "nothing went");
        let hunk = diff[0].part(Some(vec![hunks[0].key]));
        assert!(hunk.hunks.is_none(), "all of its hunks is the file whole");
        discard(path, None, std::slice::from_ref(&hunk)).unwrap();
        assert!(!path.join("new").exists(), "the whole file goes");
        discard(path, None, &[hunk]).expect("nothing left to discard is no error");
    }

    #[test]
    fn discarding_in_a_linked_worktree_leaves_the_main_checkout_alone() {
        let dir = repo_with("one\n");
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        let linked = outside.path().join("agent");
        git(
            path,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "agent",
                linked.to_str().unwrap(),
            ],
        );
        std::fs::write(path.join("file"), "main\n").unwrap();
        std::fs::write(linked.join("file"), "agent\n").unwrap();

        let source = DiffSource::Linked("agent".into());
        let diff = load_diff(path, source, None, None, None).unwrap();
        let whole = Selected::whole(&diff[0].file);
        discard(path, Some("agent"), &[whole]).unwrap();
        assert_eq!(
            std::fs::read_to_string(linked.join("file")).unwrap(),
            "one\n",
            "the worktree's file is back to its commit"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            "main\n",
            "the main checkout's change to the same path stays"
        );
    }

    #[test]
    fn a_failed_discard_in_a_linked_worktree_leaves_its_conflicts() {
        let dir = repo_with("base\n");
        let path = dir.path();
        let outside = tempfile::tempdir().unwrap();
        let linked = outside.path().join("agent");
        let linked_dir = linked.to_str().unwrap();
        git(path, &["worktree", "add", "-q", "-b", "agent", linked_dir]);
        git(path, &["checkout", "-q", "-b", "other"]);
        std::fs::write(path.join("file"), "theirs\n").unwrap();
        git(path, &["commit", "-q", "-am", "theirs"]);
        std::fs::write(linked.join("file"), "ours\n").unwrap();
        std::fs::write(linked.join("gone"), "one\n").unwrap();
        git(&linked, &["add", "gone"]);
        git(&linked, &["commit", "-q", "-am", "ours"]);
        let merge = Command::new("git")
            .current_dir(&linked)
            .args(["merge", "-q", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success(), "both sides changed the line");
        std::fs::write(linked.join("gone"), "two\n").unwrap();

        let diff = load_diff(path, DiffSource::Linked("agent".into()), None, None, None).unwrap();
        let selected: Vec<Selected> = (diff.iter())
            .map(|file| Selected {
                file: file.file.clone(),
                hunks: match &file.body {
                    DiffBody::Hunks { hunks, .. } if file.file.path == "gone" => {
                        Some(vec![hunks[0].key])
                    }
                    _ => None,
                },
            })
            .collect();
        assert_eq!(selected.len(), 2, "the conflicted file and the edit");
        // Deleted since the diff was shown, so its hunk can't be discarded alone.
        std::fs::remove_file(linked.join("gone")).unwrap();
        discard(path, Some("agent"), &selected).unwrap_err();
        assert_eq!(
            git(&linked, &["ls-files", "-u", "file"]).lines().count(),
            3,
            "with no snapshot to undo from, the conflict is still there to resolve"
        );
    }

    #[test]
    fn amending_some_hunks_leaves_the_rest_uncommitted() {
        let mut lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        lines[0] = "first edit\n".into();
        lines[19] = "last edit\n".into();
        std::fs::write(path.join("file"), lines.concat()).unwrap();

        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let DiffBody::Hunks { hunks, .. } = &diff[0].body else {
            panic!("a text diff");
        };
        assert_eq!(
            hunks.len(),
            2,
            "the edits are far enough apart for two hunks"
        );
        let head = git(path, &["rev-parse", "HEAD"]);
        let head = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        let first_hunk = Selected {
            file: diff[0].file.clone(),
            hunks: Some(vec![hunks[0].key]),
        };
        amend(path, head, &[first_hunk], None, false).unwrap();

        let amended = git(path, &["show", "HEAD:file"]);
        assert!(amended.starts_with("first edit\n"), "{amended}");
        assert!(amended.ends_with("line 20\n"), "{amended}");
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            lines.concat(),
            "the worktree keeps the last edit, now uncommitted"
        );
    }

    #[test]
    fn a_failing_pre_commit_hook_refuses_the_commit_and_passing_hooks_shape_it() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = repo_with("one\n");
        let path = dir.path();
        std::fs::write(path.join("file"), "two\n").unwrap();
        let hook = |name: &str, script: &str| {
            let file = path.join(".git/hooks").join(name);
            std::fs::write(&file, format!("#!/bin/sh\n{script}\n")).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        let changes = || vec![Selected::whole(&uncommitted(path)[0])];
        let main = Destination::Branch("refs/heads/main".into());
        let first = git(path, &["rev-parse", "HEAD"]);

        hook("pre-commit", "echo 'lint failed'; exit 1");
        let refused = commit(path, &main, &changes(), None, "Two", true).unwrap_err();
        assert!(
            format!("{refused:#}").contains("lint failed"),
            "the error is what the hook printed: {refused:#}"
        );
        assert_eq!(
            git(path, &["rev-parse", "HEAD"]),
            first,
            "nothing was committed"
        );
        assert_eq!(
            uncommitted(path).len(),
            1,
            "the change is still uncommitted"
        );

        hook("pre-commit", "exit 0");
        hook("commit-msg", "echo 'Two, checked' > \"$1\"");
        hook("post-commit", "echo 'notify failed'; exit 1");
        let (_, _, hook_failed) = commit(path, &main, &changes(), None, "Two", true).unwrap();
        assert_eq!(
            git(path, &["log", "-1", "--format=%s"]).trim(),
            "Two, checked",
            "the commit takes the message commit-msg rewrote"
        );
        assert!(uncommitted(path).is_empty(), "the commit stands");
        assert!(
            hook_failed.is_some_and(|output| output.contains("notify failed")),
            "post-commit's failure comes back to report"
        );
    }

    #[test]
    fn committing_no_changes_makes_an_empty_commit() {
        let dir = repo_with("one\n");
        let path = dir.path();
        let main = Destination::Branch("refs/heads/main".into());
        let first = git(path, &["rev-parse", "HEAD"]);
        commit(path, &main, &[], None, "Mark it reviewed", false).unwrap();
        assert_eq!(
            git(path, &["log", "-1", "--format=%s"]).trim(),
            "Mark it reviewed",
            "the commit is made"
        );
        assert_eq!(
            git(path, &["rev-parse", "HEAD~1"]),
            first,
            "it sits on the last commit"
        );
        assert_eq!(
            git(path, &["diff", "--stat", "HEAD~1", "HEAD"]),
            "",
            "it changes nothing"
        );
    }

    #[test]
    fn a_new_branch_outside_the_workspace_goes_on_top_of_the_checked_out_one() {
        let dir = repo_with("one\n");
        let path = dir.path();
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        crate::backend::settings::set_target(path, "refs/remotes/origin/main").unwrap();

        assert_eq!(
            create_branch(path, Some("feature")).unwrap(),
            "refs/heads/feature"
        );
        assert_eq!(
            git(path, &["symbolic-ref", "HEAD"]).trim(),
            "refs/heads/feature",
            "it's checked out, as `git checkout -b` would"
        );
        let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
        assert!(!workspace.managed, "still outside the workspace");
        assert_eq!(workspace.checked_out.as_deref(), Some("feature"));
    }

    #[test]
    fn a_reload_of_the_commits_keeps_the_files_while_their_tree_stays() {
        let dir = repo_with("one\n");
        let path = dir.path();
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        crate::backend::settings::set_target(path, "refs/remotes/origin/main").unwrap();
        git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
        git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
        let head = git(path, &["rev-parse", "HEAD"]);
        let top = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        std::fs::write(path.join("file"), "two\n").unwrap();
        let load = |kept| crate::backend::load_workspace(path, kept).unwrap();
        let paths = |ws: &Workspace| {
            ws.uncommitted
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<_>>()
        };
        let shown = load(Kept::default());

        reword(path, top, "reworded").unwrap();
        // Written after the files were shown, so only reading them again finds it.
        std::fs::write(path.join("new"), "new\n").unwrap();
        let commits = load(Kept::from(&shown, false, &HashSet::new()));
        let title = (commits.branches().find_map(|b| b.commits.first())).map(|c| &c.title);
        assert_eq!(
            title.map(String::as_str),
            Some("reworded"),
            "the commits are read"
        );
        assert_eq!(
            paths(&commits),
            vec!["file"],
            "the files are the ones shown"
        );
        let files = load(Kept::from(&shown, true, &HashSet::new()));
        assert_eq!(
            paths(&files),
            vec!["file", "new"],
            "unless they're asked for"
        );

        // A commit taking the change in gives `HEAD` another tree, and so another diff.
        git(path, &["commit", "-qam", "take it in"]);
        let moved = load(Kept::from(&commits, false, &HashSet::new()));
        assert_eq!(
            paths(&moved),
            vec!["new"],
            "or the tree they were read against moved"
        );
    }

    #[test]
    fn naming_an_unnamed_branch_makes_it_a_stack_and_undoes() {
        let dir = repo_with("one\n");
        let path = dir.path();
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        crate::backend::settings::set_target(path, "refs/remotes/origin/main").unwrap();
        // A commit made with plain Git on the workspace branch belongs to no branch.
        git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
        git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
        let head = git(path, &["rev-parse", "HEAD"]);
        let top = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        let branch_at_top = |path: &Path| {
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            (workspace.branches())
                .find(|b| b.commits.first().map(|c| c.id) == Some(top))
                .map(|b| b.ref_name.clone())
        };
        assert_eq!(branch_at_top(path).as_deref(), Some(""), "it shows unnamed");

        assert_eq!(name_branch(path, top, "named").unwrap(), "refs/heads/named");
        assert_eq!(branch_at_top(path).as_deref(), Some("refs/heads/named"));

        crate::backend::history::undo(path, false, &Default::default()).unwrap();
        assert_eq!(
            branch_at_top(path).as_deref(),
            Some(""),
            "undo takes the name away"
        );
    }

    #[test]
    fn a_hunk_discard_queued_behind_another_in_its_file_asks_to_try_again() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        let mut edited = lines.clone();
        edited[1] = "two\nmore\nlines\n".into();
        edited[18] = "last edit\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();

        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let DiffBody::Hunks { hunks, .. } = &diff[0].body else {
            panic!("a text diff");
        };
        assert_eq!(
            hunks.len(),
            2,
            "the edits are far enough apart for two hunks"
        );
        let hunk = |ix: usize| Selected {
            file: diff[0].file.clone(),
            hunks: Some(vec![hunks[ix].key]),
        };
        discard(path, None, &[hunk(0)]).unwrap();
        // Read before the first discard, which took two lines out above it.
        let err = discard(path, None, &[hunk(1)]).unwrap_err();
        assert!(
            err.to_string().contains("changed since the diff was shown"),
            "its place in the file is from before the first discard: {err}"
        );
        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let DiffBody::Hunks { hunks, .. } = &diff[0].body else {
            panic!("a text diff");
        };
        let fresh = Selected {
            file: diff[0].file.clone(),
            hunks: Some(vec![hunks[0].key]),
        };
        discard(path, None, &[fresh]).unwrap();
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            lines.concat(),
            "tried again on the fresh diff, each discard took back just its own hunk"
        );
    }

    #[test]
    fn a_hunk_discard_after_its_files_base_moved_asks_to_try_again() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        let mut edited = lines.clone();
        edited[1] = "two\n".into();
        edited[18] = "nineteen\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let DiffBody::Hunks { hunks, .. } = &diff[0].body else {
            panic!("a text diff");
        };
        let second = Selected {
            file: diff[0].file.clone(),
            hunks: Some(vec![hunks[1].key]),
        };
        // Commit the first edit alone, as committing part of the file does, without touching
        // the file itself.
        let mut first = lines.clone();
        first[1] = "two\n".into();
        std::fs::write(path.join("staged"), first.concat()).unwrap();
        let blob = git(path, &["hash-object", "-w", "staged"]);
        let entry = format!("100644,{},file", blob.trim());
        git(path, &["update-index", "--cacheinfo", &entry]);
        git(path, &["commit", "-q", "-m", "first edit"]);
        let err = discard(path, None, &[second]).unwrap_err();
        assert!(
            err.to_string().contains("changed since the diff was shown"),
            "its hunk was read against the file before the commit: {err}"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            edited.concat(),
            "nothing was discarded"
        );
    }

    /// The file of `repo_with` one line at a time, and the index of the line in its hunk
    /// whose text is `text`.
    fn line_in(hunk: &crate::backend::Hunk, text: &str) -> usize {
        (hunk.lines.iter())
            .position(|line| &*line.text == text)
            .unwrap_or_else(|| panic!("{text} is in the hunk"))
    }

    #[test]
    fn discarding_picked_lines_keeps_the_rest_of_their_hunk() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        let mut edited = lines.clone();
        edited[4] = "five a\nfive b\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();

        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let hunk = &diff[0].hunks()[0];
        let picks = std::collections::BTreeSet::from([line_in(hunk, "five b")]);
        let lines_picked = Selected {
            file: diff[0].file.clone(),
            hunks: Some(hunk.picked(&picks)),
        };
        discard(path, None, &[lines_picked]).unwrap();

        let mut expected = lines.clone();
        expected[4] = "five a\n".into();
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            expected.concat(),
            "only the picked line went; the rest of its hunk stays uncommitted"
        );
    }

    /// A workspace whose one commit, on top of the target, turns line 5 of `file` into two,
    /// with that commit's id and diff.
    fn workspace_with_an_edit() -> (tempfile::TempDir, CommitId, Vec<FileDiff>, Vec<String>) {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        crate::backend::settings::set_target(path, "refs/remotes/origin/main").unwrap();
        git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
        let mut edited = lines.clone();
        edited[4] = "five a\nfive b\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        git(path, &["commit", "-q", "-am", "edit"]);
        let head = git(path, &["rev-parse", "HEAD"]);
        let commit = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        let diff = load_diff(path, DiffSource::Commit(commit), None, None, None).unwrap();
        (dir, commit, diff, edited)
    }

    /// The line "five b" of `diff`'s one hunk, picked to take out of its commit.
    fn five_b_out(diff: &[FileDiff]) -> Selected {
        let hunk = &diff[0].hunks()[0];
        let picks = std::collections::BTreeSet::from([line_in(hunk, "five b")]);
        Selected {
            file: diff[0].file.clone(),
            hunks: Some(hunk.picked_out(&picks)),
        }
    }

    #[test]
    fn uncommitting_picked_lines_leaves_them_uncommitted() {
        let (dir, commit, diff, edited) = workspace_with_an_edit();
        let path = dir.path();
        let rewritten = uncommit_files(path, commit, &[five_b_out(&diff)]).unwrap();
        let commit = rewritten.get(&commit).copied().unwrap_or(commit);
        let committed = git(path, &["show", &format!("{}:file", commit.0)]);
        assert!(committed.contains("five a\n"), "{committed}");
        assert!(
            !committed.contains("five b"),
            "the picked line left the commit"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            edited.concat(),
            "and stays in the worktree, uncommitted"
        );
    }

    #[test]
    fn splitting_off_picked_lines_puts_them_in_a_commit_above() {
        let (dir, commit, diff, _) = workspace_with_an_edit();
        let path = dir.path();
        let (rewritten, new) = split_off(path, commit, &[five_b_out(&diff)]).unwrap();
        let commit = rewritten.get(&commit).copied().unwrap_or(commit);
        let below = git(path, &["show", &format!("{}:file", commit.0)]);
        let above = git(path, &["show", &format!("{}:file", new.0)]);
        assert!(!below.contains("five b"), "{below}");
        assert!(above.contains("five a\nfive b\n"), "{above}");
    }

    #[test]
    fn discarding_picked_lines_from_a_commit_takes_them_out_of_it_and_the_worktree() {
        let (dir, commit, diff, _) = workspace_with_an_edit();
        let path = dir.path();
        let rewritten = discard_from_commit(path, commit, &[five_b_out(&diff)]).unwrap();
        let commit = rewritten.get(&commit).copied().unwrap_or(commit);
        let committed = git(path, &["show", &format!("{}:file", commit.0)]);
        assert!(
            committed.contains("five a\n") && !committed.contains("five b"),
            "{committed}"
        );
        let on_disk = std::fs::read_to_string(path.join("file")).unwrap();
        assert!(!on_disk.contains("five b"), "{on_disk}");
    }

    #[test]
    fn moving_picked_lines_takes_them_into_the_other_commit() {
        let (dir, commit, diff, _) = workspace_with_an_edit();
        let path = dir.path();
        let empty = insert_empty_commit(path, &Destination::AboveCommit(commit)).unwrap();
        let rewritten = move_files(path, commit, empty, &[five_b_out(&diff)]).unwrap();
        let (commit, empty) = (
            rewritten.get(&commit).copied().unwrap_or(commit),
            rewritten.get(&empty).copied().unwrap_or(empty),
        );
        let below = git(path, &["show", &format!("{}:file", commit.0)]);
        let above = git(path, &["show", &format!("{}:file", empty.0)]);
        assert!(!below.contains("five b"), "{below}");
        assert!(above.contains("five a\nfive b\n"), "{above}");
    }

    #[test]
    fn amending_picked_lines_commits_just_those() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        let mut edited = lines.clone();
        edited[4] = "five a\nfive b\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();

        let diff = load_diff(path, DiffSource::Worktree, None, None, None).unwrap();
        let hunk = &diff[0].hunks()[0];
        let picks = std::collections::BTreeSet::from([line_in(hunk, "five a")]);
        let picked = Selected {
            file: diff[0].file.clone(),
            hunks: Some(hunk.picked(&picks)),
        };
        let head = git(path, &["rev-parse", "HEAD"]);
        let head = CommitId(gix::ObjectId::from_hex(head.trim().as_bytes()).unwrap());
        amend(path, head, &[picked], None, false).unwrap();

        let amended = git(path, &["show", "HEAD:file"]);
        // Just the added line: line 5, whose removal wasn't picked, stays below it.
        assert!(amended.contains("line 4\nfive a\nline 5\n"), "{amended}");
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            edited.concat(),
            "the rest stays uncommitted on disk"
        );
    }
}
