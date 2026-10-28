//! Mutations of commits, branches and uncommitted changes. Each opens the project, calls
//! the matching `but-api` function and returns; the UI reloads the workspace afterwards.
//! Undo history comes from `but-api`'s own oplog snapshots, except that [`discard`],
//! [`split_off`], [`move_to_new_branch`] and [`create_beside_checkout`] record their own, as
//! does [`super::conflicts::resolve_markers`] through [`record_undo`].

use gitbutler_repo::hooks::{ErrorData, HookResult, MessageHookResult};
use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, ByteSlice as _};
use but_api::commit::json::ChangesSource;
use but_core::tree::create_tree::RejectionReason;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

use super::{ChangeKind, CommitId, FullName, Selected, checkout, open, short_ref, stamp};

/// Where a commit or a set of changes lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The top of this branch.
    Branch(super::FullName),
    /// Right above this commit, in its branch.
    AboveCommit(CommitId),
    /// Right below this commit, in its branch.
    BelowCommit(CommitId),
}

impl Destination {
    fn relative_to(&self) -> Result<(RelativeTo, InsertSide)> {
        Ok(match self {
            Destination::Branch(ref_name) => {
                (RelativeTo::Reference(ref_name.clone()), InsertSide::Below)
            }
            Destination::AboveCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Above),
            Destination::BelowCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Below),
        })
    }
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
fn unmerged<'a>(repo: &gix::Repository, selected: &'a [Selected]) -> Result<Vec<&'a BStr>> {
    let index = repo.index_or_empty()?;
    Ok((selected.iter())
        .map(|part| part.file.change.path.as_bstr())
        .filter(|path| {
            (index.entry_index_by_path(path).ok())
                .and_then(|ix| index.entries().get(ix))
                .is_some_and(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
        })
        .collect())
}

/// Refuse conflicted files of the main checkout, or of the linked `worktree`, which would go
/// in markers and all.
fn refuse_conflicted(
    ctx: &but_ctx::Context,
    worktree: Option<&bstr::BStr>,
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
        let change = &part.file.change;
        let path = (change.status.previous_state_and_path())
            .and_then(|(_, previous)| previous)
            .unwrap_or(change.path.as_bstr());
        let entry = head
            .as_ref()?
            .lookup_entry(path.split(|b| *b == b'/'))
            .ok()??;
        Some(entry.object_id())
    };
    // A whole file goes back to its base whatever that is, so only being written to counts.
    let changed = (selected.iter()).find(|part| {
        let picked = part.hunks.is_some();
        (whole || picked)
            && (stamp(repo, part.file.change.path.as_bstr())
                != (part.file.modified, part.file.size)
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
pub(super) fn changes_source(worktree: Option<&bstr::BStr>) -> ChangesSource {
    worktree.map_or(ChangesSource::Head, |name| {
        ChangesSource::Worktree(name.to_string())
    })
}

/// [`selected_specs`] to take out of a commit or the worktree, refusing some of a file that
/// can only be taken whole, as the rest would go along.
fn taken_specs(selected: &[Selected]) -> Result<Vec<DiffSpec>> {
    refuse_converted(selected)?;
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

/// Refuse taking part of a file whose diff a `textconv` filter made, as for an encrypted or
/// binary file: its hunks are lines of that readable copy, and applying one would splice them
/// into the file.
fn refuse_converted(selected: &[Selected]) -> Result<()> {
    match (selected.iter()).find(|part| part.file.converted && part.hunks.is_some()) {
        Some(part) => bail!(
            "Only all of {} can be taken, as its diff is a copy a textconv filter made for \
             reading, not its own lines",
            part.file.path
        ),
        None => Ok(()),
    }
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
    // By reason, the first few of each named: a change to thousands of files, as an agent's
    // move of a folder, can have them all left out for one.
    let mut by_reason: Vec<(&str, Vec<String>)> = Vec::new();
    for (reason, spec) in rejected {
        let (why, path) = (why(reason), spec.path.to_string());
        match by_reason.iter_mut().find(|(said, _)| *said == why) {
            Some((_, paths)) => paths.push(path),
            None => by_reason.push((why, vec![path])),
        }
    }
    (by_reason.into_iter())
        .map(|(why, paths)| match paths.split_at_checked(3) {
            Some((named, rest)) if !rest.is_empty() => {
                let more = crate::ui::plural(rest.len(), "more file");
                format!("{} and {more}: {why}", named.join(", "))
            }
            _ => format!("{}: {why}", paths.join(", ")),
        })
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
    let name = format!("refs/heads/{name}");
    FullName::try_from(name.as_str()).with_context(|| format!("Invalid ref {name}"))
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
pub(super) fn refuse_unfinished(repo: &gix::Repository, on_branch: bool) -> Result<()> {
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

/// GitButler's branches build on a commit; core's own error for a repository without one
/// names its internals.
fn refuse_unborn(ctx: &but_ctx::Context) -> Result<()> {
    if ctx.repo.get()?.head()?.is_unborn() {
        bail!("The repository has no commits yet. Make the first one, then try again.");
    }
    Ok(())
}

/// Make the repository's first commit, of `changes`, on the branch `HEAD` names, which
/// `destination` must be, as `git commit` does in a new repository. The caller holds
/// exclusive worktree access.
fn commit_first(
    ctx: &mut but_ctx::Context,
    destination: &Destination,
    changes: Vec<DiffSpec>,
    message: String,
) -> Result<but_workspace::commit_engine::CreateCommitOutcome> {
    let repo = ctx.repo.get()?;
    let branch = repo.head_name()?.context("HEAD names no branch")?;
    if !matches!(destination, Destination::Branch(name) if branch == *name) {
        bail!(
            "The first commit goes on {}, the branch checked out.",
            branch.shorten()
        );
    }
    let outcome = but_workspace::commit_engine::create_commit(
        &repo,
        but_workspace::commit_engine::Destination::NewCommit {
            parent_commit_id: None,
            stack_segment: None,
            message,
        },
        changes,
        ctx.settings.context_lines,
    )?;
    if let Some(id) = outcome.new_commit {
        but_workspace::init::publish_first_commit(&repo, branch.as_ref(), id)?;
    }
    drop(repo);
    ctx.invalidate_workspace_cache()?;
    Ok(outcome)
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
    worktree: Option<&bstr::BStr>,
    message: &str,
    run_hooks: bool,
) -> Result<(CommitId, LeftOut, Option<String>)> {
    let mut ctx = open(path)?;
    let unborn = ctx.repo.get()?.head()?.is_unborn();
    refuse_unfinished(&checkout(&ctx, worktree)?, worktree.is_none())?;
    refuse_conflicted(&ctx, worktree, selected)?;
    refuse_converted(selected)?;
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
    let (new_commit, rejected) = {
        let mut guard = ctx.exclusive_worktree_access();
        if unborn {
            let outcome = commit_first(&mut ctx, destination, changes, message)?;
            (outcome.new_commit, outcome.rejected_specs)
        } else {
            let (relative_to, side) = destination.relative_to()?;
            let outcome = but_api::commit::create::commit_create(
                &mut ctx,
                relative_to,
                side,
                changes,
                changes_source(worktree),
                message,
                DryRun::No,
                guard.write_permission(),
            )?;
            (outcome.new_commit, outcome.rejected_specs)
        }
    };
    let left_out = left_out(&rejected);
    let Some(id) = new_commit else {
        bail!("Nothing could be committed.\n{}", left_out.join("\n"));
    };
    let hook_failed = run_hooks.then(|| post_commit(&ctx)).flatten();
    Ok((CommitId(id), left_out, hook_failed))
}

/// Create a new stack, named `name` or with a made-up name, and return its full ref name.
/// With the default branch checked out on its own, the new branch goes on top of it and is
/// checked out, as `git checkout -b` would. Beside another branch, both go in GitButler's
/// workspace, as `but branch new` does.
pub fn create_branch(path: &Path, name: Option<&str>) -> Result<FullName> {
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
    Ok(created.new_ref)
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
) -> Result<FullName> {
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
    Ok(new_ref)
}

/// Stack a new branch with a made-up name on top of `below` (a full ref name).
pub fn create_dependent_branch(path: &Path, below: &FullName) -> Result<FullName> {
    let relative_to = but_api::commit::json::RelativeTo::Reference(below.clone());
    create_branch_at(path, relative_to, InsertSide::Above)
}

/// Make a new branch, with a made-up name, whose newest commit is `commit`, taking it and the
/// commits below it in its branch; or with `below`, one that ends just under `commit`.
pub fn create_branch_at_commit(path: &Path, commit: CommitId, below: bool) -> Result<FullName> {
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
) -> Result<FullName> {
    let mut ctx = open(path)?;
    let created = but_api::branch::branch_create(
        &mut ctx,
        None,
        but_api::branch::json::BranchCreatePlacement::Dependent { relative_to, side },
    )?;
    Ok(created.new_ref)
}

/// Stack `branch` (a full ref name) on top of `onto`, with the branches above it.
pub fn stack_branch(
    path: &Path,
    branch: &FullName,
    onto: &FullName,
) -> Result<Vec<(Option<FullName>, usize)>> {
    move_branch_with(path, |ctx| {
        but_api::branch::move_branch(
            ctx,
            branch.as_ref(),
            RelativeTo::Reference(onto.clone()),
            InsertSide::Above,
            DryRun::No,
        )
    })
}

/// Move `branch` right below `commit`, the last commit of the branch it then sits under.
/// Below that branch's reference instead, `branch` would take over its commits.
pub fn move_branch_below(
    path: &Path,
    branch: &FullName,
    commit: CommitId,
) -> Result<Vec<(Option<FullName>, usize)>> {
    move_branch_with(path, |ctx| {
        but_api::branch::move_branch(
            ctx,
            branch.as_ref(),
            RelativeTo::Commit(commit.0),
            InsertSide::Below,
            DryRun::No,
        )
    })
}

/// Take `branch` out of its stack into a stack of its own.
pub fn tear_off_branch(path: &Path, branch: &FullName) -> Result<Vec<(Option<FullName>, usize)>> {
    move_branch_with(path, |ctx| {
        but_api::branch::tear_off_branch(ctx, branch.as_ref(), DryRun::No)
    })
}

/// Move a branch with `op`, saying which branches (full refs, `None` for an unnamed one) it
/// left with conflicted commits they didn't have, each with how many: a branch's commits
/// rebased onto others' can conflict.
fn move_branch_with(
    path: &Path,
    op: impl FnOnce(&mut but_ctx::Context) -> Result<but_api::branch::MoveBranchResult>,
) -> Result<Vec<(Option<FullName>, usize)>> {
    let mut ctx = open(path)?;
    let before = but_api::legacy::workspace::head_info(&ctx)?;
    let moved = op(&mut ctx)?;
    Ok(super::upstream::newly_conflicted(
        &before,
        &moved.workspace.conflicted_commits,
    ))
}

/// Take the stack holding `branch` out of the workspace. Its branches stay in the repository.
pub fn unapply_stack(path: &Path, branch: &FullName) -> Result<()> {
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

pub(super) fn holds(segment: &but_workspace::ref_info::Segment, branch: &FullName) -> bool {
    segment.ref_name() == Some(branch.as_ref())
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
pub fn rename_branch(path: &Path, ref_name: &FullName, new_name: &str) -> Result<FullName> {
    let mut ctx = open(path)?;
    let from = ref_name;
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
    Ok(outcome.new_ref)
}

/// Name the main checkout's unnamed branch whose newest commit is `top`, putting a `HEAD`
/// detached there on it, and return its full ref name.
pub fn name_branch(path: &Path, top: CommitId, name: &str) -> Result<FullName> {
    let mut ctx = open(path)?;
    let new_ref = branch_ref(name)?;
    let outcome =
        but_api::branch::branch_name_segment(&mut ctx, ChangesSource::Head, top.0, Some(new_ref))?;
    Ok(outcome.new_ref)
}

pub fn remove_branch(path: &Path, ref_name: &FullName, tip: Option<CommitId>) -> Result<()> {
    let mut ctx = open(path)?;
    let name = ref_name;
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

/// Delete `ref_name` with its commits, in one step that one undo takes back, as `but branch
/// delete` does: for the only named branch of its stack, as [`remove_branch`] keeps the
/// commits, which need another branch to hold them. A workspace left with one stack checks
/// that out, as unapplying does. One whose newest commit is no longer `tip` stays: its new
/// commits would go too.
pub fn delete_branch_and_commits(path: &Path, ref_name: &FullName, tip: CommitId) -> Result<()> {
    use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
    let mut ctx = open(path)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let perm = guard.write_permission();
    let commits: Vec<gix::ObjectId> = {
        let (_, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        // Checked out on its own, HEAD would be left on nothing.
        if !ws.kind.has_managed_ref() {
            bail!("{} is checked out on its own", short_ref(ref_name));
        }
        let segment = (ws.try_find_segment_by_refname(ref_name.as_ref()))
            .with_context(|| format!("{} isn't applied any more", short_ref(ref_name)))?;
        segment.commits.iter().map(|commit| commit.id).collect()
    };
    if commits.first() != Some(&tip.0) {
        bail!(
            "{} changed since you asked, so it wasn't deleted",
            short_ref(ref_name)
        );
    }
    // It names the branch, so undo brings it back.
    let snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        &ctx,
        SnapshotDetails::new(OperationKind::DeleteBranch)
            .with_trailers([Trailer::Name(ref_name.to_string())]),
        &[ref_name.as_ref()],
        perm.read_permission(),
        DryRun::No,
    );
    let (rolled_back, _) = but_transaction::with_transaction_with_perm_only(
        &mut ctx,
        &mut meta,
        perm,
        DryRun::No,
        |mut tx| {
            tx.discard_commits(commits)?;
            tx.remove_reference(ref_name.as_ref())?;
            Ok(but_transaction::Commit(()))
        },
    )?;
    if !rolled_back && let Some(Err(err)) = snapshot.map(|snapshot| snapshot.commit(&ctx, perm)) {
        tracing::warn!(?err, "couldn't record the deleted branch for undo");
    }
    let (mut repo, ws, _) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    // The transaction leaves Git's `branch.<name>` section, as its upstream and description,
    // which a branch made later under its name would take on.
    but_workspace::branch::remove_reference::delete_local_branch(&mut repo, ref_name.as_ref())?;
    but_workspace::branch::dissolve_unnecessary_workspace(&ws, &repo, &meta, Default::default())?;
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
    worktree: Option<&bstr::BStr>,
    run_hooks: bool,
) -> Result<(Rewritten, LeftOut, Option<String>)> {
    let mut ctx = open(path)?;
    refuse_unfinished(&checkout(&ctx, worktree)?, worktree.is_none())?;
    refuse_conflicted(&ctx, worktree, selected)?;
    refuse_converted(selected)?;
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
    refuse_buried_merges(&*ctx.repo.get()?, commits)?;
    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        commits.iter().map(|id| id.0).collect(),
        None,
        DryRun::No,
    )?;
    Ok(())
}

/// Refuse to take a merge commit out from under the commits above it, as an agent's
/// `git merge` of the target leaves one: the engine has the commit above merge what it merged
/// instead, so its changes stay and only the history changes. One at the top comes out.
fn refuse_buried_merges(repo: &gix::Repository, commits: &[CommitId]) -> Result<()> {
    let merges: Vec<&CommitId> = (commits.iter())
        .filter(|id| (repo.find_commit(id.0)).is_ok_and(|c| c.parent_ids().count() > 1))
        .collect();
    if merges.is_empty() {
        return Ok(());
    }
    // What each checkout has on top: the workspace's stacks, its commit's parents, or HEAD, and
    // each linked worktree's HEAD. Not other branches, as an agent's backup of one.
    let head = repo.head_id()?;
    let mut tips: Vec<gix::ObjectId> = match but_workspace::WorkspaceCommit::from_id(head) {
        Ok(commit) if commit.is_managed() => commit.inner.parents.to_vec(),
        _ => vec![head.detach()],
    };
    tips.extend(
        (repo.worktrees()?.into_iter())
            .filter_map(|wt| Some(wt.into_repo().ok()?.head_id().ok()?.detach())),
    );
    // Above it on the way down from a tip, unless all of those go too.
    let stays_above = |tip: gix::ObjectId, merge: gix::ObjectId| -> Result<bool> {
        let mut at = tip;
        while at != merge {
            if !commits.contains(&CommitId(at)) {
                return Ok(true);
            }
            let Some(parent) = repo.find_commit(at)?.parent_ids().next() else {
                return Ok(false);
            };
            at = parent.detach();
        }
        Ok(false)
    };
    for merge in merges {
        let buried = (tips.iter().filter(|tip| **tip != merge.0))
            .filter(|tip| super::is_ancestor(repo, merge.0, **tip).unwrap_or(false))
            .any(|tip| stays_above(*tip, merge.0).unwrap_or(true));
        if buried {
            bail!(
                "{} is a merge with commits above it, which would merge the same changes \
                 instead, so nothing would come back. Take out the commits above it first.",
                merge.short()
            );
        }
    }
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
pub fn move_to_new_branch(path: &Path, commits: &[CommitId]) -> Result<(Rewritten, FullName)> {
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
    Ok((rewritten(&workspace), name))
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
    refuse_buried_merges(&*ctx.repo.get()?, commits)?;
    let commits = commits.iter().map(|id| id.0).collect();
    but_api::commit::discard_commit::commit_discard(&mut ctx, commits, DryRun::No)?;
    Ok(())
}

/// Why a discard didn't throw away all it was asked to, when that isn't an error.
#[derive(Debug, PartialEq)]
pub enum Undiscarded {
    /// GitButler couldn't record it for undo, so nothing was thrown away.
    Unrecorded(String),
    /// It failed partway, after recording what undo brings back.
    Partway(String),
}

/// Throw away the `selected` uncommitted changes of the main checkout, or of the linked
/// `worktree` with that name. The main checkout's are recorded for undo first. When that
/// fails nothing is thrown away, for the user to decide whether to go on without undo, unless
/// `unrecorded` says they did.
pub fn discard(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    selected: &[Selected],
    unrecorded: bool,
) -> Result<Option<Undiscarded>> {
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
        .find(|part| part.hunks.is_some() && conflicted.contains(&part.file.change.path.as_bstr()))
    {
        bail!(
            "{} has conflicts: discard all of it, or mark it resolved first",
            part.file.path
        );
    }
    // A discard undo can't take back, a linked worktree's or one asked for anyway, isn't of
    // what was written after the diff was shown, as by an agent while the question was up.
    refuse_changed(repo, selected, worktree.is_some() || unrecorded)?;
    let conflicted: Vec<bstr::BString> = conflicted.into_iter().map(Into::into).collect();
    // Hunks that moved since the diff was shown refuse it before anything is recorded.
    let context_lines = ctx.settings.context_lines;
    let specs = current_hunks(repo, selected, taken_specs(selected)?, context_lines)?;
    // The snapshot is recorded before anything changes, as the but-api discard does: undo
    // then brings back whatever goes, conflicts included, even when some of it couldn't be
    // discarded. The oplog can't restore a linked worktree, so its discard has none.
    let mut recorded = false;
    if worktree.is_none() {
        let details = but_oplog::legacy::SnapshotDetails::new(OperationKind::DiscardChanges);
        let snapshot = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            &ctx,
            details,
            perm.read_permission(),
        )
        .and_then(|snapshot| snapshot.commit(&ctx, perm));
        match snapshot {
            Ok(()) => recorded = true,
            Err(err) if !unrecorded => {
                return Ok(Some(Undiscarded::Unrecorded(format!("{err:#}"))));
            }
            Err(err) => tracing::warn!(?err, "discarding without a snapshot to undo it"),
        }
    }
    let discarded = (|| -> Result<()> {
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
            if let Some(spec) =
                (rejected.iter()).find(|spec| left.iter().any(|c| c.path == spec.path))
            {
                bail!("Couldn't discard {}", spec.path);
            }
        }
        Ok(())
    })();
    match discarded {
        // What it threw away before failing comes back with undo, as its step stays. Paths in
        // the project as it names them, as a failed operation's card says them.
        Err(err) if recorded => Ok(Some(Undiscarded::Partway(
            super::failure_detail(&err).replace(&format!("{}/", path.display()), ""),
        ))),
        discarded => discarded.map(|()| None),
    }
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
pub fn mark_resolved(
    path: &Path,
    worktree: Option<&bstr::BStr>,
    paths: Vec<bstr::BString>,
) -> Result<()> {
    let mut ctx = open(path)?;
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
    use crate::backend::model::build::full;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::process::Command;

    use super::*;
    use crate::backend::{Checkout, DiffBody, DiffSource, FileDiff, Kept, Workspace, load_diff};

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
    pub(in crate::backend) fn target_origin_main(path: &Path) {
        git(
            path,
            &["remote", "add", "origin", "https://example.com/repo.git"],
        );
        git(path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(path, &["branch", "--set-upstream-to=origin/main", "main"]);
        crate::backend::settings::set_target(path, &full("refs/heads/main")).unwrap();
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

            set_branch_description(path, &full("refs/heads/topic"), "  Why it's there \n").unwrap();
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
                set_branch_description(path, &full("refs/heads/renamed"), "Lost?").is_err(),
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
    fn a_merge_comes_out_only_with_what_sits_on_it() {
        let dir = repo_with("one\n");
        let path = dir.path();
        git(path, &["checkout", "-q", "-b", "side"]);
        git(path, &["commit", "-q", "--allow-empty", "-m", "side"]);
        git(path, &["checkout", "-q", "-b", "feature", "main"]);
        git(path, &["commit", "-q", "--allow-empty", "-m", "mine"]);
        git(path, &["merge", "-q", "--no-edit", "side"]);
        let merge = head(path);
        git(path, &["commit", "-q", "--allow-empty", "-m", "above"]);
        let above = head(path);
        let repo = gix::open(path).unwrap();
        let refused = refuse_buried_merges(&repo, &[merge]).unwrap_err();
        assert!(
            refused.to_string().contains("commits above it"),
            "the commit above would merge the same changes: {refused:#}"
        );
        assert!(
            refuse_buried_merges(&repo, &[above, merge]).is_ok(),
            "taken out together, nothing stays above it"
        );

        git(path, &["branch", "backup"]);
        git(path, &["reset", "-q", "--hard", "HEAD~1"]);
        let repo = gix::open(path).unwrap();
        assert!(
            refuse_buried_merges(&repo, &[merge]).is_ok(),
            "at the top of what's checked out, a backup no checkout has doesn't keep it in"
        );
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
    fn a_linked_worktree_inside_the_checkout_isnt_an_uncommitted_change() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            git(
                path,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    "agent",
                    ".claude/worktrees/c1",
                ],
            );
            std::fs::write(path.join("file"), "two\n").unwrap();
            let files: Vec<String> = uncommitted(path).into_iter().map(|f| f.path).collect();
            assert_eq!(
                files,
                ["file"],
                "as agents put them, it's a checkout of its own"
            );
        });
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
            let refused = discard(path, None, &[part], true).unwrap_err();
            assert!(
                refused.to_string().contains("has conflicts"),
                "part of it would leave the rest resolved, markers and all: {refused:#}"
            );
            let whole = Selected::whole(&files[0]);
            discard(path, None, &[whole], true).unwrap();
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "ours\n"
            );
            let files = uncommitted(path);
            assert!(files.is_empty(), "nothing's left to commit: {files:?}");
        })
    }

    #[test]
    fn a_discard_undo_couldnt_take_back_discards_nothing_until_asked_to() {
        but_testsupport::isolated_app_data_dir(|| {
            // With no target, the oplog has nothing to record a snapshot against.
            let dir = repo_with("one\n");
            let path = dir.path();
            std::fs::write(path.join("file"), "two\n").unwrap();
            let whole = Selected::whole(&uncommitted(path)[0]);
            let why = discard(path, None, std::slice::from_ref(&whole), false).unwrap();
            assert!(why.is_some(), "it says why it couldn't record the discard");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "two\n",
                "and throws nothing away, as undo couldn't bring it back"
            );
            assert_eq!(discard(path, None, &[whole], true).unwrap(), None);
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "one\n",
                "asked to go on without undo, it does"
            );
        })
    }

    #[test]
    fn a_discard_that_fails_after_recording_says_so_for_undo_to_take_back() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            std::fs::create_dir(path.join("legacy")).unwrap();
            std::fs::write(path.join("legacy/README"), "old\n").unwrap();
            git(path, &["add", "legacy"]);
            git(path, &["commit", "-q", "-m", "legacy"]);
            // A folder replaced by a file of its name, which bringing the folder back fails on.
            std::fs::remove_dir_all(path.join("legacy")).unwrap();
            std::fs::write(path.join("legacy"), "now a file\n").unwrap();
            let gone = (uncommitted(path).into_iter())
                .find(|file| file.path == "legacy/README")
                .unwrap();

            let outcome = discard(path, None, &[Selected::whole(&gone)], false).unwrap();
            assert!(
                matches!(outcome, Some(super::Undiscarded::Partway(_))),
                "it recorded first, so the failure is an outcome undo takes back: {outcome:?}"
            );
        })
    }

    #[test]
    fn an_added_file_is_discarded_whole_or_not_at_all() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            std::fs::write(path.join("new"), "one\ntwo\n").unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let line = diff[0].part(Some(
                hunks[0].picked(&std::collections::BTreeSet::from([1])),
            ));
            let refused = discard(path, None, &[line], false).unwrap_err();
            assert!(
                refused
                    .to_string()
                    .contains("Only all of new can be taken out"),
                "a line of it would take the rest along: {refused:#}"
            );
            assert!(path.join("new").exists(), "nothing went");
            let hunk = diff[0].part(Some(vec![hunks[0].key]));
            assert!(hunk.hunks.is_none(), "all of its hunks is the file whole");
            assert_eq!(
                discard(path, None, std::slice::from_ref(&hunk), false).unwrap(),
                None
            );
            assert!(!path.join("new").exists(), "the whole file goes");
            discard(path, None, &[hunk], false).expect("nothing left to discard is no error");
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
            let refused = discard(path, Some("agent".into()), &[whole], true).unwrap_err();
            assert!(
                refused
                    .to_string()
                    .contains("changed since the diff was shown"),
                "what was written after the diff was read isn't thrown away unseen, as nothing \
                 brings a linked worktree's discard back: {refused}"
            );
            discard(path, Some("agent".into()), &[read()], true).unwrap();
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
            assert!(
                diff.is_empty(),
                "a staged addition deleted from the worktree has no change against HEAD to discard"
            );
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
            let err = remove(path, "agent".into(), &asked).unwrap_err();
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
            let err = remove(path, "agent".into(), &asked).unwrap_err();
            assert!(
                format!("{err:#}").contains("file"),
                "nor what was written to a file named since: {err:#}"
            );

            let moved = outside.path().join("moved");
            std::fs::rename(&linked, &moved).unwrap();
            let err = remove(path, "agent".into(), &asked).unwrap_err();
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
            discard(path, Some("agent".into()), &selected, true).unwrap_err();
            assert_eq!(
                git(&linked, &["ls-files", "-u", "file"]).lines().count(),
                3,
                "with no snapshot to undo from, the conflict is still there to resolve"
            );
            mark_resolved(path, Some("agent".into()), vec!["file".into()]).unwrap();
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
    fn a_hunk_of_a_textconv_diff_is_refused_rather_than_spliced_in() {
        but_testsupport::isolated_app_data_dir(|| {
            let mut lines: Vec<String> = twenty_lines();
            let dir = repo_with(&lines.concat());
            let path = dir.path();
            // As git-crypt shows an encrypted file: `cat` stands in for the decryption.
            git(path, &["config", "diff.plain.textconv", "cat"]);
            std::fs::write(path.join(".gitattributes"), "file diff=plain\n").unwrap();
            git(path, &["add", ".gitattributes"]);
            git(path, &["commit", "-qm", "attributes"]);
            lines[0] = "first edit\n".into();
            lines[19] = "last edit\n".into();
            std::fs::write(path.join("file"), lines.concat()).unwrap();

            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let first_hunk = diff[0].part(Some(vec![diff[0].hunks()[0].key]));
            let branch = Destination::Branch(full("refs/heads/main"));
            let err = commit(
                path,
                &branch,
                std::slice::from_ref(&first_hunk),
                None,
                "part",
                false,
            )
            .expect_err("a hunk of the converted copy isn't one of the file");
            assert!(err.to_string().contains("textconv"), "{err}");
            discard(path, None, &[first_hunk], false).expect_err("nor is one to discard");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                lines.concat(),
                "the file is as it was"
            );
            git(path, &["commit", "-qam", "both edits"]);
            let commit = head(path);
            let diff = load_diff(path, DiffSource::Commit(commit), None, None, None, None).unwrap();
            let first_hunk = diff[0].part(Some(vec![diff[0].hunks()[0].key]));
            let err = uncommit_files(path, commit, &[first_hunk])
                .expect_err("nor is one of a commit's to take out");
            assert!(err.to_string().contains("textconv"), "{err}");
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
            let main = Destination::Branch(full("refs/heads/main"));
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
            let main = Destination::Branch(full("refs/heads/main"));
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
            let main = Destination::Branch(full("refs/heads/main"));
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
                (workspace
                    .lanes_in(&Checkout::Main)
                    .flat_map(|s| &s.branches))
                .map(|b| b.ref_name.as_ref())
                .collect::<Vec<_>>(),
                [Some(&new)],
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
            crate::backend::settings::set_target(path, &full("refs/heads/main")).unwrap();
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

            let tip = workspace
                .find_branch(Some(&full("refs/heads/feature")))
                .unwrap()
                .commits[0]
                .id;
            crate::backend::upstream::land(path, &full("refs/heads/feature"), tip, false).unwrap();
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
                let mut lanes: Vec<Vec<String>> = (workspace.lanes_in(&Checkout::Main))
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
            let second = short_ref(&new).into_owned();
            let mut both = vec![vec!["feature".to_owned()], vec![second.clone()]];
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
                git(path, &["branch", "--list", &second]).trim().is_empty(),
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
                git(path, &["rev-parse", &new.to_string()]),
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
                    workspace.target_branch.as_ref(),
                    Some(&full("refs/heads/main")),
                    "the composer knows which branch not to commit to"
                );
                (workspace
                    .lanes_in(&Checkout::Main)
                    .flat_map(|s| &s.branches))
                .map(|b| (b.name.clone(), b.commits.len()))
                .collect::<Vec<_>>()
            };
            assert_eq!(
                branches(path),
                [("main".to_owned(), 1)],
                "main is listed while it has a commit origin doesn't"
            );

            let new = create_branch(path, None).unwrap();
            let name = short_ref(&new).into_owned();
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
            let title = (commits
                .branches_in(&Checkout::Main)
                .find_map(|b| b.commits.first()))
            .map(|c| &c.title);
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
            let left = git(path, &["branch", "--list", &short_ref(&name)]);
            assert_eq!(
                left, "",
                "{name} would be listed beside the commit back in place"
            );
        })
    }

    #[test]
    fn the_apps_own_operations_dont_stop_undoing_the_one_before() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let (named, done) = (oplog_head(path).unwrap(), now());
            let mut ran = vec![(started, done)];
            // Git logs moves in whole seconds.
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let started = now();
            insert_empty_commit(path, &Destination::Branch(full("refs/heads/named"))).unwrap();
            ran.push((started, now()));
            let inserted = oplog_head(path).unwrap();
            let started = now();
            let undone = undo_snapshot(path, inserted, Some((ran[1].1, &ran))).unwrap();
            assert!(matches!(undone, Undone::Done(_)), "the newest undoes");
            ran.push((started, now()));
            assert!(
                matches!(
                    undo_snapshot(path, named, Some((done, &ran))).unwrap(),
                    Undone::Done(_)
                ),
                "the branch moves the app's own operation and undo logged aren't another's"
            );
        })
    }

    #[test]
    fn undo_stops_for_a_commit_in_the_second_of_a_later_operation() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let done = now();
            let named = oplog_head(path).unwrap();
            // Another of the app's operations ran a minute on, and an agent committed in its
            // second.
            let later = (done + 60_000, done + 60_500);
            let committed = Command::new("git")
                .current_dir(path)
                .env("GIT_COMMITTER_DATE", format!("{} +0000", later.0 / 1000))
                .args(["commit", "-q", "--allow-empty", "-m", "agent work"])
                .status()
                .unwrap();
            assert!(committed.success(), "git commits");
            assert!(
                matches!(
                    undo_snapshot(path, named, Some((done, &[(started, done), later]))).unwrap(),
                    Undone::Behind(why) if why.contains("agent work")
                ),
                "a Git commit is no operation of the app's, whichever second it's in"
            );
        })
    }

    #[test]
    fn undo_refuses_without_a_log_of_head() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{oplog_head, undo_snapshot};
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let done = now();
            let named = oplog_head(path).unwrap();
            // As core.logAllRefUpdates off leaves it.
            std::fs::remove_file(path.join(".git/logs/HEAD")).unwrap();
            let undone = undo_snapshot(path, named, Some((done, &[(started, done)])));
            assert!(
                undone.is_err_and(|refused| refused.to_string().contains("no log")),
                "with nothing to tell what moved by, undo doesn't guess"
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
            let started = now();
            name_branch(path, top, "named").unwrap();
            let named = oplog_head(path).unwrap();
            let done = now();
            // Later than it, as an edit in an editor is.
            std::thread::sleep(std::time::Duration::from_millis(5));
            std::fs::write(path.join("file"), "two\n").unwrap();
            assert!(
                matches!(undo_snapshot(path, named, Some((done, &[(started, done)]))).unwrap(),
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
            let undoing = now();
            assert!(
                matches!(undo_snapshot(path, named, None).unwrap(), Undone::Done(_)),
                "what came before it is still there to undo"
            );
            let undone = now();

            let undo = oplog_head(path).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
            std::fs::write(path.join("file"), "three\n").unwrap();
            let spans = [(undoing, undone)];
            assert!(
                matches!(redo_snapshot(path, undo, &Default::default(), &spans, None).unwrap(),
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
                redo_snapshot(path, undo, &Default::default(), &[], None)
                    .unwrap()
                    .is_none(),
                "redoing would take that one out too"
            );
        })
    }

    #[test]
    fn undo_stops_for_an_applied_branch_moved_outside_the_app_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let named = oplog_head(path).unwrap();
            let done = now();
            // A minute on, a commit put on the branch from a terminal, as `update-ref` or a
            // cherry-pick in another checkout does, with HEAD left where it is.
            let later = format!("{} +0000", done / 1000 + 60);
            let in_terminal = |args: &[&str]| {
                let out = Command::new("git")
                    .current_dir(path)
                    .env("GIT_COMMITTER_DATE", &later)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(out.status.success(), "git {args:?}: {out:?}");
                String::from_utf8(out.stdout).unwrap()
            };
            let tree = git(path, &["rev-parse", "named^{tree}"]);
            let commit =
                in_terminal(&["commit-tree", tree.trim(), "-p", "named", "-m", "elsewhere"]);
            in_terminal(&["update-ref", "refs/heads/named", commit.trim()]);
            assert!(
                matches!(undo_snapshot(path, named, Some((done, &[(started, done)]))).unwrap(),
                    Undone::Behind(why) if why.contains("named")),
                "undoing would move the branch back and drop that commit, so it says so"
            );
        })
    }

    #[test]
    fn undo_goes_on_past_what_git_logged_that_a_restore_leaves() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let (_outside, linked) = agent_worktree(path);
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let done = now();
            let named = oplog_head(path).unwrap();
            // A minute on, in a terminal.
            let later = format!("{} +0000", done / 1000 + 60);
            let in_terminal = |dir: &Path, args: &[&str]| {
                let out = Command::new("git")
                    .current_dir(dir)
                    .env("GIT_COMMITTER_DATE", &later)
                    .args(args)
                    .output()
                    .unwrap();
                assert!(out.status.success(), "git {args:?}: {out:?}");
            };
            std::fs::write(linked.join("file"), "agent's\n").unwrap();
            in_terminal(&linked, &["stash", "-q"]);
            in_terminal(path, &["reset", "-q", "--hard", "HEAD"]);
            assert!(
                matches!(
                    undo_snapshot(path, named, Some((done, &[(started, done)]))).unwrap(),
                    Undone::Done(_)
                ),
                "an agent's stash in its worktree and a reset to where HEAD was move nothing \
                 the undo puts back"
            );
        })
    }

    #[test]
    fn undo_stops_for_a_commit_in_the_same_second_it_was_asked_after() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{Undone, oplog_head, undo_snapshot};
            let now = || {
                let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
                since.unwrap().as_millis() as i64
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now();
            name_branch(path, head(path), "named").unwrap();
            let done = now();
            let named = oplog_head(path).unwrap();
            // "Undo anyway" a minute on, and a terminal commit 10ms after, which Git logs at
            // the start of that second.
            let accepted = done + 60_000 + 500;
            let committed = Command::new("git")
                .current_dir(path)
                .env("GIT_COMMITTER_DATE", format!("{} +0000", accepted / 1000))
                .args(["commit", "-q", "--allow-empty", "-m", "terminal work"])
                .status()
                .unwrap();
            assert!(committed.success(), "git commits");
            assert!(
                matches!(
                    undo_snapshot(path, named, Some((accepted, &[(started, done)]))).unwrap(),
                    Undone::Behind(why) if why.contains("terminal work")
                ),
                "logged in the second the user said to undo anyway, it may be after that"
            );
        })
    }

    #[test]
    fn a_submodule_added_as_a_project_is_listed_at_its_files() {
        but_testsupport::isolated_app_data_dir(|| {
            let lib = repo_with("lib\n");
            let dir = repo_with("one\n");
            let path = dir.path();
            let from = lib.path().to_str().unwrap();
            let add = [
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                from,
                "lib",
            ];
            git(path, &add);
            let (id, _) = crate::backend::add_project(&path.join("lib")).unwrap();
            let listed = crate::backend::list_projects().unwrap();
            let project = listed.iter().find(|project| project.id == id).unwrap();
            assert_eq!(
                project.path.canonicalize().unwrap(),
                path.join("lib").canonicalize().unwrap(),
                "its repository is in the superproject's, but its files are in lib"
            );
        })
    }

    #[test]
    fn applying_a_branch_an_archived_worktree_has_lets_the_worktree_go_of_it() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::branches::{ApplyOutcome, apply};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            git(&linked, &["commit", "-q", "--allow-empty", "-m", "agent's"]);
            let theirs = head(&linked);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);

            let applied = apply(path, &full("refs/heads/agent"), Some("agent".into())).unwrap();
            assert!(
                matches!(applied, ApplyOutcome::Applied(_)),
                "the branch is applied"
            );
            assert_eq!(
                git(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
                "HEAD",
                "the worktree let go of the branch, so the workspace's commits don't move under it"
            );
            assert_eq!(
                head(&linked),
                theirs,
                "and stays on its commit, files and all"
            );
        })
    }

    #[test]
    fn a_branch_that_doesnt_apply_goes_back_to_its_archived_worktree() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::branches::{ApplyOutcome, apply};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            std::fs::write(linked.join("file"), "agent's\n").unwrap();
            git(&linked, &["commit", "-q", "-am", "agent's"]);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            std::fs::write(path.join("file"), "ours\n").unwrap();
            git(path, &["commit", "-q", "-am", "ours"]);
            name_branch(path, head(path), "ours").unwrap();

            let applied = apply(path, &full("refs/heads/agent"), Some("agent".into())).unwrap();
            assert!(
                matches!(applied, ApplyOutcome::Conflicts(_)),
                "both change the same line"
            );
            assert_eq!(
                git(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
                "agent",
                "not applied, the branch goes back to the worktree, so its next commits move it"
            );
        })
    }

    #[test]
    fn a_folder_the_enclosing_repository_ignores_isnt_added_as_it() {
        but_testsupport::isolated_app_data_dir(|| {
            // As a home folder kept as a dotfiles repository.
            let home = repo_with("one\n");
            std::fs::write(home.path().join(".gitignore"), "*\n!.gitignore\n").unwrap();
            let picked = home.path().join("code/new");
            std::fs::create_dir_all(&picked).unwrap();
            let refused = crate::backend::add_project(&picked).unwrap_err();
            assert!(
                refused.is::<crate::backend::NotARepository>(),
                "it's no work of the home folder's, so it's offered to become a repository: \
                 {refused:#}"
            );

            // One with a file force-added, as dotfiles keep ~/.config/nvim, is the home's.
            let tracked = home.path().join("tracked");
            std::fs::create_dir(&tracked).unwrap();
            std::fs::write(tracked.join("init.lua"), "x\n").unwrap();
            git(home.path(), &["add", "-f", "tracked/init.lua"]);
            assert!(
                crate::backend::add_project(&tracked).is_ok(),
                "a folder with files the repository tracks adds the repository"
            );
            // One that holds repositories names them, as anywhere else.
            let code = home.path().join("code");
            git(&code, &["init", "-q", "app"]);
            let named = crate::backend::add_project(&code).unwrap_err();
            assert!(
                format!("{named:#}").contains("app in it is"),
                "it says which to add: {named:#}"
            );

            let project = repo_with("one\n");
            std::fs::create_dir(project.path().join("src")).unwrap();
            assert!(
                crate::backend::add_project(&project.path().join("src")).is_ok(),
                "a folder the repository doesn't ignore adds the repository"
            );
        })
    }

    #[test]
    fn changes_left_out_are_said_by_reason() {
        use but_core::tree::create_tree::RejectionReason as R;
        let spec = |path: &str| DiffSpec {
            previous_path: None,
            path: path.into(),
            hunk_headers: Vec::new(),
        };
        let mut rejected: Vec<_> = (0..3000)
            .map(|n| (R::WorkspaceMergeConflict, spec(&format!("lib/{n}.ts"))))
            .collect();
        rejected.push((R::WorktreeFileMissingForObjectConversion, spec("gone.ts")));
        assert_eq!(
            super::left_out(&rejected),
            [
                "lib/0.ts, lib/1.ts, lib/2.ts and 2997 more files: it builds on changes in another \
                 branch; commit there, or move those first",
                "gone.ts: the file is gone from disk",
            ],
            "a moved folder's thousands of files are one line, not one each"
        );
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

            let refused = remove_branch(path, &full("refs/heads/side"), Some(asked)).unwrap_err();
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
    fn a_lone_branch_goes_with_its_commits_and_undoes() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            std::fs::write(path.join("feature.txt"), "work\n").unwrap();
            git(path, &["add", "feature.txt"]);
            git(path, &["commit", "-q", "-m", "feature work"]);
            let work = head(path);
            git(path, &["config", "branch.feature.remote", "origin"]);
            git(
                path,
                &["config", "branch.feature.merge", "refs/heads/feature"],
            );
            git(path, &["config", "branch.feature.description", "Why"]);
            create_branch(path, Some("second")).unwrap();
            let head_ref = |path: &Path| git(path, &["symbolic-ref", "HEAD"]).trim().to_owned();
            assert_eq!(head_ref(path), "refs/heads/gitbutler/workspace");

            delete_branch_and_commits(path, &full("refs/heads/feature"), work).unwrap();
            assert!(
                git(path, &["branch", "--list", "feature"]).is_empty(),
                "the branch is gone"
            );
            let config = Command::new("git")
                .current_dir(path)
                .args(["config", "--get-regexp", r"^branch\.feature\."])
                .output()
                .unwrap();
            assert!(
                config.stdout.is_empty(),
                "so is its configuration, which a new branch of its name would take on: {:?}",
                String::from_utf8_lossy(&config.stdout)
            );
            assert!(
                !path.join("feature.txt").exists(),
                "and what its commit changed with it"
            );
            assert_eq!(
                head_ref(path),
                "refs/heads/second",
                "the one branch left is checked out, as after unapplying"
            );

            let last = crate::backend::history::oplog_head(path).unwrap();
            crate::backend::history::undo_snapshot(path, last, None).unwrap();
            assert_eq!(
                git(path, &["rev-parse", "feature"]).trim(),
                work.0.to_string(),
                "one undo brings the branch back with its commit"
            );
            assert!(path.join("feature.txt").exists(), "and its file");
            assert_eq!(head_ref(path), "refs/heads/gitbutler/workspace");
        })
    }

    #[test]
    fn a_branch_checked_out_on_its_own_keeps_its_commits() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "not pushed"]);
            let tip = head(path);

            let refused =
                delete_branch_and_commits(path, &full("refs/heads/main"), tip).unwrap_err();
            assert!(
                format!("{refused:#}").contains("checked out on its own"),
                "there's no workspace to leave it for: {refused:#}"
            );
            assert_eq!(head(path), tip, "main and its commit stay");
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/main"
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
                (workspace.branches_in(&Checkout::Main))
                    .find(|b| b.commits.first().map(|c| c.id) == Some(top))
                    .map(|b| b.ref_name.clone())
            };
            assert_eq!(branch_at_top(path), Some(None), "it shows unnamed");

            assert_eq!(name_branch(path, top, "named").unwrap(), "refs/heads/named");
            assert_eq!(branch_at_top(path), Some(Some(full("refs/heads/named"))));

            let last = crate::backend::history::oplog_head(path).unwrap();
            crate::backend::history::undo_snapshot(path, last, None).unwrap();
            assert_eq!(branch_at_top(path), Some(None), "undo takes the name away");
        })
    }

    #[test]
    fn unstacking_says_which_branches_it_left_conflicted() {
        but_testsupport::isolated_app_data_dir(|| {
            // `top`, stacked on a branch that adds api.txt, with a commit writing `file`.
            let unstacked = |file: &str| {
                let dir = repo_with("base\n");
                let path = dir.path();
                target_origin_main(path);
                git(path, &["checkout", "-q", "-b", "top"]);
                std::fs::write(path.join("api.txt"), "list\n").unwrap();
                git(path, &["add", "api.txt"]);
                git(path, &["commit", "-q", "-m", "API"]);
                let api = head(path);
                std::fs::write(path.join(file), "list\npage\n").unwrap();
                git(path, &["add", file]);
                git(path, &["commit", "-q", "-m", "Page"]);
                create_branch(path, Some("beside")).unwrap();
                create_branch_at_commit(path, api, false).unwrap();
                tear_off_branch(path, &full("refs/heads/top")).unwrap()
            };
            assert_eq!(
                unstacked("api.txt"),
                [(Some(full("refs/heads/top")), 1)],
                "its commit changes a file only the branch it left has"
            );
            assert_eq!(
                unstacked("page.txt"),
                [],
                "a commit of its own applies anywhere"
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
            discard(path, None, &[hunk(0)], true).unwrap();
            // Read before the first discard, which took two lines out above it.
            let err = discard(path, None, &[hunk(1)], true).unwrap_err();
            assert!(
                err.to_string().contains("changed since the diff was shown"),
                "its place in the file is from before the first discard: {err}"
            );
            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let fresh = diff[0].part(Some(vec![hunks[0].key]));
            discard(path, None, &[fresh], true).unwrap();
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
            let mut first = lines;
            first[1] = "two\n".into();
            std::fs::write(path.join("staged"), first.concat()).unwrap();
            let blob = git(path, &["hash-object", "-w", "staged"]);
            let entry = format!("100644,{},file", blob.trim());
            git(path, &["update-index", "--cacheinfo", &entry]);
            git(path, &["commit", "-q", "-m", "first edit"]);
            let err = discard(path, None, &[second], true).unwrap_err();
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
            discard(path, None, &[lines_picked], true).unwrap();

            let mut expected = lines;
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
        let mut edited = lines;
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

            let found = crate::backend::pull_conflicts(path, &full("refs/heads/feature")).unwrap();
            assert_eq!(
                found,
                [(Some(full("refs/heads/feature")), 1)],
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
            let mut edited = lines;
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
