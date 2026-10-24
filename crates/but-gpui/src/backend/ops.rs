//! Mutations of commits, branches and uncommitted changes. Each opens the project, calls
//! the matching `but-api` function and returns; the UI reloads the workspace afterwards.
//! Undo history comes from `but-api`'s own oplog snapshots, except that [`discard`],
//! [`split_off`], [`move_to_new_branch`] and [`create_beside_checkout`] record their own, as
//! does [`super::conflicts::resolve_markers`] through [`record_undo`].

use gitbutler_repo::hooks::{ErrorData, HookResult, MessageHookResult};
use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use but_api::commit::json::ChangesSource;
use but_core::tree::create_tree::RejectionReason;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

use super::{ChangeKind, CommitId, Selected, checkout, open, short_ref, stamp};

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
            (index.entry_index_by_path(path.as_bytes().as_bstr()).ok())
                .and_then(|ix| index.entries().get(ix))
                .is_some_and(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
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
    refuse_changed(&repo, selected, false)
}

/// Refuse some of an uncommitted file written to since its diff was read, as by an agent,
/// or whose base moved, as when the CLI committed part of it: its hunks and lines are placed
/// by line numbers that either moves, and the engine would take whatever lies there now.
/// With `whole`, refuse whole files too, for what can't be undone.
fn refuse_changed(repo: &gix::Repository, selected: &[Selected], whole: bool) -> Result<()> {
    let head = repo.head_tree().ok();
    let base_now = |part: &Selected| {
        let path = part
            .file
            .previous_path
            .as_deref()
            .unwrap_or(&part.file.path);
        let entry = head.as_ref()?.lookup_entry_by_path(path).ok()??;
        Some(entry.object_id())
    };
    // A whole file goes back to its base whatever that is, so only being written to counts.
    let changed = (selected.iter()).find(|part| {
        let picked = part.hunks.is_some();
        (whole || picked)
            && (stamp(repo, part.file.path.as_str().into()) != (part.file.modified, part.file.size)
                || (picked && base_now(part) != part.file.base()))
    });
    if let Some(part) = changed {
        bail!(
            "{} changed since the diff was shown; try again",
            part.file.path
        );
    }
    Ok(())
}

/// Record the project as it is, so undo brings back what `kind` is about to change. One that
/// can't be recorded only logs: the operation goes ahead, without its undo step.
pub(super) fn record_undo(
    ctx: &but_ctx::Context,
    kind: but_oplog::legacy::OperationKind,
    perm: &mut but_ctx::access::RepoExclusive,
) {
    let details = but_oplog::legacy::SnapshotDetails::new(kind);
    let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        details,
        perm.read_permission(),
        DryRun::No,
    );
    if let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(ctx, perm)) {
        tracing::warn!(?err, ?kind, "couldn't record the operation for undo");
    }
}

/// Where changes are read from: the main checkout, or the linked `worktree`.
pub(super) fn changes_source(worktree: Option<&str>) -> ChangesSource {
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
pub(super) fn branch_ref(name: &str) -> Result<gix::refs::FullName> {
    let name = but_core::branch::normalize_short_name(name.trim())?;
    full_name(&format!("refs/heads/{name}"))
}

/// What a hook printed, as an error card shows it: its last lines first, where a linter or
/// test runner says what failed, without the labels put before each of its streams, then all
/// of it, for copying.
pub(super) fn hook_output(output: &str) -> String {
    let said: Vec<&str> = (output.lines())
        .map(str::trim_end)
        .filter(|line| !matches!(line.trim(), "" | "stdout:" | "stderr:"))
        .collect();
    match said.len() {
        ..=3 => said.join("\n"),
        n => format!(
            "{}\n\n{output}",
            said.get(n - 3..).unwrap_or_default().join("\n")
        ),
    }
}

/// Refuse to change the checkout while Git is partway through its own operation there, which
/// would go on from where it stopped, past what's done meanwhile: a rebase or a merge, or a
/// bisect, whose reset leaves behind what's committed on the commit it checked out. With
/// `on_branch`, as the main checkout must be, also while no branch is checked out at all.
fn refuse_unfinished(repo: &gix::Repository, on_branch: bool) -> Result<()> {
    if let Some(op) = super::git_operation(repo) {
        bail!(
            "{op} isn't finished. Finish it with `{op} --continue`, or stop it with \
             `{op} --abort`, first."
        );
    }
    if matches!(repo.state(), Some(gix::state::InProgress::Bisect)) {
        bail!("git bisect isn't finished. End it with `git bisect reset` first.");
    }
    if on_branch && repo.head()?.is_detached() {
        bail!(
            "No branch is checked out (HEAD is detached). Check one out, or make one with \
             `git switch -c <name>`, first."
        );
    }
    Ok(())
}

/// Run the pre-commit hook on what `changes` would commit; its failure refuses the commit.
fn pre_commit(ctx: &but_ctx::Context, changes: &[DiffSpec]) -> Result<()> {
    let result = but_api::legacy::repo::pre_commit_hook_diffspecs(ctx, changes.to_vec())?;
    if let HookResult::Failure(ErrorData { error }) = result {
        bail!(
            "The pre-commit hook refused the commit:\n{}",
            hook_output(&error)
        );
    }
    Ok(())
}

/// Run the post-commit hook, and say what it printed if it failed: the commit stands.
fn post_commit(ctx: &but_ctx::Context) -> Option<String> {
    match but_api::legacy::repo::post_commit_hook(ctx) {
        Ok(HookResult::Failure(ErrorData { error })) => Some(hook_output(&error)),
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
    refuse_unfinished(&checkout(&ctx, worktree)?, worktree.is_none())?;
    refuse_conflicted(&ctx, worktree, selected)?;
    // The hooks run in the main checkout, not where a linked worktree's changes are.
    let run_hooks = run_hooks && worktree.is_none();
    // No changes make an empty commit.
    let changes = selected_specs(selected);
    let mut message = message.to_owned();
    if run_hooks {
        pre_commit(&ctx, &changes)?;
        // Ending in a line break, as Git hands it over, so what a hook appends starts a line
        // of its own rather than running on from the title.
        match but_api::legacy::repo::message_hook(&ctx, format!("{message}\n"))? {
            MessageHookResult::Failure(ErrorData { error }) => {
                bail!(
                    "The commit-msg hook refused the commit:\n{}",
                    hook_output(&error)
                )
            }
            MessageHookResult::Message(rewritten) => {
                message = rewritten.message.trim_end().to_owned()
            }
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

/// Create a new stack, named `name` or with a made-up name, and return its full ref name.
/// With the default branch checked out on its own, the new branch goes on top of it and is
/// checked out, as `git checkout -b` would. Beside another branch, both go in GitButler's
/// workspace, as `but branch new` does.
pub fn create_branch(path: &Path, name: Option<&str>) -> Result<String> {
    use but_api::branch::json::BranchCreatePlacement as Placement;
    let new_ref = name.map(branch_ref).transpose()?;
    let mut ctx = open(path)?;
    refuse_unborn(&ctx)?;
    // A branch made here and checked out couldn't be undone: the oplog takes a detached
    // HEAD for the workspace and would restore that instead.
    refuse_unfinished(&*ctx.repo.get()?, true)?;
    let head = ctx.repo.get()?.head_name()?;
    let checked_out = head.filter(|head| {
        head.as_bstr() != but_core::WORKSPACE_REF_NAME
            && head.as_ref().category() == Some(gix::refs::Category::LocalBranch)
    });
    let target = ctx.project_meta()?.target_ref;
    let placement = match checked_out {
        Some(head)
            if (target.as_ref())
                .is_some_and(|target| target.as_local_full_name_ref() != head.as_ref()) =>
        {
            return create_beside_checkout(&mut ctx, &head, new_ref);
        }
        Some(head) => Placement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(head),
            side: InsertSide::Above,
        },
        None => Placement::Independent { order: None },
    };
    let created = but_api::branch::branch_create(&mut ctx, new_ref, placement)?;
    Ok(created.new_ref.to_string())
}

/// Make a branch, `new_ref` or with a made-up name, where `head`, the branch checked out,
/// leaves the target, and apply it beside `head`, which brings in GitButler's workspace for the
/// two. Returns its full name. Starting there, as `but branch new` does, the two merge without
/// conflicts, whatever the target has done since.
///
/// A stopgap until `but_api::branch::branch_create` does this itself: it refuses a branch of
/// its own outside the workspace, and the CLI's way around that lives in the CLI. This is
/// `branch_checkout_new` with the branch applied rather than checked out, in one step to undo.
fn create_beside_checkout(
    ctx: &mut but_ctx::Context,
    head: &gix::refs::FullName,
    new_ref: Option<gix::refs::FullName>,
) -> Result<String> {
    use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let (new_ref, base) = {
        let repo = ctx.repo.get()?;
        let new_ref = match new_ref {
            // As `branch_create` refuses one taken locally or by a remote's branch, or nesting
            // with another.
            Some(new_ref) => {
                if but_core::branch::find_unique_refname(&repo, new_ref.as_ref())? != new_ref {
                    bail!("Reference '{}' already exists", new_ref.shorten());
                }
                but_core::branch::refuse_colliding_refname(&repo, new_ref.as_ref(), None)?;
                new_ref
            }
            None => but_core::branch::unique_canned_refname(&repo)?,
        };
        let target = ctx.project_meta()?.resolve_target_commit(&repo)?;
        let base = (repo.merge_base(repo.head_id()?, target)?)
            .with_context(|| format!("{} shares no history with the target", head.shorten()))?;
        (new_ref, base.detach())
    };
    // It lists the new branch, so undo deletes it again.
    let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch)
            .with_trailers([Trailer::Name(new_ref.to_string())]),
        &[new_ref.as_ref()],
        perm.read_permission(),
        DryRun::No,
    );
    let previous = gix::refs::transaction::PreviousValue::MustNotExist;
    (ctx.repo.get()?)
        .reference(new_ref.as_ref(), base, previous, "GitButler: new branch")
        .with_context(|| format!("Couldn't create branch {}", new_ref.shorten()))?;
    let applied = but_api::branch::apply_only_with_perm(ctx, new_ref.as_ref(), perm);
    if !(applied.as_ref()).is_ok_and(|outcome| outcome.status.persisted_mutation()) {
        ctx.repo.get()?.find_reference(new_ref.as_ref())?.delete()?;
        applied?;
        bail!(
            "Couldn't start {} beside {}",
            new_ref.shorten(),
            head.shorten()
        );
    }
    if let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(ctx, perm)) {
        tracing::warn!(?err, "couldn't record the new branch for undo");
    }
    Ok(new_ref.to_string())
}

/// Stack a new branch with a made-up name on top of `below` (a full ref name).
pub fn create_dependent_branch(path: &Path, below: &str) -> Result<String> {
    let relative_to = but_api::commit::json::RelativeTo::Reference(full_name(below)?);
    create_branch_at(path, relative_to, InsertSide::Above)
}

/// Make a new branch, with a made-up name, whose newest commit is `commit`, taking it and the
/// commits below it in its branch; or with `below`, one that ends just under `commit`.
pub fn create_branch_at_commit(path: &Path, commit: CommitId, below: bool) -> Result<String> {
    let side = if below {
        InsertSide::Below
    } else {
        InsertSide::Above
    };
    let relative_to = but_api::commit::json::RelativeTo::Commit(commit.0);
    create_branch_at(path, relative_to, side)
}

fn create_branch_at(
    path: &Path,
    relative_to: but_api::commit::json::RelativeTo,
    side: InsertSide,
) -> Result<String> {
    let mut ctx = open(path)?;
    let created = but_api::branch::branch_create(
        &mut ctx,
        None,
        but_api::branch::json::BranchCreatePlacement::Dependent { relative_to, side },
    )?;
    Ok(created.new_ref.to_string())
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
    let from = full_name(ref_name)?;
    let outcome =
        but_api::branch::branch_rename(&mut ctx, from.clone(), new_name.trim().to_owned())?;
    // Its description goes with it, as Git keeps it by the name.
    let moved = super::branches::move_description(
        &*ctx.repo.get()?,
        from.shorten(),
        Some(outcome.new_ref.shorten()),
    );
    if let Err(err) = moved {
        tracing::warn!(?err, "couldn't move the renamed branch's description");
    }
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

pub fn remove_branch(path: &Path, ref_name: &str, tip: Option<CommitId>) -> Result<()> {
    let mut ctx = open(path)?;
    let name = full_name(ref_name)?;
    // Removing a ref that's gone does nothing, so say so rather than that it went, as after
    // an agent renamed the branch while the confirmation was up. With `tip`, one that moved
    // since stays: its new commits would go with it.
    match super::ref_tip(&*ctx.repo.get()?, name.as_ref()) {
        None => bail!("{} was renamed or deleted meanwhile", short_ref(ref_name)),
        Some(at) if tip.is_some_and(|tip| tip.0 != at) => {
            bail!(
                "{} changed since you asked, so it wasn't deleted",
                short_ref(ref_name)
            )
        }
        Some(_) => {}
    }
    but_api::branch::branch_remove(&mut ctx, name.clone())?;
    // So a branch made later under its name starts without its description.
    if let Err(err) = super::branches::move_description(&*ctx.repo.get()?, name.shorten(), None) {
        tracing::warn!(?err, "couldn't drop the deleted branch's description");
    }
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
    refuse_unfinished(&checkout(&ctx, worktree)?, worktree.is_none())?;
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
        // There's no kind for a split; it makes a commit, rather than moving a file.
        SnapshotDetails::new(OperationKind::CreateCommit),
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
    let perm = guard.write_permission();
    // It lists the new branch, so undo deletes it again.
    let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        &ctx,
        SnapshotDetails::new(OperationKind::MoveCommit),
        &[name.as_ref()],
        perm.read_permission(),
        DryRun::No,
    );
    let (rolled_back, workspace) = but_transaction::with_transaction_with_perm_only(
        &mut ctx,
        &mut meta,
        perm,
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
    // The commits moved already, so a snapshot that can't be recorded only logs.
    if !rolled_back && let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(&ctx, perm)) {
        tracing::warn!(?err, "couldn't record the new branch for undo");
    }
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

/// Drop `commits` and everything they changed.
pub fn discard_commits(path: &Path, commits: &[CommitId]) -> Result<()> {
    let mut ctx = open(path)?;
    let commits = commits.iter().map(|id| id.0).collect();
    but_api::commit::discard_commit::commit_discard(&mut ctx, commits, DryRun::No)?;
    Ok(())
}

/// Throw away the `selected` uncommitted changes of the main checkout, or of the linked
/// `worktree` with that name.
pub fn discard(path: &Path, worktree: Option<&str>, selected: &[Selected]) -> Result<()> {
    use but_oplog::legacy::OperationKind;
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
    // A linked worktree's discard can't be undone, so not of what was written after the
    // diff was shown, as by an agent while the confirmation was up.
    refuse_changed(repo, selected, worktree.is_some())?;
    let conflicted: Vec<bstr::BString> = conflicted.into_iter().map(Into::into).collect();
    // Hunks that moved since the diff was shown refuse it before anything is recorded.
    let context_lines = ctx.settings.context_lines;
    let specs = current_hunks(repo, selected, taken_specs(selected)?, context_lines)?;
    // The snapshot is recorded before anything changes, as the but-api discard does: undo
    // then brings back whatever goes, conflicts included, even when some of it couldn't be
    // discarded. The oplog can't restore a linked worktree, so its discard has none.
    if worktree.is_none() {
        record_undo(&ctx, OperationKind::DiscardChanges, perm);
    }
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
    // Whole files have no hunks to find.
    if specs.iter().all(|spec| spec.hunk_headers.is_empty()) {
        return Ok(specs);
    }
    let changes = but_core::diff::worktree_changes(repo)?.changes;
    let mut current = Vec::new();
    for (part, mut spec) in selected.iter().zip(specs) {
        if !spec.hunk_headers.is_empty() {
            let stale =
                || anyhow::anyhow!("{} changed since the diff was shown; try again", spec.path);
            let change = (changes.iter())
                .find(|change| change.path == spec.path)
                .filter(|change| {
                    let base = change.status.previous_state_and_path().map(|(s, _)| s.id);
                    base == part.file.base()
                })
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
pub fn mark_resolved(path: &Path, worktree: Option<&str>, files: Vec<String>) -> Result<()> {
    let mut ctx = open(path)?;
    let paths = files.into_iter().map(Into::into).collect();
    // Undo can't restore a linked worktree, or a commit checked out for editing, so neither
    // is recorded.
    if worktree.is_some() || super::edit::editing(&ctx)?.is_some() {
        let _guard = ctx.exclusive_worktree_access();
        return but_workspace::resolve_worktree_conflicts(&checkout(&ctx, worktree)?, paths);
    }
    but_api::workspace::resolve_worktree_conflicts(&mut ctx, paths)
}

#[cfg(test)]
pub(super) mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::process::Command;

    use super::*;
    use crate::backend::{DiffBody, DiffSource, FileDiff, Kept, Workspace, load_diff};

    /// A linked worktree of `path` on a new branch `agent`, in a directory of its own. Dropping
    /// the directory deletes the worktree.
    pub(in crate::backend) fn agent_worktree(path: &Path) -> (tempfile::TempDir, PathBuf) {
        let outside = tempfile::tempdir().unwrap();
        let linked = outside.path().join("agent");
        let linked_dir = linked.to_str().unwrap();
        git(path, &["worktree", "add", "-q", "-b", "agent", linked_dir]);
        (outside, linked)
    }

    /// HEAD's commit in the repository at `dir`.
    pub(in crate::backend) fn head(dir: &Path) -> CommitId {
        let id = git(dir, &["rev-parse", "HEAD"]);
        CommitId(gix::ObjectId::from_hex(id.trim().as_bytes()).unwrap())
    }

    /// "line 1" to "line 20", a line each.
    fn twenty_lines() -> Vec<String> {
        (1..=20).map(|n| format!("line {n}\n")).collect()
    }

    pub(in crate::backend) fn git(dir: &Path, args: &[&str]) -> String {
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

    /// Give the repository at `path` a remote whose `main` is at `HEAD`, tracked by the local
    /// `main`, and make that the target.
    fn target_origin_main(path: &Path) {
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
        crate::backend::settings::set_target(path, "refs/heads/main").unwrap();
    }

    /// A repository on `main` with one commit of `file` holding `content`. It signs nothing,
    /// checks files out with the line endings they were committed with, and reads hooks from its
    /// own `.git/hooks`, whatever the global config says.
    pub(in crate::backend) fn repo_with(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        git(path, &["init", "-q", "-b", "main"]);
        git(path, &["config", "user.name", "T"]);
        git(path, &["config", "user.email", "t@example.com"]);
        git(path, &["config", "commit.gpgsign", "false"]);
        git(path, &["config", "gitbutler.signCommits", "false"]);
        // Git for Windows turns this on by default, which would check files out with CRLF.
        git(path, &["config", "core.autocrlf", "false"]);
        let hooks = path.join(".git/hooks");
        git(path, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        std::fs::write(path.join("file"), content).unwrap();
        git(path, &["add", "file"]);
        git(path, &["commit", "-q", "-m", "first"]);
        dir
    }

    #[test]
    fn a_description_is_listed_and_follows_its_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::branches::{
                all_branches, move_description, set_branch_description,
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "topic"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Topic"]);
            git(path, &["checkout", "-q", "main"]);

            set_branch_description(path, "refs/heads/topic", "  Why it's there \n").unwrap();
            let listed = (all_branches(path).unwrap().into_iter())
                .find(|branch| branch.name == "topic")
                .and_then(|branch| branch.description);
            assert_eq!(
                listed.as_deref(),
                Some("Why it's there"),
                "kept where Git keeps it, trimmed, and listed with the branch"
            );

            let repo = gix::open(path).unwrap();
            let kept = |name: &str| {
                let config =
                    but_core::git_config::open_repo_local_config_for_reading(&repo).unwrap();
                config
                    .string(format!("branch.{name}.description").as_str())
                    .map(|d| d.to_string())
            };
            move_description(&repo, "topic".into(), Some("renamed".into())).unwrap();
            assert_eq!(
                (kept("topic"), kept("renamed").as_deref()),
                (None, Some("Why it's there")),
                "it moves to the new name, as `git branch -m` moves it"
            );
            move_description(&repo, "renamed".into(), None).unwrap();
            assert_eq!(
                kept("renamed"),
                None,
                "and goes with the branch, as `git branch -d` drops it"
            );
            assert!(
                set_branch_description(path, "refs/heads/renamed", "Lost?").is_err(),
                "a branch that's gone isn't given one, for a later branch by its name to take up"
            );
        })
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
        let head = head(path);
        (dir, head)
    }

    #[test]
    fn a_conflicted_file_is_listed_and_kept_out_until_marked_resolved() {
        but_testsupport::isolated_app_data_dir(|| {
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
            // As an update leaves it: conflicted, with no merge of Git's to finish.
            std::fs::remove_file(path.join(".git/MERGE_HEAD")).unwrap();
            let refused = amend(path, head, std::slice::from_ref(&whole), None, false).unwrap_err();
            assert!(
                refused.to_string().contains("resolved first"),
                "the markers would go in: {refused:#}"
            );

            mark_resolved(path, None, vec!["file".into()]).unwrap();
            let files = uncommitted(path);
            assert_eq!(files.len(), 1, "the markers are still a change: {files:?}");
            assert!(!files[0].conflicted, "the index takes the file as it is");
        })
    }

    #[test]
    fn discarding_a_conflicted_file_takes_it_back_to_head() {
        but_testsupport::isolated_app_data_dir(|| {
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
        })
    }

    #[test]
    fn an_added_file_is_discarded_whole_or_not_at_all() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            std::fs::write(path.join("new"), "one\ntwo\n").unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
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
        })
    }

    #[test]
    fn discarding_in_a_linked_worktree_leaves_the_main_checkout_alone() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let (_outside, linked) = agent_worktree(path);
            std::fs::write(path.join("file"), "main\n").unwrap();
            std::fs::write(linked.join("file"), "agent\n").unwrap();

            let read = || {
                let source = DiffSource::Linked("agent".into());
                let diff = load_diff(path, source, None, None, None, None).unwrap();
                Selected::whole(&diff[0].file)
            };
            let whole = read();
            std::fs::write(linked.join("file"), "agent, more\n").unwrap();
            let refused = discard(path, Some("agent"), &[whole]).unwrap_err();
            assert!(
                refused
                    .to_string()
                    .contains("changed since the diff was shown"),
                "what was written after the diff was read isn't thrown away unseen, as nothing \
                 brings a linked worktree's discard back: {refused}"
            );
            discard(path, Some("agent"), &[read()]).unwrap();
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

            std::fs::write(linked.join("added"), "x\n").unwrap();
            git(&linked, &["add", "added"]);
            std::fs::remove_file(linked.join("added")).unwrap();
            let source = DiffSource::Linked("agent".into());
            let diff = load_diff(path, source, None, None, None, None).unwrap();
            let added = diff.iter().find(|f| f.file.path == "added").unwrap();
            discard(path, Some("agent"), &[Selected::whole(&added.file)])
                .expect("a whole file's base, here the index's, isn't what was shown to compare");
        })
    }

    #[test]
    fn deleting_a_linked_worktree_loses_only_the_files_named() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            let (outside, linked) = agent_worktree(path);
            std::fs::write(linked.join("file"), "seen\n").unwrap();
            let repo = gix::open(&linked).unwrap();
            let (modified, size) = stamp(&repo, "file".into());
            let asked = [("file".to_owned(), modified, size)];
            // Written after the user was asked about the change above.
            std::fs::write(linked.join("fresh"), "new\n").unwrap();

            let remove = crate::backend::worktrees::remove;
            let err = remove(path, "agent", &asked).unwrap_err();
            assert!(
                format!("{err:#}").contains("fresh"),
                "it names the change nobody agreed to lose: {err:#}"
            );
            assert!(
                linked.join("fresh").exists(),
                "and leaves the worktree as it was"
            );

            std::fs::remove_file(linked.join("fresh")).unwrap();
            let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
            let file = std::fs::File::options()
                .write(true)
                .open(linked.join("file"));
            file.unwrap().set_modified(later).unwrap();
            let err = remove(path, "agent", &asked).unwrap_err();
            assert!(
                format!("{err:#}").contains("file"),
                "nor what was written to a file named since: {err:#}"
            );

            let moved = outside.path().join("moved");
            std::fs::rename(&linked, &moved).unwrap();
            let err = remove(path, "agent", &asked).unwrap_err();
            assert!(
                format!("{err:#}").starts_with("Couldn't check agent's files"),
                "a worktree it can't read, it can't check, so it refuses: {err:#}"
            );
        })
    }

    #[test]
    fn a_failed_discard_in_a_linked_worktree_leaves_its_conflicts() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            let (_outside, linked) = agent_worktree(path);
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

            let source = DiffSource::Linked("agent".into());
            let diff = load_diff(path, source, None, None, None, None).unwrap();
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
            mark_resolved(path, Some("agent"), vec!["file".into()]).unwrap();
            assert!(
                git(&linked, &["ls-files", "-u"]).is_empty(),
                "marked resolved in the worktree's own index, not the main checkout's"
            );
        })
    }

    #[test]
    fn amending_some_hunks_leaves_the_rest_uncommitted() {
        but_testsupport::isolated_app_data_dir(|| {
            let mut lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            lines[0] = "first edit\n".into();
            lines[19] = "last edit\n".into();
            std::fs::write(path.join("file"), lines.concat()).unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            assert_eq!(
                hunks.len(),
                2,
                "the edits are far enough apart for two hunks"
            );
            let head = head(path);
            let first_hunk = diff[0].part(Some(vec![hunks[0].key]));
            amend(path, head, &[first_hunk], None, false).unwrap();

            let amended = git(path, &["show", "HEAD:file"]);
            assert!(amended.starts_with("first edit\n"), "{amended}");
            assert!(amended.ends_with("line 20\n"), "{amended}");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                lines.concat(),
                "the worktree keeps the last edit, now uncommitted"
            );
        })
    }

    #[test]
    fn amending_part_of_a_file_whose_base_moved_asks_to_try_again() {
        but_testsupport::isolated_app_data_dir(|| {
            let mut lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            lines[0] = "first edit\n".into();
            lines[19] = "last edit\n".into();
            std::fs::write(path.join("file"), lines.concat()).unwrap();
            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let first_hunk = diff[0].part(Some(vec![hunks[0].key]));
            // Committed elsewhere, as by the CLI, which leaves the file as it is.
            git(path, &["commit", "-qam", "elsewhere"]);
            let head = head(path);
            let err = amend(path, head, &[first_hunk], None, false).unwrap_err();
            assert!(
                err.to_string().contains("changed since the diff was shown"),
                "its hunks are numbered against a base that's gone: {err}"
            );
        })
    }

    // Its hooks are shell scripts made executable the Unix way.
    #[cfg(unix)]
    #[test]
    fn a_failing_pre_commit_hook_refuses_the_commit_and_passing_hooks_shape_it() {
        but_testsupport::isolated_app_data_dir(|| {
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
            hook("commit-msg", "printf '\\nIssue: SHOP-12\\n' >> \"$1\"");
            hook("post-commit", "echo 'notify failed'; exit 1");
            let (_, _, hook_failed) = commit(path, &main, &changes(), None, "Two", true).unwrap();
            assert_eq!(
                git(path, &["log", "-1", "--format=%s|%b"]).trim(),
                "Two|Issue: SHOP-12",
                "the commit takes the trailer commit-msg added, below the title"
            );
            assert!(uncommitted(path).is_empty(), "the commit stands");
            assert!(
                hook_failed.is_some_and(|output| output.contains("notify failed")),
                "post-commit's failure comes back to report"
            );
        })
    }

    #[test]
    fn a_commit_waits_for_a_git_merge_or_bisect_to_finish() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let main = Destination::Branch("refs/heads/main".into());
            let first = git(path, &["rev-parse", "HEAD"]);
            for (state, says) in [("MERGE_HEAD", "git merge"), ("BISECT_LOG", "git bisect")] {
                std::fs::write(path.join(".git").join(state), format!("{first}\n")).unwrap();
                let refused = commit(path, &main, &[], None, "Two", false).unwrap_err();
                assert!(
                    format!("{refused:#}").contains(&format!("{says} isn't finished")),
                    "finishing it would go on past the commit: {refused:#}"
                );
                std::fs::remove_file(path.join(".git").join(state)).unwrap();
            }
            assert_eq!(
                git(path, &["rev-parse", "HEAD"]),
                first,
                "nothing was committed"
            );
        })
    }

    #[test]
    fn committing_no_changes_makes_an_empty_commit() {
        but_testsupport::isolated_app_data_dir(|| {
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
        })
    }

    #[test]
    fn a_new_branch_from_the_default_branch_goes_on_top_of_it() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);

            let new = create_branch(path, Some("my feature")).unwrap();
            assert_eq!(
                new, "refs/heads/my-feature",
                "named as typed, as Git allows"
            );
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                new,
                "it's checked out, as `git checkout -b` would"
            );
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert!(!workspace.managed, "still outside the workspace");
            assert_eq!(
                (workspace.stacks.iter().flat_map(|s| &s.branches))
                    .map(|b| b.ref_name.as_str())
                    .collect::<Vec<_>>(),
                [new.as_str()],
                "the branch checked out is listed as the only one"
            );
        })
    }

    #[test]
    fn a_hook_says_how_it_ended_first() {
        let output = "stdout:\n[STARTED] lint\n[FAILED] cargo fmt --check\n-a\n+b\n\n\
            stderr:\nhusky - pre-commit script failed (code 1)\n";
        assert_eq!(
            super::hook_output(output),
            format!("-a\n+b\nhusky - pre-commit script failed (code 1)\n\n{output}"),
            "its last lines, then all of it to copy"
        );
        assert_eq!(
            super::hook_output("stdout:\nlint failed\n"),
            "lint failed",
            "a short one as it is, without the stream's label"
        );
    }

    #[test]
    fn a_file_rewritten_without_a_new_write_time_is_counted_again() {
        let dir = repo_with("one\n");
        let file = dir.path().join("file");
        std::fs::write(&file, "one\ntwo\n").unwrap();
        let repo = gix::open(dir.path()).unwrap();
        let first = crate::backend::worktree_files(&repo, &[]).unwrap();
        assert_eq!(first[0].lines, Some((1, 0)), "one line added");

        // Its time set back after the write, as `cp -p` or a coarse timestamp leaves it.
        let at = std::fs::metadata(&file).unwrap().modified().unwrap();
        std::fs::write(&file, "one\ntwo\nthree\n").unwrap();
        (std::fs::File::options().write(true).open(&file).unwrap())
            .set_modified(at)
            .unwrap();
        let second = crate::backend::worktree_files(&repo, &first).unwrap();
        assert_eq!(
            second[0].lines,
            Some((2, 0)),
            "its size moved, so the lines are counted again rather than kept"
        );
        let err = super::refuse_changed(&repo, &[Selected::whole(&first[0])], true).unwrap_err();
        assert!(
            err.to_string().contains("changed since the diff was shown"),
            "nor is it taken for the file the user saw: {err}"
        );
    }

    #[test]
    fn a_target_without_a_remote_is_its_own_branch_to_update_from_and_land_on() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            crate::backend::settings::set_target(path, "refs/heads/main").unwrap();
            git(path, &["checkout", "-q", "-b", "feature"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Feature"]);
            // A commit `main` gains while it isn't checked out, as from a `git merge` into it.
            let gained = git(
                path,
                &["commit-tree", "main^{tree}", "-p", "main", "-m", "On main"],
            );
            git(path, &["update-ref", "refs/heads/main", gained.trim()]);

            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            let target = workspace.target.as_ref().expect("main is the target");
            assert_eq!(
                (target.name.as_str(), target.local, target.needs_update),
                ("main", true, true),
                "the local branch is the target, with something new to update from"
            );
            assert_eq!(
                (target.incoming.iter())
                    .map(|c| c.title.as_str())
                    .collect::<Vec<_>>(),
                ["On main"],
                "what main gained is listed as new on it"
            );

            let tip = workspace.find_branch("refs/heads/feature").unwrap().commits[0].id;
            crate::backend::upstream::land(path, "feature", tip, false).unwrap();
            assert!(
                git(path, &["log", "--format=%s", "main"])
                    .lines()
                    .any(|s| s == "Feature"),
                "main moved to take the branch in, with no remote to push to"
            );
        })
    }

    #[test]
    fn a_new_branch_from_a_detached_head_is_refused() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "--detach"]);

            let before = git(path, &["branch", "--list"]);
            let err = create_branch(path, None).unwrap_err();
            assert!(err.to_string().contains("detached"), "{err}");
            assert_eq!(git(path, &["branch", "--list"]), before, "nothing is made");
        })
    }

    #[test]
    fn a_new_branch_beside_a_checked_out_one_starts_the_workspace_and_undoes() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            git(
                path,
                &["commit", "-q", "--allow-empty", "-m", "feature work"],
            );
            std::fs::write(path.join("file"), "two\n").unwrap();
            let lanes = |path: &Path| {
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                let mut lanes: Vec<Vec<String>> = (workspace.stacks.iter())
                    .map(|s| s.branches.iter().map(|b| b.name.clone()).collect())
                    .collect();
                lanes.sort();
                (workspace.managed, lanes)
            };
            let head = |path: &Path| git(path, &["symbolic-ref", "HEAD"]).trim().to_owned();

            // A teammate's branch on the remote, which a branch of that name would take as
            // its own.
            git(path, &["update-ref", "refs/remotes/origin/fix", "HEAD"]);
            let taken = create_branch(path, Some("fix"));
            assert!(
                taken.is_err() && !lanes(path).0,
                "fix is taken, so nothing changes"
            );
            let new = create_branch(path, Some("second")).unwrap();
            let second = new.strip_prefix("refs/heads/").unwrap();
            let mut both = vec![vec!["feature".to_owned()], vec![second.to_owned()]];
            both.sort();
            assert_eq!(
                lanes(path),
                (true, both),
                "two branches side by side, so GitButler's workspace holds them"
            );
            assert_eq!(head(path), "refs/heads/gitbutler/workspace");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "two\n",
                "the uncommitted change comes along"
            );

            let last = crate::backend::history::oplog_head(path).unwrap();
            crate::backend::history::undo_snapshot(path, last, None).unwrap();
            assert_eq!(
                head(path),
                "refs/heads/feature",
                "one undo goes back to the branch on its own"
            );
            assert!(
                git(path, &["branch", "--list", second]).trim().is_empty(),
                "and takes the new branch away"
            );
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "two\n",
                "keeping the uncommitted change"
            );
        })
    }

    #[test]
    fn a_listed_branch_names_everyone_with_a_commit_of_its_own() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            for (who, title) in [("Ada", "first"), ("Bo", "second"), ("Ada", "third")] {
                let name = format!("user.name={who}");
                git(
                    path,
                    &["-c", &name, "commit", "-q", "--allow-empty", "-m", title],
                );
            }
            git(path, &["checkout", "-q", "main"]);

            let listed = crate::backend::branches::all_branches(path).unwrap();
            let feature = listed.iter().find(|b| b.name == "feature").unwrap();
            assert_eq!(
                feature.people,
                ["Ada", "Bo"],
                "who its own commits are by, newest first, each once"
            );
        })
    }

    #[test]
    fn a_new_branch_beside_starts_where_the_checked_out_one_leaves_the_target() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            git(path, &["checkout", "-q", "-b", "feature"]);
            std::fs::write(path.join("file"), "feature\n").unwrap();
            git(path, &["commit", "-q", "-am", "feature work"]);
            // The target moved on since, changing the same line.
            git(path, &["checkout", "-q", "main"]);
            std::fs::write(path.join("file"), "target\n").unwrap();
            git(path, &["commit", "-q", "-am", "target work"]);
            target_origin_main(path);
            git(path, &["checkout", "-q", "feature"]);
            let fork = git(path, &["merge-base", "feature", "main"]);

            let new = create_branch(path, None).unwrap();
            assert_eq!(
                git(path, &["rev-parse", &new]),
                fork,
                "where feature leaves the target, so the two merge without conflicts"
            );
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/gitbutler/workspace"
            );
        })
    }

    #[test]
    fn commits_made_on_the_default_branch_stay_below_the_next_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "on main"]);
            let branches = |path: &Path| {
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                assert_eq!(
                    workspace.target_branch.as_deref(),
                    Some("refs/heads/main"),
                    "the composer knows which branch not to commit to"
                );
                (workspace.stacks.iter().flat_map(|s| &s.branches))
                    .map(|b| (b.name.clone(), b.commits.len()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                branches(path),
                [("main".to_owned(), 1)],
                "main is listed while it has a commit origin doesn't"
            );

            let new = create_branch(path, None).unwrap();
            let name = new.strip_prefix("refs/heads/").unwrap().to_owned();
            assert_eq!(
                branches(path),
                [(name, 0), ("main".to_owned(), 1)],
                "the composer's new branch goes on top of main's commit, which stays where it is"
            );
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                new,
                "the new branch is checked out, so the commit lands on it"
            );
        })
    }

    #[test]
    fn a_detached_unborn_or_untargeted_checkout_is_unrecorded() {
        but_testsupport::isolated_app_data_dir(|| {
            let load = |path: &Path| crate::backend::load_workspace(path, Default::default());
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            assert!(
                !load(path).unwrap().unrecorded,
                "on a branch, undo can go back"
            );
            git(path, &["checkout", "-q", "--detach"]);
            assert!(
                load(path).unwrap().unrecorded,
                "the oplog won't restore a detached HEAD"
            );

            let unborn = tempfile::tempdir().unwrap();
            git(unborn.path(), &["init", "-q"]);
            assert!(
                load(unborn.path()).unwrap().unrecorded,
                "with no commits, there's no target to record a snapshot against"
            );

            let untargeted = repo_with("one\n");
            assert!(
                load(untargeted.path()).unwrap().unrecorded,
                "the oplog records a snapshot against the target, so none is recorded without one"
            );
        })
    }

    #[test]
    fn history_doesnt_say_the_checked_out_target_was_removed() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            crate::backend::history::take_snapshot(path, Some("Before".into())).unwrap();

            use crate::backend::history;
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            let now = history::branches(&workspace);
            assert!(
                now.is_empty(),
                "the target with nothing of its own isn't listed"
            );
            let (operations, _) = history::list(path, None, 10, Some(now)).unwrap();
            assert!(
                operations[0].changes.is_empty(),
                "the newest snapshot, compared with now, lists main as the workspace does: not at \
                 all. {:?}",
                operations[0].changes
            );
        })
    }

    #[test]
    fn history_shows_commits_made_outside_the_workspace() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            let commit =
                |message: &str| git(path, &["commit", "-q", "--allow-empty", "-m", message]);
            commit("Lower");
            // A branch stacked under the checked-out one.
            git(path, &["branch", "lower"]);
            commit("First");
            crate::backend::history::take_snapshot(path, None).unwrap();
            commit("Second");
            crate::backend::history::take_snapshot(path, None).unwrap();
            commit("Third");

            use crate::backend::history::{self, Change};
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert!(!workspace.managed, "a branch checked out by itself");
            let now = history::branches(&workspace);
            let (operations, _) = history::list(path, None, 10, Some(now)).unwrap();
            let lines: Vec<Vec<String>> = (operations.iter())
                .map(|operation| {
                    (operation.changes.iter())
                        .map(|change| match change {
                            Change::Added { commit, branch } => {
                                format!("+ {} on {branch}", commit.title())
                            }
                            other => format!("{other:?}"),
                        })
                        .collect()
                })
                .collect();
            assert_eq!(
                lines,
                [["+ Third on feature"], ["+ Second on feature"]],
                "each snapshot records the branches' commits as the workspace shows them"
            );
        })
    }

    #[test]
    fn a_reload_of_the_commits_keeps_the_files_while_their_tree_stays() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let top = head(path);
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
        })
    }

    #[test]
    fn undoing_a_move_to_a_new_branch_takes_the_branch_away_too() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let (dir, commit, ..) = workspace_with_an_edit();
            let path = dir.path();
            let (_, name) = move_to_new_branch(path, &[commit]).unwrap();
            let moved = oplog_head(path).unwrap();
            assert!(matches!(
                undo_snapshot(path, moved, None).unwrap(),
                Undone::Done(_)
            ));
            let left = git(path, &["branch", "--list", short_ref(&name)]);
            assert_eq!(
                left, "",
                "{name} would be listed beside the commit back in place"
            );
        })
    }

    #[test]
    fn undo_and_redo_stop_for_what_came_after_and_for_files_written_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{
                Undone, oplog_head, redo_snapshot, take_snapshot, undo_snapshot,
            };
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let top = head(path);
            name_branch(path, top, "named").unwrap();
            let named = oplog_head(path).unwrap();
            let done = now();
            // Later than it, as an edit in an editor is.
            std::thread::sleep(std::time::Duration::from_millis(5));
            std::fs::write(path.join("file"), "two\n").unwrap();
            assert!(
                matches!(undo_snapshot(path, named, Some((done, &[]))).unwrap(),
                    Undone::Newer(files) if files == ["file"]),
                "a file written after the operation is one undoing it puts back"
            );

            take_snapshot(path, None).unwrap();
            let snapshot = oplog_head(path).unwrap();
            assert!(
                matches!(undo_snapshot(path, named, None).unwrap(), Undone::Behind(_)),
                "the snapshot came after it, and undoing it would take that back too"
            );
            assert!(
                matches!(
                    undo_snapshot(path, snapshot, Some((done, &[(done, now())]))).unwrap(),
                    Undone::Done(_)
                ),
                "written while the app's own undo ran, the file isn't the user's change"
            );
            assert!(
                matches!(
                    undo_snapshot(path, snapshot, None).unwrap(),
                    Undone::Already(_)
                ),
                "undone once, as by another client, it isn't taken for behind what came before"
            );
            assert!(
                matches!(undo_snapshot(path, named, None).unwrap(), Undone::Done(_)),
                "what came before it is still there to undo"
            );

            let undo = oplog_head(path).unwrap();
            std::fs::write(path.join("file"), "three\n").unwrap();
            assert!(
                matches!(redo_snapshot(path, undo, &Default::default(), Some(&[])).unwrap(),
                    Some(Undone::Newer(files)) if files == ["file"]),
                "a file written after the undo is one redoing it puts back"
            );
        })
    }

    #[test]
    fn undo_and_redo_stop_for_a_commit_made_outside_the_app_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, redo_snapshot, undo_snapshot};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            name_branch(path, head(path), "named").unwrap();
            let named = oplog_head(path).unwrap();
            let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
            let done = since.unwrap().as_millis() as i64;
            // In a terminal, `minutes` on.
            let commit_later = |minutes: i64, title: &str| {
                let later = format!("{} +0000", done / 1000 + minutes * 60);
                let committed = std::process::Command::new("git")
                    .current_dir(path)
                    .env("GIT_COMMITTER_DATE", later)
                    .args(["commit", "-q", "--allow-empty", "-m", title])
                    .status()
                    .unwrap();
                assert!(committed.success(), "git commits");
            };
            commit_later(1, "terminal work");
            assert!(
                matches!(undo_snapshot(path, named, Some((done, &[]))).unwrap(),
                    Undone::Behind(why) if why.contains("terminal work")),
                "undoing would take the commit out of every branch, so it says what came after"
            );

            assert!(
                matches!(undo_snapshot(path, named, None).unwrap(), Undone::Done(_)),
                "undone anyway"
            );
            let undo = oplog_head(path).unwrap();
            commit_later(2, "more terminal work");
            assert!(
                matches!(
                    redo_snapshot(path, undo, &Default::default(), Some(&[])).unwrap(),
                    Some(Undone::Behind(_))
                ),
                "redoing would take that one out too"
            );
        })
    }

    #[test]
    fn a_branch_that_moved_since_it_was_asked_about_isnt_deleted() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["branch", "side"]);
            let asked = head(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "an agent's"]);
            git(path, &["branch", "-f", "side", "HEAD"]);

            let refused = remove_branch(path, "refs/heads/side", Some(asked)).unwrap_err();
            assert!(
                format!("{refused:#}").contains("changed since you asked"),
                "it says why it stays: {refused:#}"
            );
            assert!(
                !git(path, &["branch", "--list", "side"]).is_empty(),
                "the branch and its new commit stay"
            );
        })
    }

    #[test]
    fn naming_an_unnamed_branch_makes_it_a_stack_and_undoes() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            // A commit made with plain Git on the workspace branch belongs to no branch.
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let top = head(path);
            let branch_at_top = |path: &Path| {
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                (workspace.branches())
                    .find(|b| b.commits.first().map(|c| c.id) == Some(top))
                    .map(|b| b.ref_name.clone())
            };
            assert_eq!(branch_at_top(path).as_deref(), Some(""), "it shows unnamed");

            assert_eq!(name_branch(path, top, "named").unwrap(), "refs/heads/named");
            assert_eq!(branch_at_top(path).as_deref(), Some("refs/heads/named"));

            let last = crate::backend::history::oplog_head(path).unwrap();
            crate::backend::history::undo_snapshot(path, last, None).unwrap();
            assert_eq!(
                branch_at_top(path).as_deref(),
                Some(""),
                "undo takes the name away"
            );
        })
    }

    #[test]
    fn a_hunk_discard_queued_behind_another_in_its_file_asks_to_try_again() {
        but_testsupport::isolated_app_data_dir(|| {
            let lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            let mut edited = lines.clone();
            edited[1] = "two\nmore\nlines\n".into();
            edited[18] = "last edit\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            assert_eq!(
                hunks.len(),
                2,
                "the edits are far enough apart for two hunks"
            );
            let hunk = |ix: usize| diff[0].part(Some(vec![hunks[ix].key]));
            discard(path, None, &[hunk(0)]).unwrap();
            // Read before the first discard, which took two lines out above it.
            let err = discard(path, None, &[hunk(1)]).unwrap_err();
            assert!(
                err.to_string().contains("changed since the diff was shown"),
                "its place in the file is from before the first discard: {err}"
            );
            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let fresh = diff[0].part(Some(vec![hunks[0].key]));
            discard(path, None, &[fresh]).unwrap();
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                lines.concat(),
                "tried again on the fresh diff, each discard took back just its own hunk"
            );
        })
    }

    #[test]
    fn a_hunk_discard_after_its_files_base_moved_asks_to_try_again() {
        but_testsupport::isolated_app_data_dir(|| {
            let lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            let mut edited = lines.clone();
            edited[1] = "two\n".into();
            edited[18] = "nineteen\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();
            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let second = diff[0].part(Some(vec![hunks[1].key]));
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
        })
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
        but_testsupport::isolated_app_data_dir(|| {
            let lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            let mut edited = lines.clone();
            edited[4] = "five a\nfive b\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunk = &diff[0].hunks()[0];
            let picks = std::collections::BTreeSet::from([line_in(hunk, "five b")]);
            let lines_picked = diff[0].part(Some(hunk.picked(&picks)));
            discard(path, None, &[lines_picked]).unwrap();

            let mut expected = lines.clone();
            expected[4] = "five a\n".into();
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                expected.concat(),
                "only the picked line went; the rest of its hunk stays uncommitted"
            );
        })
    }

    /// A workspace whose one commit, on top of the target, turns line 5 of `file` into two,
    /// with that commit's id and diff.
    fn workspace_with_an_edit() -> (tempfile::TempDir, CommitId, Vec<FileDiff>, Vec<String>) {
        let lines: Vec<String> = twenty_lines();
        let dir = repo_with(&lines.concat());
        let path = dir.path();
        target_origin_main(path);
        git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
        let mut edited = lines.clone();
        edited[4] = "five a\nfive b\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        git(path, &["commit", "-q", "-am", "edit"]);
        let commit = head(path);
        let diff = load_diff(path, DiffSource::Commit(commit), None, None, None, None).unwrap();
        (dir, commit, diff, edited)
    }

    #[test]
    fn a_pull_whose_incoming_commit_conflicts_is_caught_before_it_runs() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            target_origin_main(path);
            let commit = |content: &str, message: &str| {
                std::fs::write(path.join("file"), content).unwrap();
                git(path, &["commit", "-q", "-am", message]);
            };
            git(path, &["checkout", "-q", "-b", "feature"]);
            commit("one\n", "One");
            commit("two\n", "Two");
            git(path, &["update-ref", "refs/remotes/origin/feature", "HEAD"]);
            git(path, &["config", "branch.feature.remote", "origin"]);
            git(
                path,
                &["config", "branch.feature.merge", "refs/heads/feature"],
            );
            // "One" rewritten here, so the remote's "Two" no longer applies on top of it.
            git(path, &["reset", "-q", "--hard", "main"]);
            commit("uno\n", "One");

            let found = crate::backend::pull_conflicts(path, "refs/heads/feature").unwrap();
            assert_eq!(
                found,
                [("refs/heads/feature".to_owned(), 1)],
                "the remote's commit would land conflicted"
            );
        })
    }

    #[test]
    fn a_conflicted_deletion_lists_its_file_though_its_tree_changes_nothing() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("base\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            std::fs::write(path.join("file"), "edited\n").unwrap();
            git(path, &["commit", "-q", "-am", "edit"]);
            let edit = head(path);
            git(path, &["rm", "-q", "file"]);
            git(path, &["commit", "-q", "-m", "delete"]);
            let delete = head(path);
            let rewritten = move_commits(path, &[delete], &Destination::BelowCommit(edit)).unwrap();
            let delete = rewritten[&delete];

            let diff = load_diff(path, DiffSource::Commit(delete), None, None, None, None).unwrap();
            // Below the edit, the deletion conflicts with the file it was to delete and keeps it,
            // so its tree changes nothing.
            assert_eq!(diff.len(), 1, "the conflict still lists the file: {diff:?}");
            assert!(diff[0].file.conflicted, "{:?}", diff[0].file);
            assert!(
                matches!(diff[0].body, DiffBody::Unresolvable(_)),
                "with why it can't be picked: {:?}",
                diff[0].body
            );
        })
    }

    /// The line "five b" of `diff`'s one hunk, picked to take out of its commit.
    fn five_b_out(diff: &[FileDiff]) -> Selected {
        let hunk = &diff[0].hunks()[0];
        let picks = std::collections::BTreeSet::from([line_in(hunk, "five b")]);
        diff[0].part(Some(hunk.picked_out(&picks)))
    }

    #[test]
    fn uncommitting_picked_lines_leaves_them_uncommitted() {
        but_testsupport::isolated_app_data_dir(|| {
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
        })
    }

    #[test]
    fn splitting_off_picked_lines_puts_them_in_a_commit_above() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, commit, diff, _) = workspace_with_an_edit();
            let path = dir.path();
            let (rewritten, new) = split_off(path, commit, &[five_b_out(&diff)]).unwrap();
            let commit = rewritten.get(&commit).copied().unwrap_or(commit);
            let below = git(path, &["show", &format!("{}:file", commit.0)]);
            let above = git(path, &["show", &format!("{}:file", new.0)]);
            assert!(!below.contains("five b"), "{below}");
            assert!(above.contains("five a\nfive b\n"), "{above}");
        })
    }

    #[test]
    fn discarding_picked_lines_from_a_commit_takes_them_out_of_it_and_the_worktree() {
        but_testsupport::isolated_app_data_dir(|| {
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
        })
    }

    #[test]
    fn moving_picked_lines_takes_them_into_the_other_commit() {
        but_testsupport::isolated_app_data_dir(|| {
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
        })
    }

    #[test]
    fn amending_picked_lines_commits_just_those() {
        but_testsupport::isolated_app_data_dir(|| {
            let lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            let mut edited = lines.clone();
            edited[4] = "five a\nfive b\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunk = &diff[0].hunks()[0];
            let picks = std::collections::BTreeSet::from([line_in(hunk, "five a")]);
            let picked = diff[0].part(Some(hunk.picked(&picks)));
            let head = head(path);
            amend(path, head, &[picked], None, false).unwrap();

            let amended = git(path, &["show", "HEAD:file"]);
            // Just the added line: line 5, whose removal wasn't picked, stays below it.
            assert!(amended.contains("line 4\nfive a\nline 5\n"), "{amended}");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                edited.concat(),
                "the rest stays uncommitted on disk"
            );
        })
    }

    #[test]
    fn a_new_branch_checked_out_in_a_linked_worktree_moves_it() {
        let dir = repo_with("base\n");
        let path = dir.path();
        let (_outside, linked) = agent_worktree(path);
        let before = crate::backend::worktrees::heads(path);
        git(&linked, &["checkout", "-q", "-b", "agent-next"]);
        assert_ne!(
            crate::backend::worktrees::heads(path),
            before,
            "same commit, but the oplog can't put the branch it has checked out back"
        );
    }
}
