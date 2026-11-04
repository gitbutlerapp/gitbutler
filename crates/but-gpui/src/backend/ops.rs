//! Mutations of commits, branches and uncommitted changes; the UI reloads the workspace
//! afterwards. An [`Intent`], such as [`Squash`], runs under the one lock [`Run::locked`] takes,
//! in one `but_transaction` transaction or one `_with_perm` call, recording one oplog entry. One
//! that makes or moves a branch runs as the CLI's do outside the workspace
//! ([`SingleBranchMode`]). [`DiscardChanges`] records its own before it writes, as does
//! [`super::conflicts::ResolveMarkers`].

use gitbutler_repo::hooks::{ErrorData, HookResult, MessageHookResult};
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, ByteSlice as _};
use but_api::commit::json::ChangesSource;
use but_core::commit::CommitIdentifiers;
use but_core::tree::create_tree::RejectionReason;
use but_core::{DiffSpec, DryRun, RefMetadata};
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use but_transaction::single_branch_mode::SingleBranchMode;
use but_transaction::{DynamicOutcome, Transaction};
use but_workspace::commit::ChangeSource;

use super::intent::Run;
use super::{
    ChangeKind, Checkout, CommitId, FullName, Intent, OpKind, Selected, checkout, short_ref, stamp,
};

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
    fn relative_to(&self) -> (RelativeTo, InsertSide) {
        match self {
            Destination::Branch(ref_name) => {
                (RelativeTo::Reference(ref_name.clone()), InsertSide::Below)
            }
            Destination::AboveCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Above),
            Destination::BelowCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Below),
        }
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

/// Make the repository's first commit, of `changes`, on `branch`, which must be the one `HEAD`
/// names, as `git commit` does in a new repository. It records no oplog entry: there's nothing
/// before it to go back to.
fn commit_first(
    ctx: &mut but_ctx::Context,
    branch: FullName,
    changes: Vec<DiffSpec>,
    message: String,
    leave_out: bool,
) -> Result<(FullName, Made, Rejected)> {
    use gix::prelude::ObjectIdExt as _;
    let repo = ctx.repo.get()?;
    let head = repo.head_name()?.context("HEAD names no branch")?;
    if branch != head {
        bail!(
            "The first commit goes on {}, the branch checked out.",
            head.shorten()
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
    let id = match outcome.new_commit {
        Some(id) if leave_out || outcome.rejected_specs.is_empty() => id,
        made => return Err(refused(outcome.rejected_specs, made.is_some())),
    };
    but_workspace::init::publish_first_commit(&repo, branch.as_ref(), id)?;
    let made = Made {
        id: CommitId(id),
        key: but_core::Commit::from_id(id.attach(&repo))?
            .change_id()
            .to_string(),
    };
    drop(repo);
    ctx.invalidate_workspace_cache()?;
    Ok((branch, made, outcome.rejected_specs))
}

/// The branch a commit goes on, and where in it.
#[derive(Debug, Clone)]
pub enum CommitTo {
    /// On top of it.
    Branch(FullName),
    /// Right below `commit`, in `branch`, which has it.
    BelowCommit { commit: CommitId, branch: FullName },
    /// A new one named `name`, or with a made-up name, made in the commit's own oplog entry, so
    /// one undo takes back both, and placed as [`MoveToNewBranch`] places one: `beside` a branch
    /// in its stack, below it with the flag, or with `None` in a stack of its own, before the
    /// others with `first`.
    NewBranchAt {
        beside: Option<(FullName, bool)>,
        first: bool,
        name: Option<String>,
    },
}

/// Commit the `changes` of `checkout` to `to`. With `run_hooks`, the repository's hooks run in
/// Git's order. pre-commit and commit-msg run first, and either refuses the commit by failing;
/// commit-msg can also rewrite the message. post-commit runs once the commit is made, which
/// stands even if the hook fails. Hooks run outside GitButler's lock, so one that runs `but`
/// doesn't wait on it.
///
/// A change it can't take, as one building on another branch's commits, leaves the commit
/// unmade, [`CommitOutcome::Blocked`] saying why, unless `leave_out`, as the user agreed to
/// commit without such changes.
///
/// With `restack`, its branch is stacked first, in the same oplog entry, as [`Blocked::stack`]
/// offers: only for a commit on top of that branch or a new branch beside one, and only if that
/// stacks cleanly still.
#[derive(Clone)]
pub struct CommitChanges {
    pub to: CommitTo,
    pub changes: Arc<[Selected]>,
    pub checkout: Checkout,
    pub message: String,
    pub run_hooks: bool,
    pub leave_out: bool,
    pub restack: Option<Stacking>,
}

pub use but_hunk_dependency::rejection::Stacking;

/// How a commit went.
#[derive(Debug)]
pub enum CommitOutcome {
    Committed(Committed),
    /// Not made, as some of its changes couldn't go where it was asked to.
    Blocked(Blocked),
}

/// A commit not made as some of its changes couldn't go where it was asked to: those, and
/// where they could all go instead.
#[derive(Debug)]
pub struct Blocked {
    pub changes: Vec<BlockedChange>,
    /// The branch every one of them builds on, the topmost of those in their one stack, so
    /// that committing to it, or to a new branch on it, takes them all.
    pub builds_on: Option<FullName>,
    /// Whether `builds_on` has been merged, so it takes no commits: only a new branch above it
    /// can take them.
    pub builds_on_merged: bool,
    /// Whether the commit would still take some of what was asked, without them.
    pub takes_others: bool,
    /// Where they build on two stacks: stacking one on the other, after which a new branch
    /// above it takes them all, tried and found to conflict nothing.
    pub stack: Option<Stacking>,
}

/// A file's change a commit couldn't take: its hunks that build on other commits, the
/// branches those are in, or why else not.
#[derive(Debug)]
pub struct BlockedChange {
    pub path: String,
    /// Its hunks that build on commits in the workspace, where those could be told.
    pub hunks: Vec<BlockedHunk>,
    /// Branches whose commits change the file too, when no hunk could be told to build on one.
    pub suspects: Vec<FullName>,
    /// Every branch it builds on, those of `hunks` and `suspects`.
    pub builds_on: Vec<FullName>,
    pub why: &'static str,
}

/// How many of the files a [`Blocked`] commit left out the dialog draws the hunks of. It counts
/// the rest, as an agent's move of a folder can leave thousands out.
pub const BLOCKED_FILES_SHOWN: usize = 20;
/// How many of each of those files' hunks it draws.
pub const BLOCKED_HUNKS_SHOWN: usize = 4;

/// A hunk that builds on commits in the workspace: where it is, what it changes, and on what.
#[derive(Debug)]
pub struct BlockedHunk {
    /// Where it was in the file before, as a range of lines; empty where it only adds.
    pub old_start: u32,
    pub old_lines: u32,
    /// Where it is in the file as it is now, as a range of lines; empty where it only removes.
    pub new_start: u32,
    pub new_lines: u32,
    /// Its lines as the diff shows them: numbered, highlighted and with the changed words
    /// marked. Empty past the hunks the dialog draws, [`BLOCKED_HUNKS_SHOWN`] of the first
    /// [`BLOCKED_FILES_SHOWN`] files, as reading them all could take long for thousands.
    pub lines: Vec<super::DiffLine>,
    pub builds_on: Vec<BuiltOn>,
}

/// A commit a hunk builds on.
#[derive(Debug)]
pub struct BuiltOn {
    /// What it's shown by: see [`super::short_reference`].
    pub reference: String,
    pub title: String,
    /// The branch that has it, if it could be told.
    pub branch: Option<FullName>,
}

/// What a commit couldn't take of a file, as `change` explains it, reading the titles of the
/// commits it builds on from `repo`, and the lines of its hunks if it's `shown`.
fn blocked_change(
    repo: &gix::Repository,
    change: &but_hunk_dependency::rejection::RejectedChange,
    shown: bool,
) -> BlockedChange {
    let mut builds_on: Vec<FullName> = Vec::new();
    for name in change.branches().flatten() {
        if !builds_on.iter().any(|known| known.as_ref() == name) {
            builds_on.push(name.to_owned());
        }
    }
    let hunks = (change.dependencies.iter().enumerate())
        .map(|(ix, dependency)| {
            let header = &dependency.hunk;
            let lines = match shown && ix < BLOCKED_HUNKS_SHOWN {
                true => {
                    let mut hunk = super::diff::parse_hunk(&but_core::unified_diff::DiffHunk {
                        old_start: header.old_start,
                        old_lines: header.old_lines,
                        new_start: header.new_start,
                        new_lines: header.new_lines,
                        diff: dependency.diff.clone(),
                    });
                    super::annotate::emphasize(&mut hunk);
                    let path = change.path.to_str_lossy();
                    super::diff::highlight_lines(&path, hunk.lines.iter_mut());
                    hunk.lines
                }
                false => Vec::new(),
            };
            let builds_on = (dependency.commits.iter())
                .map(|commit| BuiltOn {
                    reference: super::short_reference(repo, CommitId(commit.commit_id)),
                    title: commit_title(repo, commit.commit_id),
                    branch: commit.branch.clone(),
                })
                .collect();
            BlockedHunk {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
                lines,
                builds_on,
            }
        })
        .collect();
    BlockedChange {
        path: change.path.to_string(),
        hunks,
        suspects: change.suspected_branches.clone(),
        builds_on,
        why: why(&change.reason),
    }
}

/// The title of commit `id`, or nothing if it can't be read.
fn commit_title(repo: &gix::Repository, id: gix::ObjectId) -> String {
    let message = (repo.find_commit(id).ok()).and_then(|commit| {
        let message = commit.message_raw().ok()?;
        Some(message.to_str_lossy().into_owned())
    });
    message.map_or_else(String::new, |message| super::subject_and_body(&message).0)
}

/// Changes a commit couldn't take, which roll it back unless the user agreed to leave them
/// out, and whether it would have taken the rest.
#[derive(Debug)]
struct Refused {
    rejected: Rejected,
    takes_others: bool,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Nothing was committed.\n{}",
            left_out(&self.rejected).join("\n")
        )
    }
}

impl std::error::Error for Refused {}

/// The commit refused for `rejected`, or, if nothing was rejected, for taking nothing.
fn refused(rejected: Rejected, takes_others: bool) -> anyhow::Error {
    match rejected.is_empty() {
        true => anyhow::anyhow!("Nothing could be committed."),
        false => anyhow::Error::new(Refused {
            rejected,
            takes_others,
        }),
    }
}

/// What a commit to `into` (`None` for a new branch) that was rolled back for `refused` left
/// out, and which branch it could all go on instead. That's read only with `explain`, for the
/// main checkout, whose changes the workspace's commits are what they build on.
fn blocked(
    ctx: &mut but_ctx::Context,
    perm: &mut but_ctx::access::RepoExclusive,
    Refused {
        rejected,
        takes_others,
    }: Refused,
    into: Option<&FullName>,
    explain: bool,
    specs: &[DiffSpec],
) -> Blocked {
    use but_hunk_dependency::rejection::{dependency_branch, explain_rejections, stackings};
    // The transaction rolled back, so the workspace is as the dependencies are read against.
    let explained = explain.then(|| -> Result<_> {
        ctx.invalidate_workspace_cache()?;
        let merged = merged_branches(&but_api::legacy::workspace::head_info(ctx)?);
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        let changes = explain_rejections(&repo, &ws, &rejected, into.map(|name| name.as_ref()));
        let builds_on = dependency_branch(&ws, &changes);
        let stackings = stackings(&ws, &changes, &merged);
        let merged = (builds_on.as_ref()).is_some_and(|branch| merged.contains(branch));
        let changes: Vec<_> = (changes.iter().enumerate())
            .map(|(ix, change)| blocked_change(&repo, change, ix < BLOCKED_FILES_SHOWN))
            .collect();
        Ok((changes, (builds_on, merged), stackings))
    });
    let explained = match explained.transpose() {
        Ok(explained) => explained,
        // Saying why it failed must never hide that it did.
        Err(err) => {
            tracing::warn!(?err, "couldn't read what the changes left out build on");
            None
        }
    };
    let (changes, (builds_on, builds_on_merged), stackings) = explained.unwrap_or_else(|| {
        let changes = (rejected.iter())
            .map(|(reason, spec)| BlockedChange {
                path: spec.path.to_string(),
                hunks: Vec::new(),
                suspects: Vec::new(),
                builds_on: Vec::new(),
                why: why(reason),
            })
            .collect();
        (changes, (None, false), Vec::new())
    });
    // The first that works, as each is tried in full.
    let stack = (stackings.into_iter()).find(|stacking| {
        stacks_cleanly(ctx, perm, stacking, specs.to_vec()).unwrap_or_else(|err| {
            tracing::warn!(
                ?err,
                "couldn't try stacking the branches the changes build on"
            );
            false
        })
    });
    Blocked {
        changes,
        builds_on,
        builds_on_merged,
        takes_others,
        stack,
    }
}

/// The branches of the workspace that have been merged, as the app shows them: a commit of
/// theirs is in the target, by its id or, after a squash or rebase merge, by its changes.
fn merged_branches(info: &but_workspace::RefInfo) -> Vec<FullName> {
    (info.lanes())
        .flat_map(|lane| lane.segments)
        .filter(|segment| {
            (segment.commits.iter()).any(|commit| {
                matches!(
                    commit.relation,
                    but_workspace::ref_info::LocalCommitRelation::Integrated(_)
                )
            })
        })
        .filter_map(|segment| segment.ref_name().map(ToOwned::to_owned))
        .collect()
}

/// Whether stacking as `stacking` says, then committing `specs` of the main checkout on top of
/// the branch stacked, takes them all and leaves no commit conflicted that wasn't. Tried in a
/// dry run, which changes nothing.
fn stacks_cleanly(
    ctx: &mut but_ctx::Context,
    perm: &mut but_ctx::access::RepoExclusive,
    stacking: &Stacking,
    specs: Vec<DiffSpec>,
) -> Result<bool> {
    use but_transaction::DynamicOutcome;
    let before = but_api::legacy::workspace::head_info(ctx)?;
    let mut meta = ctx.meta()?;
    let (_, outcome) = but_transaction::with_transaction_with_perm_only(
        ctx,
        &mut meta,
        perm,
        DryRun::Yes,
        |mut tx| {
            stack_in(&mut tx, Some(stacking))?;
            let on_top = RelativeTo::Reference(stacking.top.clone());
            let made = tx.create_commit(
                on_top,
                InsertSide::Below,
                specs,
                String::new(),
                ChangeSource::Head,
            )?;
            Ok(
                match made.new_commit.filter(|_| made.rejected_specs.is_empty()) {
                    Some(_) => DynamicOutcome::Commit(()),
                    None => DynamicOutcome::Rollback(()),
                },
            )
        },
    )?;
    // What the dry run read of the workspace isn't what's there.
    ctx.invalidate_workspace_cache()?;
    Ok(match outcome {
        DynamicOutcome::Commit(((), workspace)) => {
            super::upstream::newly_conflicted(&before, &workspace.conflicted_commits).is_empty()
        }
        DynamicOutcome::Rollback(()) => false,
    })
}

/// A commit made on `branch`, with the changes left out of it and what a failing post-commit
/// hook printed.
#[derive(Debug)]
pub struct Committed {
    pub branch: FullName,
    pub made: Made,
    pub left_out: LeftOut,
    pub hook_failed: Option<String>,
}

/// A new commit, with the change id a reload finds it by once an agent rewrote it.
#[derive(Debug)]
pub struct Made {
    pub id: CommitId,
    pub key: String,
}

impl From<CommitIdentifiers> for Made {
    fn from(made: CommitIdentifiers) -> Self {
        Self {
            id: CommitId(made.id),
            key: made.change_id.to_string(),
        }
    }
}

type Rejected = Vec<(RejectionReason, DiffSpec)>;

impl Intent for CommitChanges {
    type Done = CommitOutcome;

    fn apply(self, run: &mut Run) -> Result<CommitOutcome> {
        let Self {
            to,
            changes,
            checkout,
            message,
            run_hooks,
            leave_out,
            restack,
        } = self;
        let worktree = checkout.linked();
        if let Some(stacking) = &restack {
            let goes_on_it = match &to {
                CommitTo::NewBranchAt {
                    beside: Some((branch, below)),
                    ..
                } => !below && *branch == stacking.top,
                CommitTo::Branch(branch) => *branch == stacking.top,
                CommitTo::BelowCommit { .. } | CommitTo::NewBranchAt { beside: None, .. } => false,
            };
            if worktree.is_some() || !goes_on_it {
                bail!(
                    "Only {} or a new branch in the workspace goes on branches stacked for it",
                    short_ref(&stacking.top)
                );
            }
        }
        // A new branch goes where the main checkout's would, unless it's placed beside one.
        if matches!(to, CommitTo::NewBranchAt { beside: None, .. }) && worktree.is_some() {
            bail!("The worktree has no branch of its own checked out");
        }
        // The hooks run in the main checkout, not where a linked worktree's changes are.
        let run_hooks = run_hooks && worktree.is_none();
        // No changes make an empty commit.
        let specs = selected_specs(&changes);
        run.refuse_index_locked()?;
        let ctx = run.ctx();
        refuse_unfinished(&super::checkout(ctx, worktree)?, worktree.is_none())?;
        refuse_conflicted(ctx, worktree, &changes)?;
        refuse_converted(&changes)?;
        let message = match run_hooks {
            true => hooks_before_commit(ctx, &specs, message)?,
            false => message,
        };
        // The branch it goes on already, which nothing left out builds on.
        let into = match &to {
            CommitTo::Branch(branch) | CommitTo::BelowCommit { branch, .. } => Some(branch.clone()),
            CommitTo::NewBranchAt { .. } => None,
        };
        let outcome = run.locked(|ctx, perm| {
            // What it was tried with when it was offered may have changed since, as by an agent.
            if let Some(stacking) = &restack
                && !stacks_cleanly(ctx, perm, stacking, specs.clone())?
            {
                let moved: Vec<_> = stacking.branches.iter().map(short_ref).collect();
                bail!(
                    "Stacking {} on {} wouldn't take these changes cleanly any more, so nothing \
                     was changed",
                    moved.join(", "),
                    short_ref(&stacking.onto)
                );
            }
            let tried = specs.clone();
            let made = commit_locked(ctx, perm, to, specs, message, worktree, leave_out, restack);
            match made.map_err(|err| err.downcast::<Refused>()) {
                Ok(made) => Ok(Ok(made)),
                Err(Ok(refused)) => Ok(Err(blocked(
                    ctx,
                    perm,
                    refused,
                    into.as_ref(),
                    worktree.is_none(),
                    &tried,
                ))),
                Err(Err(err)) => Err(err),
            }
        })?;
        let (branch, made, rejected) = match outcome {
            Ok(made) => made,
            Err(blocked) => return Ok(CommitOutcome::Blocked(blocked)),
        };
        let hook_failed = run_hooks.then(|| post_commit(run.ctx())).flatten();
        Ok(CommitOutcome::Committed(Committed {
            branch,
            made,
            left_out: left_out(&rejected),
            hook_failed,
        }))
    }
}

/// Commit `specs` of the main checkout, or of the linked `worktree`, to `to`, under the lock,
/// in one oplog entry, stacking first as `restack` says. Changes it can't take refuse it with
/// [`Refused`], unless `leave_out`.
#[expect(clippy::too_many_arguments)]
fn commit_locked(
    ctx: &mut but_ctx::Context,
    perm: &mut but_ctx::access::RepoExclusive,
    to: CommitTo,
    specs: Vec<DiffSpec>,
    message: String,
    worktree: Option<&BStr>,
    leave_out: bool,
    restack: Option<Stacking>,
) -> Result<(FullName, Made, Rejected)> {
    let unborn = ctx.repo.get()?.head()?.is_unborn();
    // A new branch, beside one or with `None` of its own, and with `true` first.
    let (branch, new_at, below) = match to {
        CommitTo::Branch(branch) if unborn => {
            return commit_first(ctx, branch, specs, message, leave_out);
        }
        CommitTo::Branch(branch) => (branch, None, None),
        CommitTo::BelowCommit { commit, branch } => (branch, None, Some(commit)),
        CommitTo::NewBranchAt {
            beside,
            first,
            name,
        } => {
            refuse_unborn(ctx)?;
            let name = match name {
                Some(name) => branch_ref(&name)?,
                None => but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?,
            };
            (name, Some((beside, first)), None)
        }
    };
    let linked = (worktree.map(|name| super::checkout(ctx, Some(name)))).transpose()?;
    let source = match (worktree, &linked) {
        (Some(name), Some(repo)) => ChangeSource::Worktree { repo, name },
        _ => ChangeSource::Head,
    };
    // Named so undo moves it back, leaving the changes uncommitted there.
    let worktree_branch = match worktree {
        Some(name) => ctx.worktree_head(name)?.and_then(|head| head.ref_name),
        None => None,
    };
    // Named so undo deletes it again.
    let new_branch = new_at.as_ref().map(|_| branch.as_ref());
    let refs: Vec<&gix::refs::FullNameRef> = (new_branch.into_iter())
        .chain(worktree_branch.as_ref().map(|name| name.as_ref()))
        .collect();
    let details = SnapshotDetails::new(OperationKind::CreateCommit);
    let mut meta = ctx.meta()?;
    let at = (&branch, below);
    let outcome = match new_at {
        // A branch made for it runs as `but commit` makes one.
        Some((beside, first)) => {
            let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
            let independent = beside.is_none();
            sbm.transaction_with_workspace_setup(
                ctx,
                &mut meta,
                details,
                &refs,
                perm,
                independent,
                |mut tx| {
                    stack_in(&mut tx, restack.as_ref())?;
                    match &beside {
                        Some((beside, below)) => sbm.create_stacked_reference(
                            &mut tx,
                            branch.as_ref(),
                            beside.as_ref(),
                            position(*below),
                        )?,
                        // A stack's place is its tip's among the workspace commit's
                        // parents.
                        None => sbm.create_unstacked_reference(
                            &mut tx,
                            branch.as_ref(),
                            first.then_some(0),
                        )?,
                    }
                    commit_in(&mut tx, at, specs, message, source, leave_out)
                },
            )?
        }
        None => but_transaction::with_transaction_with_perm(
            ctx,
            &mut meta,
            perm,
            details,
            &refs,
            DryRun::No,
            |mut tx| {
                stack_in(&mut tx, restack.as_ref())?;
                commit_in(&mut tx, at, specs, message, source, leave_out)
            },
        )?,
    };
    match outcome {
        DynamicOutcome::Commit(((made, rejected), _workspace)) => {
            Ok((branch.clone(), made, rejected))
        }
        DynamicOutcome::Rollback((rejected, takes_others)) => Err(refused(rejected, takes_others)),
    }
}

/// Stack in `tx` as `restack` says, if it's given, before a commit goes on the branch on top:
/// each branch moved directly above the one before it, the first above its `onto`.
fn stack_in<M: RefMetadata>(
    tx: &mut Transaction<'_, '_, M>,
    restack: Option<&Stacking>,
) -> Result<()> {
    let Some(stacking) = restack else {
        return Ok(());
    };
    let mut below = &stacking.onto;
    for branch in &stacking.branches {
        let onto = RelativeTo::Reference(below.clone());
        tx.move_branch(branch.as_ref(), onto, InsertSide::Above)?;
        below = branch;
    }
    Ok(())
}

/// Commit `specs` in `tx` on top of `branch`, or right below its commit `below`. Changes it
/// can't take roll it back, leaving no branch made for it behind, unless `leave_out`; the
/// rollback says whether it would have taken the rest.
fn commit_in<M: RefMetadata>(
    tx: &mut Transaction<'_, '_, M>,
    (branch, below): (&FullName, Option<CommitId>),
    specs: Vec<DiffSpec>,
    message: String,
    source: ChangeSource<'_>,
    leave_out: bool,
) -> Result<DynamicOutcome<(Made, Rejected), (Rejected, bool)>> {
    let relative_to = match below {
        Some(commit) => RelativeTo::Commit(commit.0),
        None => RelativeTo::Reference(branch.clone()),
    };
    let made = tx.create_commit(relative_to, InsertSide::Below, specs, message, source)?;
    let rejected = made.rejected_specs;
    Ok(match made.new_commit {
        Some(commit) if leave_out || rejected.is_empty() => {
            DynamicOutcome::Commit((Made::from(commit), rejected))
        }
        commit => DynamicOutcome::Rollback((rejected, commit.is_some())),
    })
}

/// Where `below` puts a new branch beside another: right below it, or else right above.
fn position(below: bool) -> but_workspace::branch::create_reference::Position {
    use but_workspace::branch::create_reference::Position;
    match below {
        true => Position::Below,
        false => Position::Above,
    }
}

/// Run pre-commit on what `changes` would commit, then commit-msg on `message`, returning the
/// message it leaves. Either refuses the commit by failing.
fn hooks_before_commit(
    ctx: &but_ctx::Context,
    changes: &[DiffSpec],
    message: String,
) -> Result<String> {
    pre_commit(ctx, changes)?;
    // Ending in a line break, as Git hands it over, so what a hook appends starts a line of
    // its own rather than running on from the title.
    match but_api::legacy::repo::message_hook(ctx, format!("{message}\n"))? {
        MessageHookResult::Failure(ErrorData { error }) => {
            bail!(
                "The commit-msg hook refused the commit:\n{}",
                hook_output(&error)
            )
        }
        MessageHookResult::Message(rewritten) => Ok(rewritten.message.trim_end().to_owned()),
        MessageHookResult::Success | MessageHookResult::NotConfigured => Ok(message),
    }
}

/// Create a new stack, named `name` or with a made-up name, and return its full ref name, as
/// `but branch new` does. With the default branch checked out on its own, the new branch goes
/// on top of it and is checked out, as `git checkout -b` would. Beside another branch, both go
/// in GitButler's workspace.
pub struct CreateBranch {
    pub name: Option<String>,
}

impl Intent for CreateBranch {
    type Done = FullName;

    fn apply(self, run: &mut Run) -> Result<FullName> {
        let name = self.name.as_deref().map(branch_ref).transpose()?;
        run.locked(|ctx, perm| {
            refuse_unborn(ctx)?;
            // A branch made here and checked out couldn't be undone: the oplog takes a detached
            // HEAD for the workspace and would restore that instead.
            refuse_unfinished(&*ctx.repo.get()?, true)?;
            let new_ref = match name {
                Some(name) => name,
                None => but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?,
            };
            // It lists the new branch, so undo deletes it again.
            let details = SnapshotDetails::new(OperationKind::CreateBranch)
                .with_trailers([Trailer::Name(new_ref.to_string())]);
            let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
            let mut meta = ctx.meta()?;
            sbm.transaction_with_workspace_setup(
                ctx,
                &mut meta,
                details,
                &[new_ref.as_ref()],
                perm,
                true,
                |mut tx| sbm.create_unstacked_reference(&mut tx, new_ref.as_ref(), None),
            )?;
            Ok(new_ref)
        })
    }
}

/// Stack a new branch with a made-up name on top of `below`, as `but branch new` does: on top
/// of the branch checked out on its own, it's checked out.
pub struct CreateDependentBranch {
    pub below: FullName,
}

impl Intent for CreateDependentBranch {
    type Done = FullName;

    fn apply(self, run: &mut Run) -> Result<FullName> {
        run.locked(|ctx, perm| {
            let new_ref = but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?;
            // It lists the new branch, so undo deletes it again.
            let details = SnapshotDetails::new(OperationKind::CreateBranch)
                .with_trailers([Trailer::Name(new_ref.to_string())]);
            let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
            let mut meta = ctx.meta()?;
            sbm.transaction_with_workspace_setup(
                ctx,
                &mut meta,
                details,
                &[new_ref.as_ref()],
                perm,
                false,
                |mut tx| {
                    let (new_ref, below) = (new_ref.as_ref(), self.below.as_ref());
                    sbm.create_stacked_reference(&mut tx, new_ref, below, position(false))
                },
            )?;
            Ok(new_ref)
        })
    }
}

/// Make a new branch, with a made-up name, whose newest commit is `commit`, taking it and the
/// commits below it in its branch; or with `below`, one that ends just under `commit`.
pub struct CreateBranchAtCommit {
    pub commit: CommitId,
    pub below: bool,
}

impl Intent for CreateBranchAtCommit {
    type Done = FullName;

    fn apply(self, run: &mut Run) -> Result<FullName> {
        use but_workspace::branch::create_reference::{Anchor, Position};
        let position = match self.below {
            true => Position::Below,
            false => Position::Above,
        };
        let anchor = Anchor::AtCommit {
            commit_id: self.commit.0,
            position,
        };
        run.locked(|ctx, perm| {
            let new_ref = but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?;
            // It lists the new branch, so undo deletes it again.
            let details = SnapshotDetails::new(OperationKind::CreateBranch)
                .with_trailers([Trailer::Name(new_ref.to_string())]);
            let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
            let mut meta = ctx.meta()?;
            sbm.transaction_with_workspace_setup(
                ctx,
                &mut meta,
                details,
                &[new_ref.as_ref()],
                perm,
                false,
                |mut tx| tx.create_reference(new_ref.as_ref(), anchor, None),
            )?;
            Ok(new_ref)
        })
    }
}

/// Stack `branch` on top of `onto`. The branches above it stay, on the one it was on.
pub struct StackBranch {
    pub branch: FullName,
    pub onto: FullName,
}

impl Intent for StackBranch {
    type Done = Vec<(Option<FullName>, usize)>;

    fn apply(self, run: &mut Run) -> Result<Self::Done> {
        let onto = RelativeTo::Reference(self.onto);
        move_branch(run, &self.branch, BranchTo::Beside(onto, InsertSide::Above))
    }
}

/// Move `branch` right below `commit`, the last commit of the branch it then sits under.
/// Below that branch's reference instead, `branch` would take over its commits.
pub struct MoveBranchBelow {
    pub branch: FullName,
    pub commit: CommitId,
}

impl Intent for MoveBranchBelow {
    type Done = Vec<(Option<FullName>, usize)>;

    fn apply(self, run: &mut Run) -> Result<Self::Done> {
        let below = RelativeTo::Commit(self.commit.0);
        move_branch(
            run,
            &self.branch,
            BranchTo::Beside(below, InsertSide::Below),
        )
    }
}

/// Take `branch` out of its stack into a stack of its own, after the others, or with `first`
/// before them.
pub struct TearOffBranch {
    pub branch: FullName,
    pub first: bool,
}

impl Intent for TearOffBranch {
    type Done = Vec<(Option<FullName>, usize)>;

    fn apply(self, run: &mut Run) -> Result<Self::Done> {
        let first = self.first;
        move_branch(run, &self.branch, BranchTo::Apart { first })
    }
}

/// Where [`move_branch`] puts a branch.
enum BranchTo {
    /// To that side of a branch or commit.
    Beside(RelativeTo, InsertSide),
    /// Into a stack of its own, after the others, or with `first` before them.
    Apart { first: bool },
}

/// Move `branch` where `to` says, saying which branches it left with conflicted commits they
/// didn't have, each with how many.
fn move_branch(
    run: &mut Run,
    branch: &FullName,
    to: BranchTo,
) -> Result<Vec<(Option<FullName>, usize)>> {
    let kind = match to {
        BranchTo::Beside(..) => OperationKind::MoveBranch,
        BranchTo::Apart { .. } => OperationKind::TearOffBranch,
    };
    // Out of a stack, it's a branch of its own, which a branch checked out on its own only has
    // beside it in GitButler's workspace, as `but move` sets it up.
    let independent = matches!(to, BranchTo::Apart { .. });
    run.locked(|ctx, perm| {
        let before = but_api::legacy::workspace::head_info(ctx)?;
        let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
        let mut meta = ctx.meta()?;
        let moved = sbm.transaction_with_workspace_setup(
            ctx,
            &mut meta,
            SnapshotDetails::new(kind),
            &[],
            perm,
            independent,
            |mut tx| match to {
                BranchTo::Beside(relative_to, side) => {
                    tx.move_branch(branch.as_ref(), relative_to, side)
                }
                // A stack's place is its tip's among the workspace commit's parents.
                BranchTo::Apart { first } => {
                    tx.tear_off_branch(branch.as_ref(), first.then_some(0))
                }
            },
        )?;
        Ok(super::upstream::newly_conflicted(
            &before,
            &moved.conflicted_commits,
        ))
    })
}

/// Take the stack holding `branch` out of the workspace. Its branches stay in the repository.
pub struct UnapplyStack {
    pub branch: FullName,
}

impl Intent for UnapplyStack {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        run.locked(|ctx, perm| {
            let info = but_api::legacy::workspace::head_info(ctx)?;
            let stack_id = info
                .stacks
                .iter()
                .find(|stack| stack.segments.iter().any(|s| holds(s, &self.branch)))
                .and_then(|stack| stack.id)
                .context("That branch isn't in the workspace any more")?;
            but_api::legacy::virtual_branches::unapply_stack_with_perm(ctx, stack_id, perm)
        })
    }
}

pub(super) fn holds(segment: &but_workspace::ref_info::Segment, branch: &FullName) -> bool {
    segment.ref_name() == Some(branch.as_ref())
}

/// An empty commit at `at`, to move changes into later.
pub struct InsertEmptyCommit {
    pub at: Destination,
}

impl Intent for InsertEmptyCommit {
    type Done = CommitId;

    fn apply(self, run: &mut Run) -> Result<CommitId> {
        let (relative_to, side) = self.at.relative_to();
        let (made, _workspace) = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::InsertBlankCommit),
                &[],
                DryRun::No,
                |mut tx| {
                    let made = tx.insert_blank_commit(relative_to, side)?;
                    Ok(but_transaction::Commit(CommitId(made.id)))
                },
            )
        })?;
        Ok(made)
    }
}

/// Put copies of `commits`, listed newest first, at `to` in one go, keeping the originals
/// where they are. The oldest goes in first and each newer one on top, as in their history.
/// Done is what it rewrote, and the newest copy.
pub struct CherryPick {
    pub commits: Arc<[CommitId]>,
    pub to: Destination,
}

impl Intent for CherryPick {
    type Done = (Rewritten, Option<CommitId>);

    fn apply(self, run: &mut Run) -> Result<(Rewritten, Option<CommitId>)> {
        let Self { commits, to } = self;
        let (relative_to, side) = to.relative_to();
        let (newest, workspace) = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::CherryPick).with_count(commits.len()),
                &[],
                DryRun::No,
                |mut tx| {
                    let oldest_first = commits.iter().rev().map(|id| id.0);
                    let copies = tx.cherry_pick_commits(oldest_first, relative_to, side, false)?;
                    let newest = copies.last().map(|copy| CommitId(copy.id));
                    Ok(but_transaction::Commit(newest))
                },
            )
        })?;
        Ok((rewritten(&workspace), newest))
    }
}

/// Rename `branch` to `name`, its description going with it, and return its new full ref name.
pub struct RenameBranch {
    pub branch: FullName,
    pub name: String,
}

impl Intent for RenameBranch {
    type Done = FullName;

    fn apply(self, run: &mut Run) -> Result<FullName> {
        let Self { branch, name } = self;
        run.locked(|ctx, perm| {
            let outcome =
                but_api::branch::branch_rename_with_perm(ctx, branch.clone(), name, perm)?;
            // Its description goes with it, as Git keeps it by the name.
            let moved = super::branches::move_description(
                &*ctx.repo.get()?,
                branch.shorten(),
                outcome.new_ref.shorten(),
            );
            if let Err(err) = moved {
                tracing::warn!(?err, "couldn't move the renamed branch's description");
            }
            Ok(outcome.new_ref)
        })
    }
}

/// Name the main checkout's unnamed branch whose newest commit is `top`, putting a `HEAD`
/// detached there on it, and return its full ref name.
pub struct NameBranch {
    pub top: CommitId,
    pub name: String,
}

impl Intent for NameBranch {
    type Done = FullName;

    fn apply(self, run: &mut Run) -> Result<FullName> {
        let new_ref = branch_ref(&self.name)?;
        run.locked(|ctx, perm| {
            let outcome = but_api::branch::branch_name_segment_with_perm(
                ctx,
                ChangesSource::Head,
                self.top.0,
                Some(new_ref),
                perm,
            )?;
            Ok(outcome.new_ref)
        })
    }
}

/// Delete `branch` with its commits, in one step that one undo takes back, as `but branch
/// delete` does. A checkout on it moves to the branch below, or the target. A workspace left
/// with one stack checks that out, as unapplying does. Outside the workspace only its ref goes,
/// its commits with it unless another ref has them. One whose newest commit is no longer `tip`,
/// `None` when it had none, stays: its new commits would go too.
pub struct DeleteBranchAndCommits {
    pub branch: FullName,
    pub tip: Option<CommitId>,
}

impl Intent for DeleteBranchAndCommits {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let Self { branch, tip } = self;
        run.locked(|ctx, perm| {
            // The target is what a checkout on a deleted branch falls back to.
            let target = ctx.project_meta()?.target_ref;
            if target.is_some_and(|target| target.as_local_full_name_ref() == branch.as_ref()) {
                bail!("{} is the target branch", short_ref(&branch));
            }
            let commits: Option<Vec<gix::ObjectId>> = {
                let (_, ws, _) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
                (ws.find_segment_by_refname(branch.as_ref()))
                    .map(|segment| segment.commits.iter().map(|commit| commit.id).collect())
            };
            let Some(commits) = commits else {
                // Removing a ref that's gone does nothing, so say so rather than that it went, as
                // after an agent renamed the branch while the confirmation was up.
                match super::ref_tip(&*ctx.repo.get()?, branch.as_ref()) {
                    None => bail!("{} was renamed or deleted meanwhile", short_ref(&branch)),
                    Some(at) if tip.is_none_or(|tip| tip.0 != at) => bail!(
                        "{} changed since you asked, so it wasn't deleted",
                        short_ref(&branch)
                    ),
                    Some(_) => {}
                }
                but_api::branch::branch_remove_with_perm(ctx, branch, perm)?;
                return Ok(());
            };
            if commits.first().copied() != tip.map(|tip| tip.0) {
                bail!(
                    "{} changed since you asked, so it wasn't deleted",
                    short_ref(&branch)
                );
            }
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::DeleteBranch)
                    .with_trailers([Trailer::Name(branch.to_string())]),
                &[branch.as_ref()],
                DryRun::No,
                |mut tx| {
                    if !commits.is_empty() {
                        tx.discard_commits(commits)?;
                    }
                    tx.remove_reference(branch.as_ref())?;
                    Ok(())
                },
            )?;
            let (mut repo, ws, _) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
            // The transaction leaves Git's `branch.<name>` section, as its upstream and
            // description, which a branch made later under its name would take on.
            but_workspace::branch::remove_reference::delete_local_branch(
                &mut repo,
                branch.as_ref(),
            )?;
            but_workspace::branch::dissolve_unnecessary_workspace(
                &ws,
                &repo,
                &meta,
                Default::default(),
            )?;
            Ok(())
        })
    }
}

/// Add the `changes` of `checkout` to `into`, keeping its message. With `run_hooks`, in the
/// main checkout only, pre-commit runs first and can refuse, outside GitButler's lock as for
/// [`CommitChanges`], and post-commit runs after, as the desktop app does.
pub struct Amend {
    pub into: CommitId,
    pub changes: Arc<[Selected]>,
    pub checkout: Checkout,
    /// The message it gets instead of its own, if any.
    pub message: Option<String>,
    pub run_hooks: bool,
}

/// The commits an amend rewrote, with the changes left out of it and what a failing
/// post-commit hook printed.
#[derive(Debug)]
pub struct Amended {
    pub rewritten: Rewritten,
    pub left_out: LeftOut,
    pub hook_failed: Option<String>,
}

impl Intent for Amend {
    type Done = Amended;

    fn apply(self, run: &mut Run) -> Result<Amended> {
        use but_transaction::DynamicOutcome;
        let Self {
            into,
            changes,
            checkout,
            message,
            run_hooks,
        } = self;
        let worktree = checkout.linked();
        let run_hooks = run_hooks && worktree.is_none();
        let specs = selected_specs(&changes);
        run.refuse_index_locked()?;
        let ctx = run.ctx();
        refuse_unfinished(&super::checkout(ctx, worktree)?, worktree.is_none())?;
        refuse_conflicted(ctx, worktree, &changes)?;
        refuse_converted(&changes)?;
        if run_hooks {
            pre_commit(ctx, &specs)?;
        }
        let outcome = run.locked(|ctx, perm| {
            let linked = (worktree.map(|name| super::checkout(ctx, Some(name)))).transpose()?;
            let source = match (worktree, &linked) {
                (Some(name), Some(repo)) => ChangeSource::Worktree { repo, name },
                _ => ChangeSource::Head,
            };
            // Named so undo moves it back, leaving the changes uncommitted there.
            let worktree_branch = match worktree {
                Some(name) => ctx.worktree_head(name)?.and_then(|head| head.ref_name),
                None => None,
            };
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::AmendCommit),
                worktree_branch
                    .as_ref()
                    .map(|name| name.as_ref())
                    .as_slice(),
                DryRun::No,
                |mut tx| {
                    let made = tx.amend_commit(into.0, specs, source)?;
                    let Some(amended) = made.new_commit else {
                        return Ok(DynamicOutcome::Rollback(made.rejected_specs));
                    };
                    // In the same transaction, so undo takes both back at once.
                    let amended = match &message {
                        Some(message) => tx.reword_commit(into.0, message.as_str().into())?,
                        None => amended,
                    };
                    Ok(DynamicOutcome::Commit((
                        CommitId(amended.id),
                        made.rejected_specs,
                    )))
                },
            )
        })?;
        let ((amended, rejected), workspace) = match outcome {
            DynamicOutcome::Commit(amended) => amended,
            DynamicOutcome::Rollback(rejected) => {
                bail!(
                    "Nothing could be amended.\n{}",
                    left_out(&rejected).join("\n")
                )
            }
        };
        let hook_failed = run_hooks.then(|| post_commit(run.ctx())).flatten();
        let mut rewritten = rewritten(&workspace);
        rewritten.insert(into, amended);
        Ok(Amended {
            rewritten,
            left_out: left_out(&rejected),
            hook_failed,
        })
    }
}

/// Move `commits` to `to`.
pub struct MoveCommits {
    pub commits: Arc<[CommitId]>,
    pub to: Destination,
}

impl Intent for MoveCommits {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        let Self { commits, to } = self;
        let (relative_to, side) = to.relative_to();
        let workspace = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::MoveCommit).with_count(commits.len()),
                &[],
                DryRun::No,
                |mut tx| tx.move_commits(commits.iter().map(|id| id.0), relative_to, side),
            )
        })?;
        Ok(rewritten(&workspace))
    }
}

/// Which messages a squash keeps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Messages {
    /// The target's first, then the others' in the order they were written.
    #[default]
    Both,
    /// Only the target's.
    Target,
}

/// Squash `commits`, newest first as picked, into `into`, keeping the messages `keep` says.
/// With `emptied`, they're all of that branch, which goes with them, as `but squash` takes a
/// branch squashed into a commit elsewhere.
pub struct Squash {
    pub commits: Arc<[CommitId]>,
    pub into: CommitId,
    pub keep: Messages,
    pub emptied: Option<FullName>,
}

impl Intent for Squash {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        use but_workspace::commit::squash_commits::MessageCombinationStrategy;
        let Self {
            commits,
            into,
            keep,
            emptied,
        } = self;
        let strategy = match keep {
            Messages::Both => MessageCombinationStrategy::KeepBoth,
            Messages::Target => MessageCombinationStrategy::KeepTarget,
        };
        let (made, workspace) = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::SquashCommit),
                // Named so undo brings the branch back.
                emptied.as_ref().map(|name| name.as_ref()).as_slice(),
                DryRun::No,
                |mut tx| {
                    if let Some(branch) = &emptied {
                        tx.remove_reference(branch.as_ref())?;
                    }
                    let oldest_first = commits.iter().rev().map(|id| id.0);
                    let made = tx.squash_commits(oldest_first, into.0, strategy)?;
                    Ok(but_transaction::Commit(CommitId(made.id)))
                },
            )
        })?;
        let mut rewritten = rewritten(&workspace);
        // The squashed commits live on in the result.
        rewritten.extend(commits.iter().chain([&into]).map(|id| (*id, made)));
        Ok(rewritten)
    }
}

/// Turn `commits` back into uncommitted changes. With `emptied`, they're all of that branch,
/// which goes with them, as `but squash` uncommits a branch.
pub struct Uncommit {
    pub commits: Arc<[CommitId]>,
    pub emptied: Option<FullName>,
}

impl Intent for Uncommit {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let Self { commits, emptied } = self;
        run.locked(|ctx, perm| {
            refuse_buried_merges(&*ctx.repo.get()?, &commits)?;
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::UndoCommit)
                    .with_count(commits.len())
                    .with_trailers(commits.iter().map(|id| Trailer::Sha(id.0))),
                // Named so undo brings the branch back.
                emptied.as_ref().map(|name| name.as_ref()).as_slice(),
                DryRun::No,
                |mut tx| {
                    tx.uncommit_commits(commits.iter().map(|id| id.0))?;
                    if let Some(branch) = &emptied {
                        tx.remove_reference(branch.as_ref())?;
                    }
                    Ok(())
                },
            )?;
            Ok(())
        })
    }
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
                super::short_reference(repo, *merge)
            );
        }
    }
    Ok(())
}

/// Take `files` out of `commit` and leave them uncommitted.
pub struct UncommitFiles {
    pub commit: CommitId,
    pub files: Arc<[Selected]>,
}

impl Intent for UncommitFiles {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        let Self { commit, files } = self;
        let changes = taken_specs(&files)?;
        let outcome = run.locked(|ctx, perm| {
            but_api::commit::uncommit::commit_uncommit_changes_with_perm(
                ctx,
                commit.0,
                changes,
                None,
                DryRun::No,
                perm,
            )
        })?;
        Ok(rewritten(&outcome.workspace))
    }
}

/// Move `files` that `from` changes into `to`.
pub struct MoveFiles {
    pub from: CommitId,
    pub to: CommitId,
    pub files: Arc<[Selected]>,
}

impl Intent for MoveFiles {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        let Self { from, to, files } = self;
        let changes = taken_specs(&files)?;
        let workspace = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::MoveCommitFile),
                &[],
                DryRun::No,
                |mut tx| {
                    tx.move_committed_changes_between(from.0, to.0, changes)?;
                    Ok(())
                },
            )
        })?;
        Ok(rewritten(&workspace))
    }
}

/// Take `files` (or some of their hunks) out of `commit` as if it had never made them.
pub struct DiscardFromCommit {
    pub commit: CommitId,
    pub files: Arc<[Selected]>,
}

impl Intent for DiscardFromCommit {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        let Self { commit, files } = self;
        let changes = taken_specs(&files)?;
        let workspace = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::DiscardChanges)
                    .with_trailers([Trailer::Sha(commit.0)]),
                &[],
                DryRun::No,
                |mut tx| {
                    tx.discard_changes_from_commit(commit.0, changes)?;
                    Ok(())
                },
            )
        })?;
        Ok(rewritten(&workspace))
    }
}

/// Move `files` (or some of their hunks) out of `commit` into a new commit right above it.
/// Done is the rewrites and the new commit.
pub struct SplitOff {
    pub commit: CommitId,
    pub files: Arc<[Selected]>,
}

impl Intent for SplitOff {
    type Done = (Rewritten, CommitId);

    fn apply(self, run: &mut Run) -> Result<(Rewritten, CommitId)> {
        let Self { commit, files } = self;
        let changes = taken_specs(&files)?;
        let (new, workspace) = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                // There's no kind for a split; it makes a commit, rather than moving a file.
                SnapshotDetails::new(OperationKind::CreateCommit),
                &[],
                DryRun::No,
                |mut tx| {
                    let blank =
                        tx.insert_blank_commit(RelativeTo::Commit(commit.0), InsertSide::Above)?;
                    let moved = tx.move_committed_changes_between(commit.0, blank.id, changes)?;
                    Ok(but_transaction::Commit(CommitId(moved.id)))
                },
            )
        })?;
        Ok((rewritten(&workspace), new))
    }
}

/// Move `commits` into a new branch of their own: beside the other stacks, after them or with
/// `first` before them, or in the stack of branch `beside`, right above it, or right below it
/// with `true`. With `copy`, copies of them go there instead, as [`CherryPick`] puts them, and
/// they stay where they are.
/// Done is what moved, and the new branch's full name.
pub struct MoveToNewBranch {
    pub commits: Arc<[CommitId]>,
    pub beside: Option<(FullName, bool)>,
    pub first: bool,
    pub copy: bool,
}

impl Intent for MoveToNewBranch {
    type Done = (Rewritten, FullName);

    fn apply(self, run: &mut Run) -> Result<(Rewritten, FullName)> {
        let Self {
            commits,
            beside,
            first,
            copy,
        } = self;
        let details = match copy {
            true => SnapshotDetails::new(OperationKind::CherryPick).with_count(commits.len()),
            false => SnapshotDetails::new(OperationKind::MoveCommit),
        };
        run.locked(|ctx, perm| {
            let name = but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?;
            let sbm = SingleBranchMode::new(ctx, perm.read_permission(), false)?;
            let mut meta = ctx.meta()?;
            let workspace = sbm.transaction_with_workspace_setup(
                ctx,
                &mut meta,
                details,
                &[name.as_ref()],
                perm,
                beside.is_none(),
                |mut tx| {
                    match &beside {
                        Some((branch, below)) => sbm.create_stacked_reference(
                            &mut tx,
                            name.as_ref(),
                            branch.as_ref(),
                            position(*below),
                        )?,
                        // A stack's place is its tip's among the workspace commit's parents.
                        None => sbm.create_unstacked_reference(
                            &mut tx,
                            name.as_ref(),
                            first.then_some(0),
                        )?,
                    }
                    let to = RelativeTo::Reference(name.clone());
                    let ids = commits.iter().map(|commit| commit.0);
                    match copy {
                        true => tx
                            .cherry_pick_commits(ids.rev(), to, InsertSide::Below, false)
                            .map(drop),
                        false => tx.move_commits(ids, to, InsertSide::Below),
                    }
                },
            )?;
            Ok((rewritten(&workspace), name))
        })
    }
}

/// Give `commit` the title and body in `message`.
pub struct Reword {
    pub commit: CommitId,
    pub message: String,
}

impl Intent for Reword {
    type Done = Rewritten;

    fn apply(self, run: &mut Run) -> Result<Rewritten> {
        let Self { commit, message } = self;
        let (made, workspace) = run.locked(|ctx, perm| {
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::UpdateCommitMessage),
                &[],
                DryRun::No,
                |mut tx| {
                    let made = tx.reword_commit(commit.0, message.as_str().into())?;
                    Ok(but_transaction::Commit(CommitId(made.id)))
                },
            )
        })?;
        let mut rewritten = rewritten(&workspace);
        rewritten.insert(commit, made);
        Ok(rewritten)
    }
}

/// Drop `commits` and everything they changed.
pub struct DiscardCommits {
    pub commits: Arc<[CommitId]>,
}

impl Intent for DiscardCommits {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let Self { commits } = self;
        run.locked(|ctx, perm| {
            refuse_buried_merges(&*ctx.repo.get()?, &commits)?;
            let mut meta = ctx.meta()?;
            but_transaction::with_transaction_with_perm(
                ctx,
                &mut meta,
                perm,
                SnapshotDetails::new(OperationKind::DiscardCommit)
                    .with_count(commits.len())
                    .with_trailers(commits.iter().map(|id| Trailer::Sha(id.0))),
                &[],
                DryRun::No,
                |mut tx| tx.discard_commits(commits.iter().map(|id| id.0)),
            )?;
            Ok(())
        })
    }
}

/// Why a discard didn't throw away all it was asked to, when that isn't an error.
#[derive(Debug, PartialEq)]
pub enum Undiscarded {
    /// GitButler couldn't record it for undo, so nothing was thrown away.
    Unrecorded(String),
    /// It failed partway, after recording what undo brings back.
    Partway(String),
}

/// Throw away uncommitted `changes` of `checkout`. The main checkout's are recorded for undo
/// first. When that fails nothing is thrown away, for the user to decide whether to go on
/// without undo, unless `unrecorded` says they did.
pub struct DiscardChanges {
    pub changes: Arc<[Selected]>,
    pub checkout: Checkout,
    pub unrecorded: bool,
}

impl Intent for DiscardChanges {
    type Done = Option<Undiscarded>;

    fn apply(self, run: &mut Run) -> Result<Option<Undiscarded>> {
        let Self {
            changes,
            checkout,
            unrecorded,
        } = self;
        let worktree = checkout.linked();
        let project = format!("{}/", run.path().display());
        // One lock throughout, so what's conflicted can't change under the discard.
        run.locked(|ctx, perm| discard(ctx, perm, worktree, &changes, unrecorded, &project))
    }
}

fn discard(
    ctx: &but_ctx::Context,
    perm: &mut but_ctx::access::RepoExclusive,
    worktree: Option<&BStr>,
    selected: &[Selected],
    unrecorded: bool,
    project: &str,
) -> Result<Option<Undiscarded>> {
    let repo = &checkout(ctx, worktree)?;
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
    // The snapshot is recorded before anything changes: undo then brings back whatever
    // goes, conflicts included, even when some of it couldn't be
    // discarded. The oplog can't restore a linked worktree, so its discard has none.
    let mut recorded = false;
    if worktree.is_none() {
        let details = SnapshotDetails::new(OperationKind::DiscardChanges);
        let snapshot = but_oplog::UnmaterializedOplogSnapshot::prepare_checkpoint(
            ctx,
            details,
            &[],
            perm.read_permission(),
        )
        .and_then(|snapshot| snapshot.commit(ctx, perm));
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
            super::failure_detail(&err).replace(project, ""),
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

/// Take the conflicted uncommitted `paths` of `checkout` as they are on disk, markers or not.
pub struct MarkResolved {
    pub paths: Vec<bstr::BString>,
    pub checkout: Checkout,
}

impl Intent for MarkResolved {
    type Done = ();

    fn apply(self, run: &mut Run) -> Result<()> {
        let worktree = self.checkout.linked();
        // Undo can't restore a linked worktree, or a commit checked out for editing, so neither
        // is recorded.
        let unrecorded = worktree.is_some() || super::edit::editing(run.ctx())?.is_some();
        run.locked(|ctx, perm| match unrecorded {
            true => {
                but_workspace::resolve_worktree_conflicts(&checkout(ctx, worktree)?, self.paths)
            }
            false => {
                but_api::workspace::resolve_worktree_conflicts_with_perm(ctx, self.paths, perm)
            }
        })
    }

    fn kind(&self) -> OpKind {
        OpKind::MarkResolved
    }
}

#[cfg(test)]
pub(super) mod tests {
    use crate::backend::model::build::full;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;
    use crate::backend::{
        Checkout, DiffBody, DiffSource, FileDiff, Kept, Workspace, load_diff, now_millis, open,
    };

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

    fn newest_entry(path: &Path) -> crate::backend::SnapshotId {
        crate::backend::history::oplog_head(&open(path).unwrap()).unwrap()
    }

    fn undo(
        path: &Path,
        snapshot: crate::backend::SnapshotId,
        newer_than: Option<(i64, &[(i64, i64)])>,
    ) -> Result<crate::backend::history::Undone> {
        let newer_than = newer_than.map(|(at, spans)| (at, spans.to_vec()));
        let undo = crate::backend::history::Undo {
            snapshot,
            newer_than,
        };
        crate::backend::apply(path, undo)?.0
    }

    fn redo(
        path: &Path,
        undo: crate::backend::SnapshotId,
        spans: &[(i64, i64)],
    ) -> Result<Option<crate::backend::history::Undone>> {
        let redo = crate::backend::history::Redo {
            undo,
            kept: Default::default(),
            spans: spans.to_vec(),
            accepted: None,
        };
        Ok(crate::backend::apply(path, redo)?.0)
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

    fn commit_to_main(changes: Vec<Selected>, message: &str, run_hooks: bool) -> CommitChanges {
        CommitChanges {
            to: CommitTo::Branch(full("refs/heads/main")),
            changes: changes.into(),
            checkout: Checkout::Main,
            message: message.into(),
            run_hooks,
            leave_out: false,
            restack: None,
        }
    }

    impl CommitOutcome {
        /// The commit made, which every commit in these tests is.
        #[track_caller]
        fn committed(self) -> Committed {
            match self {
                CommitOutcome::Committed(committed) => committed,
                CommitOutcome::Blocked(blocked) => panic!("the commit was blocked: {blocked:?}"),
            }
        }
    }

    pub(in crate::backend) fn branch_named(name: &str) -> CreateBranch {
        CreateBranch {
            name: Some(name.into()),
        }
    }

    fn name_at(top: CommitId, name: &str) -> NameBranch {
        NameBranch {
            top,
            name: name.into(),
        }
    }

    fn amend_main(into: CommitId, changes: Vec<Selected>) -> Amend {
        Amend {
            into,
            changes: changes.into(),
            checkout: Checkout::Main,
            message: None,
            run_hooks: false,
        }
    }

    fn discard(
        path: &Path,
        checkout: Checkout,
        selected: &[Selected],
        unrecorded: bool,
    ) -> Result<Option<Undiscarded>> {
        let discard = DiscardChanges {
            changes: selected.into(),
            checkout,
            unrecorded,
        };
        Ok(crate::backend::apply(path, discard)?.0)
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
    fn a_commit_without_a_change_id_is_named_by_its_short_sha() {
        let dir = repo_with("one\n");
        let repo = gix::open(dir.path()).unwrap();
        let id = head(dir.path());
        assert_eq!(
            crate::backend::short_reference(&repo, id),
            id.short(),
            "plain Git's commit carries no change ID"
        );
    }

    #[test]
    fn a_commit_is_named_by_the_start_of_the_change_id_it_carries() {
        let dir = repo_with("one\n");
        let repo = gix::open(dir.path()).unwrap();
        let plain = repo.find_commit(head(dir.path()).0).unwrap();
        let mut carrying = plain.decode().unwrap().into_owned().unwrap();
        carrying.extra_headers.push((
            "change-id".into(),
            "vtlrvnlmsxrmrsnrkvqlwrzqokpsoxzk".into(),
        ));
        let id = repo.write_object(&carrying).unwrap().detach();
        assert_eq!(
            crate::backend::short_reference(&repo, CommitId(id)),
            "vtlrvnl",
            "the header it carries, not its sha"
        );
    }

    #[test]
    fn a_description_is_listed_and_follows_its_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::branches::{DescribeBranch, all_branches, move_description};
            let describe = |path, branch, description: &str| {
                crate::backend::apply(
                    path,
                    DescribeBranch {
                        branch: full(branch),
                        description: description.to_owned(),
                    },
                )
            };
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "topic"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Topic"]);
            let topic = head(path);
            git(path, &["checkout", "-q", "main"]);

            describe(path, "refs/heads/topic", "  Why it's there \n").unwrap();
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
            move_description(&repo, "topic".into(), "renamed".into()).unwrap();
            assert_eq!(
                (kept("topic"), kept("renamed").as_deref()),
                (None, Some("Why it's there")),
                "it moves to the new name, as `git branch -m` moves it"
            );
            move_description(&repo, "renamed".into(), "topic".into()).unwrap();
            let delete = DeleteBranchAndCommits {
                branch: full("refs/heads/topic"),
                tip: Some(topic),
            };
            crate::backend::apply(path, delete).unwrap();
            assert_eq!(
                kept("topic"),
                None,
                "and goes with the branch, as `git branch -d` drops it"
            );
            assert!(
                describe(path, "refs/heads/topic", "Lost?").is_err(),
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
            let refused = crate::backend::apply(path, amend_main(head, vec![whole])).unwrap_err();
            assert!(
                refused.to_string().contains("resolved first"),
                "the markers would go in: {refused:#}"
            );

            let resolved = MarkResolved {
                paths: vec!["file".into()],
                checkout: Checkout::Main,
            };
            crate::backend::apply(path, resolved).unwrap();
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
            let refused = discard(path, Checkout::Main, &[part], true).unwrap_err();
            assert!(
                refused.to_string().contains("has conflicts"),
                "part of it would leave the rest resolved, markers and all: {refused:#}"
            );
            let whole = Selected::whole(&files[0]);
            discard(path, Checkout::Main, &[whole], true).unwrap();
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
            let why = discard(path, Checkout::Main, std::slice::from_ref(&whole), false).unwrap();
            assert!(why.is_some(), "it says why it couldn't record the discard");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                "two\n",
                "and throws nothing away, as undo couldn't bring it back"
            );
            assert_eq!(discard(path, Checkout::Main, &[whole], true).unwrap(), None);
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

            let outcome = discard(path, Checkout::Main, &[Selected::whole(&gone)], false).unwrap();
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
            let refused = discard(path, Checkout::Main, &[line], false).unwrap_err();
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
                discard(path, Checkout::Main, std::slice::from_ref(&hunk), false).unwrap(),
                None
            );
            assert!(!path.join("new").exists(), "the whole file goes");
            discard(path, Checkout::Main, &[hunk], false)
                .expect("nothing left to discard is no error");
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
            let refused =
                discard(path, Checkout::Linked("agent".into()), &[whole], true).unwrap_err();
            assert!(
                refused
                    .to_string()
                    .contains("changed since the diff was shown"),
                "what was written after the diff was read isn't thrown away unseen, as nothing \
                 brings a linked worktree's discard back: {refused}"
            );
            discard(path, Checkout::Linked("agent".into()), &[read()], true).unwrap();
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

            let remove = || {
                let delete = crate::backend::worktrees::DeleteWorktree {
                    name: "agent".into(),
                    lost: asked.to_vec(),
                };
                crate::backend::apply(path, delete)
            };
            let err = remove().unwrap_err();
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
            let err = remove().unwrap_err();
            assert!(
                format!("{err:#}").contains("file"),
                "nor what was written to a file named since: {err:#}"
            );

            let moved = outside.path().join("moved");
            std::fs::rename(&linked, &moved).unwrap();
            let err = remove().unwrap_err();
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
            discard(path, Checkout::Linked("agent".into()), &selected, true).unwrap_err();
            assert_eq!(
                git(&linked, &["ls-files", "-u", "file"]).lines().count(),
                3,
                "with no snapshot to undo from, the conflict is still there to resolve"
            );
            let resolved = MarkResolved {
                paths: vec!["file".into()],
                checkout: Checkout::Linked("agent".into()),
            };
            crate::backend::apply(path, resolved).unwrap();
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
            crate::backend::apply(path, amend_main(head, vec![first_hunk])).unwrap();

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
    fn an_amend_with_a_message_rewrites_both_in_one_undo_step() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Work"]);
            std::fs::write(path.join("file"), "two\n").unwrap();
            let into = head(path);
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let amend = Amend {
                message: Some("Better\n\nWith a body".into()),
                ..amend_main(into, vec![Selected::whole(&uncommitted(path)[0])])
            };
            let (amended, ran) = crate::backend::apply(path, amend).unwrap();

            let made = head(path);
            assert_eq!(
                amended.rewritten.get(&into),
                Some(&made),
                "what it became is the commit at the head, after the message too"
            );
            assert_eq!(
                git(path, &["log", "-1", "--format=%B"]).trim(),
                "Better\n\nWith a body",
                "the commit has the new message"
            );
            assert_eq!(
                git(path, &["show", "HEAD:file"]),
                "two\n",
                "and the changes"
            );
            assert_eq!(
                entries_since(path, before).trim().lines().count(),
                1,
                "both are one entry, for one undo step"
            );

            undo(path, ran.recorded.unwrap(), None).unwrap();
            assert_eq!(
                git(path, &["log", "-1", "--format=%B"]).trim(),
                "Work",
                "undo brings the message back"
            );
            assert_eq!(
                uncommitted(path).len(),
                1,
                "and leaves the changes uncommitted"
            );
        })
    }

    #[test]
    fn an_amend_can_empty_the_message() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            std::fs::write(path.join("file"), "two\n").unwrap();
            let amend = Amend {
                message: Some(String::new()),
                ..amend_main(head(path), vec![Selected::whole(&uncommitted(path)[0])])
            };

            crate::backend::apply(path, amend).unwrap();

            assert_eq!(
                git(path, &["log", "-1", "--format=%B"]).trim(),
                "",
                "the box said nothing, as rewording to nothing does"
            );
            assert_eq!(
                git(path, &["show", "HEAD:file"]),
                "two\n",
                "and the changes went in"
            );
        })
    }

    #[test]
    fn an_amend_without_a_message_keeps_the_commits_own() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let message = git(path, &["log", "-1", "--format=%B"]);
            std::fs::write(path.join("file"), "two\n").unwrap();
            let changes = vec![Selected::whole(&uncommitted(path)[0])];

            crate::backend::apply(path, amend_main(head(path), changes)).unwrap();

            assert_eq!(
                git(path, &["log", "-1", "--format=%B"]),
                message,
                "the message isn't touched"
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
            let part = commit_to_main(vec![first_hunk.clone()], "part", false);
            let err = crate::backend::apply(path, part)
                .expect_err("a hunk of the converted copy isn't one of the file");
            assert!(err.to_string().contains("textconv"), "{err}");
            discard(path, Checkout::Main, &[first_hunk], false).expect_err("nor is one to discard");
            assert_eq!(
                std::fs::read_to_string(path.join("file")).unwrap(),
                lines.concat(),
                "the file is as it was"
            );
            git(path, &["commit", "-qam", "both edits"]);
            let commit = head(path);
            let diff = load_diff(path, DiffSource::Commit(commit), None, None, None, None).unwrap();
            let first_hunk = diff[0].part(Some(vec![diff[0].hunks()[0].key]));
            let uncommit = UncommitFiles {
                commit,
                files: [first_hunk].into(),
            };
            let err = crate::backend::apply(path, uncommit)
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
            let err = crate::backend::apply(path, amend_main(head, vec![first_hunk])).unwrap_err();
            assert!(
                err.to_string().contains("changed since the diff was shown"),
                "its hunks are numbered against a base that's gone: {err}"
            );
        })
    }

    #[test]
    fn an_amend_that_takes_nothing_records_nothing_to_undo() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            std::fs::write(path.join("file"), "two\n").unwrap();
            let changed = Selected::whole(&uncommitted(path)[0]);
            std::fs::write(path.join("file"), "one\n").unwrap();
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let err =
                crate::backend::apply(path, amend_main(head(path), vec![changed])).unwrap_err();
            assert!(
                err.to_string().contains("Nothing could be amended"),
                "the file changes nothing: {err}"
            );
            assert_eq!(entries_since(path, before), "", "the amend rolled back");
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
            let first = git(path, &["rev-parse", "HEAD"]);

            hook("pre-commit", "echo 'lint failed'; exit 1");
            let refused =
                crate::backend::apply(path, commit_to_main(changes(), "Two", true)).unwrap_err();
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
            let (committed, _) =
                crate::backend::apply(path, commit_to_main(changes(), "Two", true)).unwrap();
            let committed = committed.committed();
            assert_eq!(
                git(path, &["log", "-1", "--format=%s|%b"]).trim(),
                "Two|Issue: SHOP-12",
                "the commit takes the trailer commit-msg added, below the title"
            );
            assert!(uncommitted(path).is_empty(), "the commit stands");
            assert!(
                (committed.hook_failed).is_some_and(|output| output.contains("notify failed")),
                "post-commit's failure comes back to report"
            );
        })
    }

    #[test]
    fn a_commit_waits_for_a_git_merge_or_bisect_to_finish() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let first = git(path, &["rev-parse", "HEAD"]);
            for (state, says) in [("MERGE_HEAD", "git merge"), ("BISECT_LOG", "git bisect")] {
                std::fs::write(path.join(".git").join(state), format!("{first}\n")).unwrap();
                let refused =
                    crate::backend::apply(path, commit_to_main(vec![], "Two", false)).unwrap_err();
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
            let first = git(path, &["rev-parse", "HEAD"]);
            let (committed, _) =
                crate::backend::apply(path, commit_to_main(vec![], "Mark it reviewed", false))
                    .unwrap();
            let committed = committed.committed();
            assert_eq!(
                git(path, &["log", "-1", "--format=%s"]).trim(),
                "Mark it reviewed",
                "the commit is made"
            );
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            assert_eq!(
                (workspace.find_commit(committed.made.id)).map(|(_, c)| c.key.as_str()),
                Some(committed.made.key.as_str()),
                "a reload finds the commit by the change id it was made with"
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
    fn a_commit_to_a_new_branch_is_one_undo_step_wherever_the_branch_goes() {
        let mut said = String::new();
        for checked_out in ["gitbutler/workspace", "main", "feature"] {
            but_testsupport::isolated_app_data_dir(|| {
                let dir = repo_with("one\n");
                let path = dir.path();
                target_origin_main(path);
                if checked_out != "main" {
                    git(path, &["checkout", "-q", "-b", checked_out]);
                    git(path, &["commit", "-q", "--allow-empty", "-m", "work"]);
                }
                let head = |path: &Path| git(path, &["symbolic-ref", "--short", "HEAD"]);
                std::fs::write(path.join("file"), "two\n").unwrap();
                let before = crate::backend::history::oplog_head(&open(path).unwrap());

                let commit = CommitChanges {
                    to: CommitTo::NewBranchAt {
                        beside: None,
                        first: false,
                        name: None,
                    },
                    ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Two", false)
                };
                let (committed, ran) = crate::backend::apply(path, commit).unwrap();
                let committed = committed.committed();
                let new = short_ref(&committed.branch).into_owned();
                let entries = entries_since(path, before);
                let head_after = head(path).replace(&new, "<new>");
                let undone = undo(path, ran.recorded.unwrap(), None);
                let undone = match undone.unwrap() {
                    crate::backend::history::Undone::Done(operation) => operation,
                    _ => "nothing",
                };
                let gone = git(path, &["branch", "--list", &new]).is_empty();
                said += &format!(
                    "{checked_out}: {} on {}; undid {undone}: on {}, <new> gone {gone}, {} uncommitted\n",
                    entries.trim(),
                    head_after.trim(),
                    head(path).trim(),
                    uncommitted(path).len(),
                );
            });
        }
        // Each placement records one entry naming the new branch, checked out when made on top
        // of main, so one undo takes back the commit and the branch, leaving the change
        // uncommitted where it was.
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
gitbutler/workspace: CreateCommit on gitbutler/workspace; undid Created commit: on gitbutler/workspace, <new> gone true, 1 uncommitted
main: CreateCommit on <new>; undid Created commit: on main, <new> gone true, 1 uncommitted
feature: CreateCommit on gitbutler/workspace; undid Created commit: on feature, <new> gone true, 1 uncommitted

"#]]
        );
    }

    #[test]
    fn a_commit_beside_that_takes_nothing_leaves_no_branch_behind() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "work"]);
            // GitButler's own, as the target's, aside.
            let branches = |path: &Path| {
                (git(path, &["branch", "--list"]).lines())
                    .filter(|line| !line.contains("gitbutler/"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let before = branches(path);
            std::fs::write(path.join("file"), "two\n").unwrap();
            let changed = Selected::whole(&uncommitted(path)[0]);
            std::fs::write(path.join("file"), "one\n").unwrap();
            let last_entry = crate::backend::history::oplog_head(&open(path).unwrap());

            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: false,
                    name: None,
                },
                ..commit_to_main(vec![changed], "Two", false)
            };
            let (outcome, _) = crate::backend::apply(path, commit).unwrap();
            let CommitOutcome::Blocked(blocked) = outcome else {
                panic!("the file changes nothing: {outcome:?}");
            };
            assert_eq!(
                (blocked.changes.iter())
                    .map(|change| (change.path.as_str(), change.why))
                    .collect::<Vec<_>>(),
                [("file", "nothing in it would change")],
                "the user is told why"
            );
            assert_eq!(branches(path), before, "the branch started for it is gone");
            assert_eq!(
                git(path, &["symbolic-ref", "--short", "HEAD"]).trim(),
                "feature",
                "and feature is checked out on its own again"
            );
            assert_eq!(
                entries_since(path, last_entry),
                "",
                "a failed commit records nothing"
            );
        })
    }

    #[test]
    fn a_linked_worktree_commits_to_no_new_branch() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: false,
                    name: None,
                },
                checkout: Checkout::Linked("feature".into()),
                ..commit_to_main(vec![], "Two", false)
            };
            let err = crate::backend::apply(dir.path(), commit).unwrap_err();
            assert_eq!(
                err.to_string(),
                "The worktree has no branch of its own checked out",
                "a new branch would be the main workspace's, not the worktree's"
            );
        })
    }

    #[test]
    fn a_new_branch_from_the_default_branch_goes_on_top_of_it() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);

            let new = crate::backend::apply(path, branch_named("my feature"))
                .unwrap()
                .0;
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
    fn a_target_without_a_remote_is_its_own_branch_to_update_from_and_merge_into() {
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
                (target.name.as_str(), target.local, target.needs_update()),
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
            let merge = crate::backend::upstream::MergeIntoTarget {
                branch: full("refs/heads/feature"),
                tip,
                whole_stack: false,
            };
            crate::backend::apply(path, merge).unwrap();
            assert!(
                git(path, &["log", "--format=%s", "main"])
                    .lines()
                    .any(|s| s == "Feature"),
                "main moved to take the branch in, with no remote to push to"
            );
        })
    }

    #[test]
    fn a_new_branch_from_the_default_branch_refuses_a_name_thats_taken() {
        but_testsupport::isolated_app_data_dir(|| {
            // The default branch checked out at the target, as in a fresh clone.
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            // A teammate's branch on the remote, which a branch of that name would take as its
            // own, and a packed one a ref edit wouldn't notice under it.
            git(path, &["update-ref", "refs/remotes/origin/fix", "HEAD"]);
            git(path, &["branch", "foo"]);
            git(path, &["pack-refs", "--all"]);
            let refs = |path: &Path| git(path, &["for-each-ref", "--format=%(refname)"]);
            let before = refs(path);

            for name in ["fix", "foo/bar"] {
                let made = crate::backend::apply(path, branch_named(name));
                assert!(made.is_err(), "{name} is taken");
                assert_eq!(refs(path), before, "nothing is made for {name}");
                assert_eq!(
                    git(path, &["symbolic-ref", "--short", "HEAD"]).trim(),
                    "main",
                    "main stays checked out"
                );
            }
        })
    }

    #[test]
    fn without_a_target_branches_stack_on_the_checked_out_one_and_move_within_it() {
        but_testsupport::isolated_app_data_dir(|| {
            // `top` checked out on its own, stacked on `lower`, in a project with no remote to
            // take a target from.
            let dir = repo_with("one\n");
            let path = dir.path();
            git(path, &["checkout", "-q", "-b", "lower"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Lower"]);
            git(path, &["checkout", "-q", "-b", "top"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Top"]);
            let stack = |path: &Path| {
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                assert!(workspace.target_branch.is_none(), "there's no target");
                let names: Vec<String> = (workspace.branches_in(&Checkout::Main))
                    .map(|branch| branch.name.replace("t-branch-1", "<new>"))
                    .collect();
                let head = git(path, &["symbolic-ref", "--short", "HEAD"]);
                format!(
                    "{} on {}",
                    names.join(", "),
                    head.trim().replace("t-branch-1", "<new>")
                )
            };

            crate::backend::apply(path, CreateBranch { name: None }).unwrap();
            let made = stack(path);
            let lower = full("refs/heads/lower");
            let onto = full("refs/heads/top");
            crate::backend::apply(
                path,
                StackBranch {
                    branch: lower,
                    onto,
                },
            )
            .unwrap();
            // Top first: a new branch goes on top of the one checked out and is checked out, as
            // `git checkout -b` would, and a branch moves within the stack, though with no target
            // the oplog has nothing to record a checkpoint against.
            snapbox::assert_data_eq!(
                format!("{made}\n{}\n", stack(path)),
                snapbox::str![[r#"
<new>, top, lower, main on <new>
<new>, lower, top, main on <new>

"#]]
            );
        })
    }

    #[test]
    fn without_a_target_a_branch_cant_be_torn_off() {
        but_testsupport::isolated_app_data_dir(|| {
            // `top` checked out on its own, stacked on `lower`, in a project with no remote to
            // take a target from, which a stack of its own would start from.
            let dir = repo_with("one\n");
            let path = dir.path();
            git(path, &["checkout", "-q", "-b", "lower"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Lower"]);
            git(path, &["checkout", "-q", "-b", "top"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "Top"]);
            let before = git(path, &["log", "--oneline", "--all"]);

            let unstack = TearOffBranch {
                branch: full("refs/heads/lower"),
                first: false,
            };
            let err = crate::backend::apply(path, unstack).unwrap_err();
            assert!(
                err.to_string().contains("cutoff"),
                "the UI offers no tear-off without a target, as this refuses: {err}"
            );
            assert_eq!(
                git(path, &["log", "--oneline", "--all"]),
                before,
                "the stack is left as it was"
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
            let err = crate::backend::apply(path, CreateBranch { name: None }).unwrap_err();
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
            let taken = crate::backend::apply(path, branch_named("fix"));
            assert!(
                taken.is_err() && !lanes(path).0,
                "fix is taken, so nothing changes"
            );
            let new = crate::backend::apply(path, branch_named("second"))
                .unwrap()
                .0;
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

            let last = newest_entry(path);
            undo(path, last, None).unwrap();
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

            let new = crate::backend::apply(path, CreateBranch { name: None })
                .unwrap()
                .0;
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

            let new = crate::backend::apply(path, CreateBranch { name: None })
                .unwrap()
                .0;
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
            let message = Some("Before".into());
            crate::backend::apply(path, crate::backend::history::TakeSnapshot { message }).unwrap();

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
            let take_snapshot = || {
                let snapshot = crate::backend::history::TakeSnapshot { message: None };
                crate::backend::apply(path, snapshot).unwrap();
            };
            commit("Lower");
            // A branch stacked under the checked-out one.
            git(path, &["branch", "lower"]);
            commit("First");
            take_snapshot();
            commit("Second");
            take_snapshot();
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

            let reword = Reword {
                commit: top,
                message: "reworded".into(),
            };
            crate::backend::apply(path, reword).unwrap();
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
            use crate::backend::history::Undone;
            let (dir, commit, ..) = workspace_with_an_edit();
            let path = dir.path();
            let moving = MoveToNewBranch {
                commits: [commit].into(),
                beside: None,
                first: false,
                copy: false,
            };
            let ((_, name), _) = crate::backend::apply(path, moving).unwrap();
            let moved = newest_entry(path);
            assert!(matches!(undo(path, moved, None).unwrap(), Undone::Done(_)));
            let left = git(path, &["branch", "--list", &short_ref(&name)]);
            assert_eq!(
                left, "",
                "{name} would be listed beside the commit back in place"
            );
        })
    }

    /// A workspace whose branch feature has commits "edit" and "second" on top of it, with a
    /// new file uncommitted, and second.
    fn feature_with_a_new_file() -> (tempfile::TempDir, CommitId) {
        let (dir, ..) = workspace_with_an_edit();
        let path = dir.path();
        git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
        let second = head(path);
        crate::backend::apply(path, name_at(second, "feature")).unwrap();
        std::fs::write(path.join("new"), "new\n").unwrap();
        (dir, second)
    }

    /// The titles of `branch`'s commits newest first, down to the target.
    fn titles_on(path: &Path, branch: &str) -> String {
        let range = format!("origin/main..{branch}");
        let log = git(path, &["log", "--format=%s", &range]);
        log.lines().collect::<Vec<_>>().join(", ")
    }

    #[test]
    fn a_commit_below_another_goes_right_under_it() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, second) = feature_with_a_new_file();
            let path = dir.path();
            let commit = CommitChanges {
                to: CommitTo::BelowCommit {
                    commit: second,
                    branch: full("refs/heads/feature"),
                },
                ..commit_to_main(
                    vec![Selected::whole(&uncommitted(path)[0])],
                    "Between",
                    false,
                )
            };
            let (committed, _) = crate::backend::apply(path, commit).unwrap();
            let committed = committed.committed();
            assert_eq!(
                titles_on(path, "feature"),
                "second, Between, edit",
                "it goes between second and the commit under it"
            );
            assert_eq!(
                committed.branch,
                full("refs/heads/feature"),
                "on the branch that has second"
            );
            assert!(uncommitted(path).is_empty(), "the new file went in");
        })
    }

    /// A workspace whose branch feature edits `file`, with that edit edited again uncommitted,
    /// which builds on it, and a new file, which doesn't.
    fn an_edit_building_on_feature() -> tempfile::TempDir {
        let (dir, edit, _, mut edited) = workspace_with_an_edit();
        let path = dir.path();
        crate::backend::apply(path, name_at(edit, "feature")).unwrap();
        edited[4] = "five c\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        std::fs::write(path.join("new"), "new\n").unwrap();
        dir
    }

    /// A commit of all of `path`'s uncommitted files to a new stack of its own.
    fn commit_all_to_a_new_stack(path: &Path) -> CommitChanges {
        let files = uncommitted(path).iter().map(Selected::whole).collect();
        CommitChanges {
            to: CommitTo::NewBranchAt {
                beside: None,
                first: false,
                name: Some("apart".into()),
            },
            ..commit_to_main(files, "Apart", false)
        }
    }

    #[test]
    fn a_commit_some_of_whose_changes_build_on_another_branch_isnt_made() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = an_edit_building_on_feature();
            let path = dir.path();
            let before = crate::backend::history::oplog_head(&open(path).unwrap());
            let (outcome, _) =
                crate::backend::apply(path, commit_all_to_a_new_stack(path)).unwrap();
            let CommitOutcome::Blocked(blocked) = outcome else {
                panic!("the edit of feature's edit can't go in a stack of its own: {outcome:?}");
            };
            assert_eq!(
                blocked.builds_on,
                Some(full("refs/heads/feature")),
                "everything left out can go on feature"
            );
            let left_out: Vec<_> = (blocked.changes.iter())
                .map(|change| (change.path.as_str(), change.builds_on.clone()))
                .collect();
            assert_eq!(
                left_out,
                [("file", vec![full("refs/heads/feature")])],
                "only the edit is left out, as it builds on feature's commit"
            );
            let hunks: Vec<_> = (blocked.changes.iter())
                .flat_map(|change| &change.hunks)
                .map(|hunk| {
                    let on: Vec<_> = (hunk.builds_on.iter())
                        .map(|commit| (commit.title.as_str(), commit.branch.clone()))
                        .collect();
                    let lines: Vec<_> = (hunk.lines.iter())
                        .map(|line| (line.kind, line.old, line.new, &*line.text))
                        .collect();
                    ((hunk.new_start, hunk.new_lines), lines, on)
                })
                .collect();
            use crate::backend::LineKind::{Added, Removed};
            assert_eq!(
                hunks,
                [(
                    (5, 1),
                    vec![
                        (Removed, Some(5), None, "five a"),
                        (Removed, Some(6), None, "five b"),
                        (Added, None, Some(5), "five c"),
                    ],
                    vec![("edit", Some(full("refs/heads/feature")))]
                )],
                "the user sees each line numbered as the diff does, and what it builds on"
            );
            assert!(
                blocked.takes_others,
                "the new file could still be committed"
            );
            assert_eq!(
                git(path, &["branch", "--list", "apart"]),
                "",
                "nothing was committed, so no branch was made for it"
            );
            assert_eq!(
                uncommitted(path).len(),
                2,
                "both files are still uncommitted"
            );
            assert_eq!(
                crate::backend::history::oplog_head(&open(path).unwrap()),
                before,
                "nothing was recorded to undo"
            );
        })
    }

    /// A workspace whose branch alpha edits line 5 and beta, a stack of its own, line 15, with
    /// both lines edited again uncommitted, so the change builds on both.
    fn an_edit_building_on_alpha_and_beta() -> tempfile::TempDir {
        let (dir, edit, _, mut edited) = workspace_with_an_edit();
        let path = dir.path();
        crate::backend::apply(path, name_at(edit, "alpha")).unwrap();
        edited[14] = "fifteen\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        let beta = CommitChanges {
            to: CommitTo::NewBranchAt {
                beside: None,
                first: false,
                name: Some("beta".into()),
            },
            ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Beta", false)
        };
        crate::backend::apply(path, beta).unwrap().0.committed();
        edited[4] = "five c\n".into();
        edited[14] = "fifteen again\n".into();
        std::fs::write(path.join("file"), edited.concat()).unwrap();
        dir
    }

    #[test]
    fn a_commit_building_on_three_unstacked_branches_stacks_them_all() {
        but_testsupport::isolated_app_data_dir(|| {
            // alpha edits line 5, beta line 12 and gamma line 18, each a stack of its own.
            let (dir, edit, _, mut edited) = workspace_with_an_edit();
            let path = dir.path();
            crate::backend::apply(path, name_at(edit, "alpha")).unwrap();
            for (line, name) in [(11, "beta"), (17, "gamma")] {
                edited[line] = format!("{name}\n");
                std::fs::write(path.join("file"), edited.concat()).unwrap();
                let commit = CommitChanges {
                    to: CommitTo::NewBranchAt {
                        beside: None,
                        first: false,
                        name: Some(name.into()),
                    },
                    ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], name, false)
                };
                crate::backend::apply(path, commit).unwrap().0.committed();
            }
            edited[4] = "five c\n".into();
            edited[11] = "beta again\n".into();
            edited[17] = "gamma again\n".into();
            std::fs::write(path.join("file"), edited.concat()).unwrap();
            let all = || commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "All", false);
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let new_stack = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: false,
                    name: None,
                },
                ..all()
            };
            let (outcome, _) = crate::backend::apply(path, new_stack).unwrap();
            let CommitOutcome::Blocked(blocked) = outcome else {
                panic!("no one branch takes edits of three: {outcome:?}");
            };
            let stacking = blocked.stack.expect("the three stack cleanly");
            assert_eq!(
                stacking.branches.len(),
                2,
                "two stacks move onto the third: {stacking:?}"
            );

            let stacked = CommitChanges {
                to: CommitTo::Branch(stacking.top.clone()),
                restack: Some(stacking.clone()),
                ..all()
            };
            let (outcome, _) = crate::backend::apply(path, stacked).unwrap();
            assert_eq!(outcome.committed().branch, stacking.top);
            let titles = titles_on(path, &stacking.top.shorten().to_string());
            assert!(
                titles.starts_with("All, ")
                    && ["edit", "beta", "gamma"].iter().all(|t| titles.contains(t)),
                "the top branch takes it, over all three branches' commits: {titles}"
            );
            assert!(uncommitted(path).is_empty(), "it took every change");
            assert_eq!(
                entries_since(path, before),
                "CreateCommit\n",
                "stacking all three and committing are one undo step"
            );
        })
    }

    #[test]
    fn a_merged_branch_is_only_ever_stacked_on() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = an_edit_building_on_alpha_and_beta();
            let path = dir.path();
            // Squash-merged: the target has alpha's changes in a commit of its own.
            let squash = git(
                path,
                &[
                    "commit-tree",
                    "alpha^{tree}",
                    "-p",
                    "refs/remotes/origin/main",
                    "-m",
                    "Squash of alpha",
                ],
            );
            git(
                path,
                &["update-ref", "refs/remotes/origin/main", squash.trim()],
            );
            let alpha = git(path, &["rev-parse", "alpha"]);

            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: false,
                    name: None,
                },
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Both", false)
            };
            let (outcome, _) = crate::backend::apply(path, commit).unwrap();
            let CommitOutcome::Blocked(blocked) = outcome else {
                panic!("a new branch can take neither edit: {outcome:?}");
            };
            let stacking = blocked.stack.expect("beta can go on alpha");
            assert_eq!(
                (stacking.branches.clone(), stacking.onto.clone()),
                (vec![full("refs/heads/beta")], full("refs/heads/alpha")),
                "beta goes on merged alpha, which never moves"
            );

            let stacked = CommitChanges {
                to: CommitTo::Branch(stacking.top.clone()),
                restack: Some(stacking),
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Both", false)
            };
            crate::backend::apply(path, stacked).unwrap().0.committed();
            assert_eq!(
                git(path, &["rev-parse", "alpha"]),
                alpha,
                "alpha's commits are as they were"
            );
            assert!(uncommitted(path).is_empty(), "beta took every change");
        })
    }

    #[test]
    fn a_commit_building_on_two_unstacked_branches_is_offered_stacking_them() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = an_edit_building_on_alpha_and_beta();
            let path = dir.path();
            let both = || CommitChanges {
                to: CommitTo::Branch(full("refs/heads/alpha")),
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Both", false)
            };
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let (outcome, _) = crate::backend::apply(path, both()).unwrap();
            let CommitOutcome::Blocked(blocked) = outcome else {
                panic!("alpha alone can't take the edit of beta's line: {outcome:?}");
            };
            assert_eq!(blocked.builds_on, None, "no one branch takes it all");
            let stacking = blocked.stack.expect("the two branches stack cleanly");
            let mut pair = [stacking.top.clone(), stacking.onto.clone()];
            pair.sort();
            assert_eq!(
                pair,
                [full("refs/heads/alpha"), full("refs/heads/beta")],
                "one of them goes on the other"
            );
            assert!(stacking.pushed.is_empty(), "nothing is pushed");
            assert_eq!(
                entries_since(path, before),
                "",
                "trying it out changed nothing"
            );
            assert_eq!(titles_on(path, "beta"), "Beta", "beta is as it was");

            let stacked = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: Some((stacking.top.clone(), false)),
                    first: false,
                    name: Some("both".into()),
                },
                restack: Some(stacking.clone()),
                ..both()
            };
            let (outcome, _) = crate::backend::apply(path, stacked).unwrap();
            outcome.committed();
            let titles = titles_on(path, "both");
            assert!(
                titles.starts_with("Both, ") && titles.contains("Beta") && titles.contains("edit"),
                "the new branch sits on both branches' commits: {titles}"
            );
            assert!(uncommitted(path).is_empty(), "it took every change");
            assert_eq!(
                entries_since(path, before),
                "CreateCommit\n",
                "stacking and committing are one undo step"
            );
            let step = newest_entry(path);
            let undone = undo(path, step, None).unwrap();
            assert!(
                matches!(undone, crate::backend::history::Undone::Done(_)),
                "nothing came after it to stop the undo"
            );
            assert_eq!(
                (titles_on(path, "alpha"), titles_on(path, "beta")),
                ("edit".to_owned(), "Beta".to_owned()),
                "undo leaves each branch on its own again"
            );
            assert_eq!(uncommitted(path).len(), 1, "with the change uncommitted");

            // Or on the branch stacked, which is on top once it's stacked.
            let on_top = CommitChanges {
                to: CommitTo::Branch(stacking.top.clone()),
                restack: Some(stacking.clone()),
                ..both()
            };
            let (outcome, _) = crate::backend::apply(path, on_top).unwrap();
            assert_eq!(outcome.committed().branch, stacking.top);
            let titles = titles_on(path, &stacking.top.shorten().to_string());
            assert!(
                titles.starts_with("Both, ") && titles.contains("Beta") && titles.contains("edit"),
                "the branch stacked takes it, over both branches' commits: {titles}"
            );
            assert!(uncommitted(path).is_empty(), "it took every change");
        })
    }

    #[test]
    fn a_blocked_commit_goes_where_its_changes_build_on_or_without_them() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = an_edit_building_on_feature();
            let path = dir.path();
            let without = CommitChanges {
                leave_out: true,
                ..commit_all_to_a_new_stack(path)
            };
            let (outcome, _) = crate::backend::apply(path, without).unwrap();
            let committed = outcome.committed();
            assert_eq!(
                titles_on(path, "apart"),
                "Apart",
                "the new file went in a stack"
            );
            assert_eq!(
                committed.left_out,
                [
                    "file: it builds on changes in a later commit or another branch; put it \
                     where those are, or move them first"
                ],
                "what was left out, as the user agreed, is said"
            );

            let to_feature = CommitChanges {
                to: CommitTo::Branch(full("refs/heads/feature")),
                ..commit_all_to_a_new_stack(path)
            };
            let (outcome, _) = crate::backend::apply(path, to_feature).unwrap();
            outcome.committed();
            assert_eq!(
                titles_on(path, "feature"),
                "Apart, edit",
                "the edit went on the commit it builds on"
            );
            assert!(uncommitted(path).is_empty(), "nothing was left out");
        })
    }

    #[test]
    fn a_commit_to_a_new_branch_above_another_stacks_it_on_top() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _) = feature_with_a_new_file();
            let path = dir.path();
            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: Some((full("refs/heads/feature"), false)),
                    first: false,
                    name: None,
                },
                ..commit_to_main(
                    vec![Selected::whole(&uncommitted(path)[0])],
                    "On top",
                    false,
                )
            };
            let (committed, _) = crate::backend::apply(path, commit).unwrap();
            let committed = committed.committed();
            assert_eq!(
                titles_on(path, &committed.branch.to_string()),
                "On top, second, edit",
                "the new branch holds its commit over feature's"
            );
            assert_eq!(
                titles_on(path, "feature"),
                "second, edit",
                "feature is as it was"
            );
        })
    }

    #[test]
    fn a_commit_to_a_new_branch_below_another_stacks_it_underneath() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _) = feature_with_a_new_file();
            let path = dir.path();
            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: Some((full("refs/heads/feature"), true)),
                    first: false,
                    name: None,
                },
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Under", false)
            };
            let (committed, _) = crate::backend::apply(path, commit).unwrap();
            let committed = committed.committed();
            assert_eq!(
                titles_on(path, &committed.branch.to_string()),
                "Under",
                "the new branch holds only its commit"
            );
            assert_eq!(
                titles_on(path, "feature"),
                "second, edit, Under",
                "feature sits on top of it"
            );
        })
    }

    #[test]
    fn a_commit_to_a_new_branch_takes_the_name_given() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _) = feature_with_a_new_file();
            let path = dir.path();
            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: false,
                    name: Some("my feature".into()),
                },
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Named", false)
            };
            let (committed, _) = crate::backend::apply(path, commit).unwrap();
            let committed = committed.committed();
            assert_eq!(
                committed.branch,
                full("refs/heads/my-feature"),
                "named as typed, as Git allows"
            );
            assert_eq!(titles_on(path, "my-feature"), "Named", "with its commit");
        })
    }

    #[test]
    fn a_commit_to_a_new_stack_first_goes_before_the_others() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _) = feature_with_a_new_file();
            let path = dir.path();
            let commit = CommitChanges {
                to: CommitTo::NewBranchAt {
                    beside: None,
                    first: true,
                    name: None,
                },
                ..commit_to_main(vec![Selected::whole(&uncommitted(path)[0])], "Apart", false)
            };
            let (committed, _) = crate::backend::apply(path, commit).unwrap();
            let committed = committed.committed();
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            let stacks: Vec<String> = (workspace.lanes_in(&Checkout::Main))
                .filter_map(|lane| lane.branches.first()?.ref_name.as_ref())
                .map(|name| match *name == committed.branch {
                    true => "<new>".to_owned(),
                    false => short_ref(name).into_owned(),
                })
                .collect();
            assert_eq!(
                stacks,
                ["<new>", "feature"],
                "the stacks top first, as the navigator lists them"
            );
            assert_eq!(
                titles_on(path, &committed.branch.to_string()),
                "Apart",
                "its stack holds only its commit"
            );
        })
    }

    /// A repository whose linked worktree "agent", in a folder of its own, has its branch agent
    /// with commits "one" and "two", and a new file uncommitted: the repository, the worktree's
    /// folder, which goes with it, and the worktree's path.
    fn agent_with_a_new_file() -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
        let dir = repo_with("one\n");
        let path = dir.path();
        target_origin_main(path);
        let ctx = open(path).unwrap();
        (ctx.db.get_cache_mut().unwrap())
            .worktree_meta_mut()
            .mark_adopted()
            .unwrap();
        drop(ctx);
        let outside = tempfile::tempdir().unwrap();
        let at = outside.path().join("agent");
        let at_str = at.to_str().unwrap();
        git(
            path,
            &["worktree", "add", "-q", "-b", "agent", at_str, "main"],
        );
        for (message, file) in [("one", "a"), ("two", "b")] {
            std::fs::write(at.join(file), message).unwrap();
            git(&at, &["add", file]);
            git(&at, &["commit", "-q", "-m", message]);
        }
        std::fs::write(at.join("new"), "new\n").unwrap();
        (dir, outside, at)
    }

    /// Commit agent's uncommitted file `to` there, with `message`.
    fn commit_agents(path: &Path, to: CommitTo, message: &str) -> Committed {
        let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
        let agent = Checkout::Linked("agent".into());
        let files = workspace.uncommitted_in(&agent).unwrap();
        let commit = CommitChanges {
            to,
            changes: Arc::from([Selected::whole(&files[0])]),
            checkout: agent,
            message: message.into(),
            run_hooks: false,
            leave_out: false,
            restack: None,
        };
        crate::backend::apply(path, commit).unwrap().0.committed()
    }

    #[test]
    fn a_linked_worktrees_branches_are_made_with_the_main_checkout_detached() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _outside, at) = agent_with_a_new_file();
            let path = dir.path();
            // All the work is in the linked worktree, with no branch checked out in the main one.
            git(path, &["checkout", "-q", "--detach"]);
            let agent = full("refs/heads/agent");

            let to = CommitTo::NewBranchAt {
                beside: Some((agent.clone(), false)),
                first: false,
                name: None,
            };
            let committed = commit_agents(path, to, "On top");
            let on_top = short_ref(&committed.branch).into_owned();
            let at_one = CreateBranchAtCommit {
                commit: CommitId(
                    gix::ObjectId::from_hex(git(&at, &["rev-parse", "agent~"]).trim().as_bytes())
                        .unwrap(),
                ),
                below: false,
            };
            let at_one = crate::backend::apply(path, at_one).unwrap().0;
            assert_eq!(
                (
                    titles_on(path, &on_top),
                    titles_on(path, &at_one.to_string())
                ),
                ("On top, two, one".to_owned(), "one".to_owned()),
                "each is made as with the main checkout on a branch"
            );
            let above = crate::backend::apply(path, CreateDependentBranch { below: agent })
                .unwrap()
                .0;
            assert_eq!(
                titles_on(path, &above.to_string()),
                "two, one",
                "and stacked on agent"
            );
            assert_eq!(
                git(path, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
                "HEAD",
                "the main checkout stays detached"
            );
        })
    }

    #[test]
    fn a_linked_worktrees_commit_below_another_goes_right_under_it() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _outside, at) = agent_with_a_new_file();
            let path = dir.path();
            let two = head(&at);
            let to = CommitTo::BelowCommit {
                commit: two,
                branch: full("refs/heads/agent"),
            };
            commit_agents(path, to, "Between");
            assert_eq!(
                titles_on(path, "agent"),
                "two, Between, one",
                "it goes between two and the commit under it"
            );
            assert_eq!(
                git(&at, &["symbolic-ref", "--short", "HEAD"]).trim(),
                "agent",
                "the worktree stays on its branch"
            );
            assert_eq!(
                git(&at, &["status", "--porcelain"]),
                "",
                "and its files are as its branch has them now"
            );
        })
    }

    #[test]
    fn a_linked_worktrees_commit_to_a_new_branch_above_its_own_checks_that_out() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _outside, at) = agent_with_a_new_file();
            let path = dir.path();
            let to = CommitTo::NewBranchAt {
                beside: Some((full("refs/heads/agent"), false)),
                first: false,
                name: None,
            };
            let committed = commit_agents(path, to, "On top");
            let new = short_ref(&committed.branch).into_owned();
            assert_eq!(
                titles_on(path, &new),
                "On top, two, one",
                "the new branch holds its commit over agent's"
            );
            assert_eq!(titles_on(path, "agent"), "two, one", "agent is as it was");
            assert_eq!(
                git(&at, &["symbolic-ref", "--short", "HEAD"]).trim(),
                new,
                "the worktree goes on to the new branch, which has its change"
            );
            assert_eq!(
                git(&at, &["status", "--porcelain"]),
                "",
                "and its files are as the new branch has them"
            );
        })
    }

    #[test]
    fn a_linked_worktrees_commit_to_a_new_branch_below_its_own_stacks_it_underneath() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, _outside, at) = agent_with_a_new_file();
            let path = dir.path();
            let to = CommitTo::NewBranchAt {
                beside: Some((full("refs/heads/agent"), true)),
                first: false,
                name: None,
            };
            let committed = commit_agents(path, to, "Under");
            let new = short_ref(&committed.branch).into_owned();
            assert_eq!(
                titles_on(path, &new),
                "Under",
                "the new branch holds only its commit"
            );
            assert_eq!(
                titles_on(path, "agent"),
                "two, one, Under",
                "agent sits on top of it"
            );
            assert_eq!(
                git(&at, &["symbolic-ref", "--short", "HEAD"]).trim(),
                "agent",
                "the worktree stays on its branch, which has the change under it"
            );
            assert_eq!(
                git(&at, &["status", "--porcelain"]),
                "",
                "and its files are as its branch has them now"
            );
        })
    }

    #[test]
    fn a_move_to_a_new_branch_beside_another_stacks_it_above_or_below() {
        let mut said = String::new();
        for below in [false, true] {
            but_testsupport::isolated_app_data_dir(|| {
                let (dir, edit, ..) = workspace_with_an_edit();
                let path = dir.path();
                git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
                let second = head(path);
                crate::backend::apply(path, name_at(second, "feature")).unwrap();
                // The commit at the far end from where the new branch goes, so it crosses
                // feature's other commit to get there.
                let moved = match below {
                    true => second,
                    false => edit,
                };
                let moving = MoveToNewBranch {
                    commits: [moved].into(),
                    beside: Some((full("refs/heads/feature"), below)),
                    first: false,
                    copy: false,
                };
                let ((_, new), _) = crate::backend::apply(path, moving).unwrap();
                let titles = |branch: &str| {
                    let range = format!("origin/main..{branch}");
                    let log = git(path, &["log", "--format=%s", &range]);
                    log.lines().collect::<Vec<_>>().join(", ")
                };
                said += &format!(
                    "below {below}: feature [{}], <new> [{}]\n",
                    titles("feature"),
                    titles(&new.to_string()),
                );
            });
        }
        // Each branch lists its commits newest first, down to the target. Above feature, the
        // new branch holds feature's commits under its own; below, feature holds the new one's.
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
below false: feature [second], <new> [edit, second]
below true: feature [edit, second], <new> [second]

"#]]
        );
    }

    #[test]
    fn a_cherry_pick_to_a_new_stack_copies_the_commits_and_leaves_them_where_they_are() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);
            crate::backend::apply(path, name_at(second, "feature")).unwrap();
            let copying = MoveToNewBranch {
                commits: [second, edit].into(),
                beside: None,
                first: false,
                copy: true,
            };
            let ((_, new), _) = crate::backend::apply(path, copying).unwrap();
            let said = format!(
                "feature [{}], <new> [{}]",
                titles_on(path, "feature"),
                titles_on(path, &new.to_string()),
            );
            // Each branch lists its commits newest first, down to the target: the copies keep
            // the order the originals have, which stay on feature.
            snapbox::assert_data_eq!(
                said,
                snapbox::str!["feature [second, edit], <new> [second, edit]"]
            );
        })
    }

    #[test]
    fn a_cherry_pick_says_which_copy_is_newest() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "third"]);
            let third = head(path);
            crate::backend::apply(path, name_at(third, "feature")).unwrap();
            let picking = CherryPick {
                commits: [third, second].into(),
                to: Destination::AboveCommit(edit),
            };
            let ((_, newest), _) = crate::backend::apply(path, picking).unwrap();
            let newest = newest.expect("two commits were copied");
            let title = git(path, &["log", "-1", "--format=%s", &newest.0.to_string()]);
            assert_eq!(
                title.trim(),
                "third",
                "the newer of the two is copied last, on top"
            );
            assert_ne!(newest, third, "it's a copy, not the original");
        })
    }

    #[test]
    fn a_move_to_a_new_stack_puts_it_after_the_others_or_first() {
        let mut said = String::new();
        for first in [false, true] {
            but_testsupport::isolated_app_data_dir(|| {
                let (dir, ..) = workspace_with_an_edit();
                let path = dir.path();
                git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
                let second = head(path);
                crate::backend::apply(path, name_at(second, "feature")).unwrap();
                let moving = MoveToNewBranch {
                    commits: [second].into(),
                    beside: None,
                    first,
                    copy: false,
                };
                let ((_, new), _) = crate::backend::apply(path, moving).unwrap();
                let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
                let stacks: Vec<String> = (workspace.lanes_in(&Checkout::Main))
                    .filter_map(|lane| lane.branches.first()?.ref_name.as_ref())
                    .map(|name| match *name == new {
                        true => "<new>".to_owned(),
                        false => short_ref(name).into_owned(),
                    })
                    .collect();
                said += &format!("first {first}: {}\n", stacks.join(", "));
            });
        }
        // The stacks top first, as the navigator lists them.
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
first false: feature, <new>
first true: <new>, feature

"#]]
        );
    }

    #[test]
    fn a_move_to_a_new_branch_beside_a_linked_worktrees_stacks_it_in_its_lane() {
        let mut said = String::new();
        for managed in [false, true] {
            for below in [false, true] {
                but_testsupport::isolated_app_data_dir(|| {
                    let dir = repo_with("one\n");
                    let path = dir.path();
                    target_origin_main(path);
                    if managed {
                        crate::backend::apply(path, branch_named("beside")).unwrap();
                    }
                    let ctx = open(path).unwrap();
                    (ctx.db.get_cache_mut().unwrap())
                        .worktree_meta_mut()
                        .mark_adopted()
                        .unwrap();
                    drop(ctx);
                    let outside = tempfile::tempdir().unwrap();
                    let at = outside.path().join("agent");
                    let at_str = at.to_str().unwrap();
                    git(
                        path,
                        &["worktree", "add", "-q", "-b", "agent", at_str, "main"],
                    );
                    for (message, file) in [("one", "a"), ("two", "b")] {
                        std::fs::write(at.join(file), message).unwrap();
                        git(&at, &["add", file]);
                        git(&at, &["commit", "-q", "-m", message]);
                    }
                    // The commit at the far end from where the new branch goes, so it crosses
                    // agent's other commit to get there.
                    let moved = match below {
                        true => "agent",
                        false => "agent~",
                    };
                    let moved = git(path, &["rev-parse", moved]);
                    let moved = CommitId(gix::ObjectId::from_hex(moved.trim().as_bytes()).unwrap());
                    let moving = MoveToNewBranch {
                        commits: [moved].into(),
                        beside: Some((full("refs/heads/agent"), below)),
                        first: false,
                        copy: false,
                    };
                    let ((_, new), _) = crate::backend::apply(path, moving).unwrap();
                    let workspace =
                        crate::backend::load_workspace(path, Default::default()).unwrap();
                    let lane = workspace.worktree_lane("agent".into()).unwrap();
                    let branches: Vec<String> = (lane.branches.iter())
                        .map(|branch| {
                            let name = match branch.ref_name.as_ref() == Some(&new) {
                                true => "<new>".to_owned(),
                                false => branch.name.clone(),
                            };
                            let titles: Vec<_> =
                                branch.commits.iter().map(|c| c.title.as_str()).collect();
                            format!("{name} [{}]", titles.join(", "))
                        })
                        .collect();
                    let checked_out = git(&at, &["symbolic-ref", "--short", "HEAD"]);
                    let checked_out = match checked_out.trim() == short_ref(&new) {
                        true => "<new>".to_owned(),
                        false => checked_out.trim().to_owned(),
                    };
                    let left = git(&at, &["status", "--porcelain"]);
                    said += &format!(
                        "managed {managed}, below {below}: {}, {checked_out} checked out{}\n",
                        branches.join(", "),
                        match left.is_empty() {
                            true => String::new(),
                            false => format!(", left {left:?}"),
                        },
                    );
                });
            }
        }
        // The worktree's lane top first, each branch's commits newest first, whether or not the
        // main checkout is the workspace. On top of the branch it has checked out, the worktree
        // checks out the new one, and either way keeps its files as they were.
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
managed false, below false: <new> [one], agent [two], <new> checked out
managed false, below true: agent [one], <new> [two], agent checked out
managed true, below false: <new> [one], agent [two], <new> checked out
managed true, below true: agent [one], <new> [two], agent checked out

"#]]
        );
    }

    #[test]
    fn the_apps_own_operations_dont_stop_undoing_the_one_before() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let (named, done) = (newest_entry(path), now_millis());
            let mut ran = vec![(started, done)];
            // Git logs moves in whole seconds.
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let started = now_millis();
            let at = Destination::Branch(full("refs/heads/named"));
            crate::backend::apply(path, InsertEmptyCommit { at }).unwrap();
            ran.push((started, now_millis()));
            let inserted = newest_entry(path);
            let started = now_millis();
            let undone = undo(path, inserted, Some((ran[1].1, &ran))).unwrap();
            assert!(matches!(undone, Undone::Done(_)), "the newest undoes");
            ran.push((started, now_millis()));
            assert!(
                matches!(
                    undo(path, named, Some((done, &ran))).unwrap(),
                    Undone::Done(_)
                ),
                "the branch moves the app's own operation and undo logged aren't another's"
            );
        })
    }

    #[test]
    fn undo_stops_for_a_commit_in_the_second_of_a_later_operation() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let done = now_millis();
            let named = newest_entry(path);
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
                    undo(path, named, Some((done, &[(started, done), later]))).unwrap(),
                    Undone::Behind(why) if why.contains("agent work")
                ),
                "a Git commit is no operation of the app's, whichever second it's in"
            );
        })
    }

    #[test]
    fn undo_refuses_without_a_log_of_head() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let done = now_millis();
            let named = newest_entry(path);
            // As core.logAllRefUpdates off leaves it.
            std::fs::remove_file(path.join(".git/logs/HEAD")).unwrap();
            let undone = undo(path, named, Some((done, &[(started, done)])));
            assert!(
                undone.is_err_and(|refused| refused.to_string().contains("no log")),
                "with nothing to tell what moved by, undo doesn't guess"
            );
        })
    }

    #[test]
    fn undo_and_redo_stop_for_what_came_after_and_for_files_written_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::{TakeSnapshot, Undone};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let top = head(path);
            let started = now_millis();
            crate::backend::apply(path, name_at(top, "named")).unwrap();
            let named = newest_entry(path);
            let done = now_millis();
            // Later than it, as an edit in an editor is.
            std::thread::sleep(std::time::Duration::from_millis(5));
            std::fs::write(path.join("file"), "two\n").unwrap();
            assert!(
                matches!(undo(path, named, Some((done, &[(started, done)]))).unwrap(),
                    Undone::Newer(files) if files == ["file"]),
                "a file written after the operation is one undoing it puts back"
            );

            crate::backend::apply(path, TakeSnapshot { message: None }).unwrap();
            let snapshot = newest_entry(path);
            assert!(
                matches!(undo(path, named, None).unwrap(), Undone::Behind(_)),
                "the snapshot came after it, and undoing it would take that back too"
            );
            assert!(
                matches!(
                    undo(path, snapshot, Some((done, &[(done, now_millis())]))).unwrap(),
                    Undone::Done(_)
                ),
                "written while the app's own undo ran, the file isn't the user's change"
            );
            assert!(
                matches!(undo(path, snapshot, None).unwrap(), Undone::Already(_)),
                "undone once, as by another client, it isn't taken for behind what came before"
            );
            let undoing = now_millis();
            assert!(
                matches!(undo(path, named, None).unwrap(), Undone::Done(_)),
                "what came before it is still there to undo"
            );
            let undone = now_millis();

            let undo = newest_entry(path);
            std::thread::sleep(std::time::Duration::from_millis(5));
            std::fs::write(path.join("file"), "three\n").unwrap();
            let spans = [(undoing, undone)];
            assert!(
                matches!(redo(path, undo, &spans).unwrap(),
                    Some(Undone::Newer(files)) if files == ["file"]),
                "a file written after the undo is one redoing it puts back"
            );
        })
    }

    #[test]
    fn undo_and_redo_stop_for_a_commit_made_outside_the_app_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let named = newest_entry(path);
            let done = now_millis();
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
                matches!(undo(path, named, Some((done, &[]))).unwrap(),
                    Undone::Behind(why) if why.contains("terminal work")),
                "undoing would take the commit out of every branch, so it says what came after"
            );

            assert!(
                matches!(undo(path, named, None).unwrap(), Undone::Done(_)),
                "undone anyway"
            );
            let undo = newest_entry(path);
            commit_later(2, "more terminal work");
            assert!(
                redo(path, undo, &[]).unwrap().is_none(),
                "redoing would take that one out too"
            );
        })
    }

    #[test]
    fn undo_stops_for_an_applied_branch_moved_outside_the_app_since() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let named = newest_entry(path);
            let done = now_millis();
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
                matches!(undo(path, named, Some((done, &[(started, done)]))).unwrap(),
                    Undone::Behind(why) if why.contains("named")),
                "undoing would move the branch back and drop that commit, so it says so"
            );
        })
    }

    #[test]
    fn undo_goes_on_past_what_git_logged_that_a_restore_leaves() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let (_outside, linked) = agent_worktree(path);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let done = now_millis();
            let named = newest_entry(path);
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
                    undo(path, named, Some((done, &[(started, done)]))).unwrap(),
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
            use crate::backend::history::Undone;
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            git(path, &["commit", "-q", "--allow-empty", "-m", "loose"]);
            let started = now_millis();
            crate::backend::apply(path, name_at(head(path), "named")).unwrap();
            let done = now_millis();
            let named = newest_entry(path);
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
                    undo(path, named, Some((accepted, &[(started, done)]))).unwrap(),
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
            use crate::backend::branches::{ApplyBranch, ApplyOutcome};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            git(&linked, &["commit", "-q", "--allow-empty", "-m", "agent's"]);
            let theirs = head(&linked);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);

            let agent = ApplyBranch {
                branch: full("refs/heads/agent"),
                freeing: Some("agent".into()),
            };
            let (applied, _) = crate::backend::apply(path, agent).unwrap();
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
            use crate::backend::branches::{ApplyBranch, ApplyOutcome};
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            std::fs::write(linked.join("file"), "agent's\n").unwrap();
            git(&linked, &["commit", "-q", "-am", "agent's"]);
            git(path, &["checkout", "-q", "-b", "gitbutler/workspace"]);
            std::fs::write(path.join("file"), "ours\n").unwrap();
            git(path, &["commit", "-q", "-am", "ours"]);
            crate::backend::apply(path, name_at(head(path), "ours")).unwrap();

            let agent = ApplyBranch {
                branch: full("refs/heads/agent"),
                freeing: Some("agent".into()),
            };
            let (applied, _) = crate::backend::apply(path, agent).unwrap();
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

            let remove = DeleteBranchAndCommits {
                branch: full("refs/heads/side"),
                tip: Some(asked),
            };
            let refused = crate::backend::apply(path, remove).unwrap_err();
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
            crate::backend::apply(path, branch_named("second")).unwrap();
            let head_ref = |path: &Path| git(path, &["symbolic-ref", "HEAD"]).trim().to_owned();
            assert_eq!(head_ref(path), "refs/heads/gitbutler/workspace");

            let delete = DeleteBranchAndCommits {
                branch: full("refs/heads/feature"),
                tip: Some(work),
            };
            crate::backend::apply(path, delete).unwrap();
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

            let last = newest_entry(path);
            undo(path, last, None).unwrap();
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
    fn a_linked_worktrees_top_branch_goes_and_leaves_it_on_the_one_below() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            let shown = crate::backend::worktrees::SetArchived {
                name: "agent".into(),
                archived: false,
            };
            crate::backend::apply(path, shown).unwrap();
            std::fs::write(linked.join("lower.txt"), "lower\n").unwrap();
            git(&linked, &["add", "lower.txt"]);
            git(&linked, &["commit", "-q", "-m", "lower"]);
            git(&linked, &["checkout", "-q", "-b", "upper"]);
            std::fs::write(linked.join("upper.txt"), "upper\n").unwrap();
            git(&linked, &["add", "upper.txt"]);
            git(&linked, &["commit", "-q", "-m", "upper"]);

            let delete = DeleteBranchAndCommits {
                branch: full("refs/heads/upper"),
                tip: Some(head(&linked)),
            };
            crate::backend::apply(path, delete).unwrap();
            assert!(
                git(path, &["branch", "--list", "upper"]).is_empty(),
                "the branch is gone"
            );
            assert_eq!(
                git(&linked, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/agent",
                "the worktree is on the branch below"
            );
            assert!(
                !linked.join("upper.txt").exists() && linked.join("lower.txt").exists(),
                "only its own commit went"
            );
            assert!(
                git(&linked, &["status", "--porcelain"]).is_empty(),
                "the worktree's files follow"
            );
        })
    }

    #[test]
    fn an_empty_branch_checked_out_on_its_own_goes_and_leaves_head_below() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["checkout", "-q", "-b", "feature"]);
            git(
                path,
                &["commit", "-q", "--allow-empty", "-m", "feature work"],
            );
            let work = head(path);
            let above = CreateDependentBranch {
                below: full("refs/heads/feature"),
            };
            let (tip, _) = crate::backend::apply(path, above).unwrap();
            assert_eq!(git(path, &["symbolic-ref", "HEAD"]).trim(), tip.to_string());

            let delete = DeleteBranchAndCommits {
                branch: tip,
                tip: None,
            };
            crate::backend::apply(path, delete).unwrap();
            assert_eq!(
                git(path, &["symbolic-ref", "HEAD"]).trim(),
                "refs/heads/feature",
                "HEAD is back on the branch it was made on"
            );
            assert_eq!(head(path), work, "whose commit stays");
        })
    }

    #[test]
    fn the_target_branch_isnt_deleted() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "not pushed"]);
            let tip = head(path);

            let delete = DeleteBranchAndCommits {
                branch: full("refs/heads/main"),
                tip: Some(tip),
            };
            let refused = crate::backend::apply(path, delete).expect_err("main is the target");
            assert!(
                format!("{refused:#}").contains("is the target branch"),
                "a checkout on it would have nothing to fall back to: {refused:#}"
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

            assert_eq!(
                crate::backend::apply(path, name_at(top, "named"))
                    .unwrap()
                    .0,
                "refs/heads/named"
            );
            assert_eq!(branch_at_top(path), Some(Some(full("refs/heads/named"))));

            let last = newest_entry(path);
            undo(path, last, None).unwrap();
            assert_eq!(branch_at_top(path), Some(None), "undo takes the name away");
        })
    }

    #[test]
    fn going_apart_outside_the_workspace_sets_it_up_and_keeps_both_branches() {
        let mut said = String::new();
        for what in ["unstack top", "unstack lower", "move Top to a new stack"] {
            but_testsupport::isolated_app_data_dir(|| {
                // `top` checked out on its own, stacked on `lower`.
                let dir = repo_with("one\n");
                let path = dir.path();
                target_origin_main(path);
                git(path, &["checkout", "-q", "-b", "lower"]);
                git(path, &["commit", "-q", "--allow-empty", "-m", "Lower"]);
                git(path, &["checkout", "-q", "-b", "top"]);
                git(path, &["commit", "-q", "--allow-empty", "-m", "Top"]);
                let lanes = |path: &Path| {
                    let workspace =
                        crate::backend::load_workspace(path, Default::default()).unwrap();
                    let mut lanes: Vec<String> = (workspace.lanes_in(&Checkout::Main))
                        .map(|lane| {
                            let branches = lane.branches.iter().map(|branch| {
                                let titles: Vec<_> =
                                    branch.commits.iter().map(|c| c.title.as_str()).collect();
                                format!("{} [{}]", branch.name, titles.join(", "))
                            });
                            branches.collect::<Vec<_>>().join(" on ")
                        })
                        .collect();
                    lanes.sort();
                    let head = git(path, &["symbolic-ref", "--short", "HEAD"]);
                    format!("{} on {}", lanes.join(" | "), head.trim())
                };
                let before = lanes(path);
                match what {
                    "move Top to a new stack" => {
                        let moving = MoveToNewBranch {
                            commits: [head(path)].into(),
                            beside: None,
                            first: false,
                            copy: false,
                        };
                        crate::backend::apply(path, moving).unwrap();
                    }
                    _ => {
                        let branch = full(&format!("refs/heads/{}", &what[8..]));
                        let unstack = TearOffBranch {
                            branch,
                            first: false,
                        };
                        crate::backend::apply(path, unstack).unwrap();
                    }
                }
                let after = lanes(path).replace("t-branch-1", "<new>");
                undo(path, newest_entry(path), None).unwrap();
                said += &format!(
                    "{what}: {after}; undone: {}\n",
                    if lanes(path) == before {
                        "as before"
                    } else {
                        "changed"
                    }
                );
            });
        }
        // Out of its stack it's a branch of its own, which only GitButler's workspace holds
        // beside another, as `but move` sets it up: none is left out of the checkout.
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
unstack top: lower [Lower] | top [Top] on gitbutler/workspace; undone: as before
unstack lower: lower [Lower] | top [Top] on gitbutler/workspace; undone: as before
move Top to a new stack: <new> [Top] | top [] on lower [Lower] on gitbutler/workspace; undone: as before

"#]]
        );
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
                crate::backend::apply(path, branch_named("beside")).unwrap();
                let at_api = CreateBranchAtCommit {
                    commit: api,
                    below: false,
                };
                crate::backend::apply(path, at_api).unwrap();
                let top = full("refs/heads/top");
                crate::backend::apply(
                    path,
                    TearOffBranch {
                        branch: top,
                        first: false,
                    },
                )
                .unwrap()
                .0
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
    fn a_hunk_discard_read_before_another_in_its_file_asks_to_try_again() {
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
            discard(path, Checkout::Main, &[hunk(0)], true).unwrap();
            // Read before the first discard, which took two lines out above it.
            let err = discard(path, Checkout::Main, &[hunk(1)], true).unwrap_err();
            assert!(
                err.to_string().contains("changed since the diff was shown"),
                "its place in the file is from before the first discard: {err}"
            );
            let diff = load_diff(path, DiffSource::Worktree, None, None, None, None).unwrap();
            let hunks = diff[0].hunks();
            let fresh = diff[0].part(Some(vec![hunks[0].key]));
            discard(path, Checkout::Main, &[fresh], true).unwrap();
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
            let err = discard(path, Checkout::Main, &[second], true).unwrap_err();
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
            discard(path, Checkout::Main, &[lines_picked], true).unwrap();

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

    /// The kind of each oplog entry newer than `before`, newest first, a line each.
    fn entries_since(path: &Path, before: Option<crate::backend::SnapshotId>) -> String {
        let ctx = open(path).unwrap();
        let snapshots = but_api::legacy::oplog::list_snapshots(&ctx, 100, None, None, None);
        (snapshots.unwrap().into_iter())
            .take_while(|snapshot| Some(snapshot.commit_id) != before.map(|id| id.0))
            .map(|snapshot| format!("{:?}\n", snapshot.details.unwrap().operation))
            .collect()
    }

    #[test]
    fn squashing_is_one_oplog_entry_its_run_saw() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);
            git(path, &["commit", "-q", "--allow-empty", "-m", "third"]);
            let third = head(path);
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let squash = Squash {
                commits: [third, second].into(),
                into: edit,
                keep: Messages::Both,
                emptied: None,
            };
            let (rewritten, ran) = crate::backend::apply(path, squash).unwrap();
            let squashed = rewritten[&edit];
            assert_eq!(
                (rewritten[&second], rewritten[&third]),
                (squashed, squashed),
                "a selection on any of the commits follows into the squash"
            );
            assert_eq!(
                ran.recorded,
                crate::backend::history::oplog_head(&open(path).unwrap()),
                "the run saw the entry it wrote"
            );
            let message = git(path, &["log", "-1", "--format=%B", &squashed.0.to_string()]);
            // One entry since the fixture, and one commit holding every message, oldest first.
            snapbox::assert_data_eq!(
                entries_since(path, before) + &message,
                snapbox::str![[r#"
SquashCommit
edit

second

third


"#]]
            );
        })
    }

    #[test]
    fn a_branch_squashed_away_goes_with_its_commits_and_undoes() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);
            crate::backend::apply(path, name_at(second, "feature")).unwrap();
            // Edit to a branch of its own, leaving feature with second alone.
            let moving = MoveToNewBranch {
                commits: [edit].into(),
                beside: None,
                first: false,
                copy: false,
            };
            let ((rewritten, _), _) = crate::backend::apply(path, moving).unwrap();
            let edit = rewritten.get(&edit).copied().unwrap_or(edit);
            // Rebased off edit.
            let feature = git(path, &["rev-parse", "feature"]);
            let second = CommitId(gix::ObjectId::from_hex(feature.trim().as_bytes()).unwrap());
            let before = crate::backend::history::oplog_head(&open(path).unwrap());

            let squash = Squash {
                commits: [second].into(),
                into: edit,
                keep: Messages::Both,
                emptied: Some(full("refs/heads/feature")),
            };
            let (rewritten, _) = crate::backend::apply(path, squash).unwrap();
            assert!(
                git(path, &["branch", "--list", "feature"]).is_empty(),
                "the branch its commits went out of goes"
            );
            let message = git(
                path,
                &["log", "-1", "--format=%B", &rewritten[&edit].0.to_string()],
            );
            // One entry, and the commit it went into holding both messages.
            snapbox::assert_data_eq!(
                entries_since(path, before) + &message,
                snapbox::str![[r#"
SquashCommit
edit

second


"#]]
            );

            undo(path, newest_entry(path), None).unwrap();
            assert_eq!(
                git(path, &["rev-parse", "feature"]).trim(),
                second.0.to_string(),
                "one undo brings the branch back with its commit"
            );
        })
    }

    #[test]
    fn a_squash_keeping_the_targets_message_drops_the_others() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            git(path, &["commit", "-q", "--allow-empty", "-m", "second"]);
            let second = head(path);

            let squash = Squash {
                commits: [second].into(),
                into: edit,
                keep: Messages::Target,
                emptied: None,
            };
            let (rewritten, _) = crate::backend::apply(path, squash).unwrap();
            let message = git(
                path,
                &["log", "-1", "--format=%B", &rewritten[&edit].0.to_string()],
            );
            assert_eq!(message.trim(), "edit", "only the target's message is left");
        })
    }

    #[test]
    fn a_branch_uncommitted_goes_and_leaves_its_changes_and_undoes() {
        but_testsupport::isolated_app_data_dir(|| {
            let (dir, edit, ..) = workspace_with_an_edit();
            let path = dir.path();
            crate::backend::apply(path, name_at(edit, "feature")).unwrap();

            let uncommit = Uncommit {
                commits: [edit].into(),
                emptied: Some(full("refs/heads/feature")),
            };
            crate::backend::apply(path, uncommit).unwrap();
            assert!(
                git(path, &["branch", "--list", "feature"]).is_empty(),
                "the branch goes"
            );
            let left: Vec<String> = (uncommitted(path).into_iter())
                .map(|file| file.path)
                .collect();
            assert_eq!(left, ["file"], "its commit's change is uncommitted");

            undo(path, newest_entry(path), None).unwrap();
            assert_eq!(
                git(path, &["rev-parse", "feature"]).trim(),
                edit.0.to_string(),
                "one undo brings the branch back with its commit"
            );
        })
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

            let pull = crate::backend::upstream::Pull {
                branch: full("refs/heads/feature"),
                accepted: None,
            };
            let (found, ran) = crate::backend::apply(path, pull).unwrap();
            assert_eq!(
                found,
                Some(vec![(Some(full("refs/heads/feature")), 1)]),
                "the remote's commit would land conflicted"
            );
            assert_eq!(
                ran.recorded, None,
                "a pull caught before it runs records nothing"
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
            let moving = MoveCommits {
                commits: [delete].into(),
                to: Destination::BelowCommit(edit),
            };
            let (rewritten, _) = crate::backend::apply(path, moving).unwrap();
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
            let uncommit = UncommitFiles {
                commit,
                files: [five_b_out(&diff)].into(),
            };
            let (rewritten, _) = crate::backend::apply(path, uncommit).unwrap();
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
            let split = SplitOff {
                commit,
                files: [five_b_out(&diff)].into(),
            };
            let ((rewritten, new), _) = crate::backend::apply(path, split).unwrap();
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
            let discard = DiscardFromCommit {
                commit,
                files: [five_b_out(&diff)].into(),
            };
            let (rewritten, _) = crate::backend::apply(path, discard).unwrap();
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
            let at = Destination::AboveCommit(commit);
            let (empty, _) = crate::backend::apply(path, InsertEmptyCommit { at }).unwrap();
            let move_files = MoveFiles {
                from: commit,
                to: empty,
                files: [five_b_out(&diff)].into(),
            };
            let (rewritten, _) = crate::backend::apply(path, move_files).unwrap();
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
            crate::backend::apply(path, amend_main(head, vec![picked])).unwrap();

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
        let repo = gix::open(path).unwrap();
        let before = crate::backend::worktrees::heads(&repo);
        git(&linked, &["checkout", "-q", "-b", "agent-next"]);
        assert!(
            crate::backend::worktrees::heads(&repo).moved_since(&before),
            "same commit, but the oplog can't put the branch it has checked out back"
        );
    }

    #[test]
    fn a_linked_worktree_removed_meanwhile_didnt_move() {
        let dir = repo_with(
            "base
",
        );
        let path = dir.path();
        let (_outside, linked) = agent_worktree(path);
        let repo = gix::open(path).unwrap();
        let before = crate::backend::worktrees::heads(&repo);
        git(
            path,
            &["worktree", "remove", "--force", linked.to_str().unwrap()],
        );
        let after = crate::backend::worktrees::heads(&repo);
        assert_ne!(after, before, "it's gone from the heads");
        assert!(
            !after.moved_since(&before),
            "but a worktree deleted beside an operation isn't one the operation moved"
        );
    }
}
