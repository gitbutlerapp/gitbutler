//! Mutations. Each opens the project, calls the matching `but-api` function and
//! returns; the UI reloads the workspace afterwards. Everything that rewrites the
//! workspace records undo history through `but-api`'s own oplog snapshots.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context as _, Result, bail};
use but_api::commit::json::ChangesSource;
use but_core::tree::create_tree::RejectionReason;
use but_core::{DiffSpec, DryRun};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};

use super::{CommitId, Selected, open};

/// Where a commit or a set of changes lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The top of this branch (a full ref name).
    Branch(String),
    /// The bottom of the branch stacked on this one.
    AboveBranch(String),
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
            Destination::AboveBranch(ref_name) => (
                RelativeTo::Reference(full_name(ref_name)?),
                InsertSide::Above,
            ),
            Destination::AboveCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Above),
            Destination::BelowCommit(id) => (RelativeTo::Commit(id.0), InsertSide::Below),
        })
    }
}

fn full_name(ref_name: &str) -> Result<gix::refs::FullName> {
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
        R::CherryPickMergeConflict | R::WorkspaceMergeConflict => {
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
        None => bail!("Nothing could be committed.\n{}", left_out.join("\n")),
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

fn holds(segment: &but_workspace::ref_info::Segment, branch: &str) -> bool {
    segment
        .ref_info
        .as_ref()
        .is_some_and(|info| info.ref_name.as_bstr() == branch)
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

/// Put a copy of `commit` at `to`, keeping the original where it is.
pub fn cherry_pick(path: &Path, commit: CommitId, to: &Destination) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let (relative_to, side) = to.relative_to()?;
    let outcome = but_api::commit::cherry_pick::commit_cherry_pick(
        &mut ctx,
        vec![commit.0],
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

pub fn remove_branch(path: &Path, ref_name: &str) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::branch::branch_remove(&mut ctx, full_name(ref_name)?)?;
    Ok(())
}

/// Fold the `selected` uncommitted changes into `commit`.
pub fn amend(path: &Path, commit: CommitId, selected: &[Selected]) -> Result<(Rewritten, LeftOut)> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::amend::commit_amend(
        &mut ctx,
        commit.0,
        selected_specs(selected),
        ChangesSource::Head,
        DryRun::No,
    )?;
    let left_out = left_out(&outcome.rejected_specs);
    if outcome.new_commit.is_none() {
        bail!("Nothing could be amended.\n{}", left_out.join("\n"));
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
pub fn uncommit_files(path: &Path, commit: CommitId, files: &[Selected]) -> Result<Rewritten> {
    let mut ctx = open(path)?;
    let outcome = but_api::commit::uncommit::commit_uncommit_changes(
        &mut ctx,
        commit.0,
        selected_specs(files),
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
        selected_specs(files),
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
        selected_specs(files),
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
    let mut meta = ctx.meta()?;
    let changes = selected_specs(files);
    let (new, workspace) = but_transaction::with_transaction(
        &mut ctx,
        &mut meta,
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

/// Rebase every stack onto the target as it is now, or with `branch` only the stack
/// holding it. Doesn't fetch first; see [`fetch()`].
pub fn update_from_target(path: &Path, branch: Option<&str>) -> Result<()> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let stacks = info.stacks.iter().filter(|stack| {
        branch.is_none_or(|branch| stack.segments.iter().any(|s| holds(s, branch)))
    });
    let updates = but_api::workspace::rebase_lane_bottoms(stacks.map(|stack| stack.lane()));
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

/// Put `branch` (a short name), or with `whole_stack` it and every branch below it,
/// straight onto the target and push the target. Returns whether the other branches still
/// need an update to sit on the moved target.
pub fn land(path: &Path, branch: &str, whole_stack: bool) -> Result<bool> {
    let mut ctx = open(path)?;
    let landed = but_api::land::branch_land(&mut ctx, branch.to_owned(), false, whole_stack)?;
    Ok(landed.reconcile_skipped)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::backend::{DiffBody, DiffSource, load_diff};

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    #[test]
    fn amending_some_hunks_leaves_the_rest_uncommitted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        git(path, &["init", "-q"]);
        git(path, &["config", "user.name", "T"]);
        git(path, &["config", "user.email", "t@example.com"]);
        git(path, &["config", "commit.gpgsign", "false"]);
        let mut lines: Vec<String> = (1..=20).map(|n| format!("line {n}\n")).collect();
        std::fs::write(path.join("file"), lines.concat()).unwrap();
        git(path, &["add", "file"]);
        git(path, &["commit", "-q", "-m", "first"]);
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
        amend(path, head, &[first_hunk]).unwrap();

        let amended = git(path, &["show", "HEAD:file"]);
        assert!(amended.starts_with("first edit\n"), "{amended}");
        assert!(amended.ends_with("line 20\n"), "{amended}");
        assert_eq!(
            std::fs::read_to_string(path.join("file")).unwrap(),
            lines.concat(),
            "the worktree keeps the last edit, now uncommitted"
        );
    }
}
