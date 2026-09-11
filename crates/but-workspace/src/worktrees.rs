//! Helpers for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration, archived-state reconciliation, and `HEAD` resolution are
//! centralized in `but-ctx`, keeping this crate independent of it.

use std::{ffi::OsStr, path::Path};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString};
use but_core::{DiffSpec, RepositoryExt};
pub use but_graph::workspace::WorktreeBase;

use crate::ref_info::{LocalCommit, Segment};

/// A non-archived linked worktree along with the first-parent history it owns exclusively,
/// i.e. the segments between its `HEAD` and the workspace, an earlier worktree, or the target.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// The stable worktree name, i.e. the directory name under `$GIT_COMMON_DIR/worktrees/`.
    pub name: BString,
    /// The branch the worktree has checked out, or `None` for a detached `HEAD`.
    pub ref_name: Option<gix::refs::FullName>,
    /// The commit the worktree `HEAD` peels to, as re-resolved during traversal.
    pub head: gix::ObjectId,
    /// What the last of [`Self::segments`] is resting on.
    ///
    /// This is `None` only if the traversal ran out of graph before reaching the workspace or the
    /// target, which happens for worktrees on unrelated history or when a traversal limit was hit.
    pub base: Option<WorktreeBase>,
    /// The segments from `head` down, never empty. The first is named by `ref_name`, or is
    /// anonymous for a detached `HEAD`.
    pub segments: Vec<Segment>,
}

impl WorktreeInfo {
    /// The commits owned by this worktree alone, from its `HEAD` down to (excluding) its
    /// [base](Self::base), along the first parent.
    pub fn commits(&self) -> impl Iterator<Item = &LocalCommit> {
        self.segments.iter().flat_map(|s| s.commits.iter())
    }
}

/// Open the linked worktree named `name` as a from-disk repository.
///
/// It shares `repo`'s object database and has no object memory, so objects written
/// through it land loose on disk and are immediately visible to in-memory
/// repositories built on the same database - which is what makes it usable as the
/// source repository of a worktree-sourced commit or amend.
pub fn open_worktree_repo(repo: &gix::Repository, name: &BStr) -> anyhow::Result<gix::Repository> {
    let proxy = repo
        .worktrees()?
        .into_iter()
        .find(|proxy| proxy.id() == name)
        .with_context(|| format!("Worktree {name} does not exist"))?;
    proxy.into_repo().map_err(Into::into)
}

/// The time of the newest reflog entry of the linked worktree named `name`, across its own
/// `HEAD` log and the log of the branch it has checked out, or `None` if neither has one.
///
/// The branch log sees updates made from any checkout, while the `HEAD` log sees checkouts and
/// commits made inside the worktree and is all a detached worktree has.
pub fn updated_at(repo: &gix::Repository, name: &BStr) -> anyhow::Result<Option<gix::date::Time>> {
    let wt_repo = open_worktree_repo(repo, name)?;
    let mut newest: Option<gix::date::Time> = None;
    for ref_name in std::iter::once("HEAD".try_into()?).chain(wt_repo.head_name()?) {
        let Some(reference) = wt_repo.try_find_reference(ref_name.as_ref())? else {
            continue;
        };
        let mut log = reference.log_iter();
        let Some(mut lines) = log.rev()? else {
            continue;
        };
        let Some(line) = lines.next().transpose()? else {
            continue;
        };
        let time = line.signature.time;
        if newest.is_none_or(|newest| time.seconds > newest.seconds) {
            newest = Some(time);
        }
    }
    Ok(newest)
}

/// Remove the linked worktree checked out at `path` the way `git worktree remove` does, which
/// refuses a dirty checkout unless `force`, and a locked one until it is unlocked.
///
/// Git is invoked directly as it has the only implementation of this, and its own error
/// message is surfaced on failure.
pub fn remove(repo: &gix::Repository, path: &Path, force: bool) -> anyhow::Result<()> {
    let mut args = Vec::new();
    if force {
        args.push(OsStr::new("--force"));
    }
    args.extend([OsStr::new("--"), path.as_os_str()]);
    git_worktree(repo, "remove", &args)
}

/// Create a linked worktree at `path` on the new branch `branch` starting at `base`, the way
/// `git worktree add -b` does, which refuses an existing branch.
///
/// An existing `path` is refused up front, as git would only notice it after creating the branch.
/// Returns the stable name git gave the worktree, normally the last component of `path`.
pub fn add(
    repo: &gix::Repository,
    path: &Path,
    branch: &gix::refs::FullNameRef,
    base: gix::ObjectId,
) -> anyhow::Result<BString> {
    if path.exists() {
        bail!("'{}' already exists", path.display());
    }
    let short_name = gix::path::from_bstr(branch.shorten());
    let base = base.to_string();
    git_worktree(
        repo,
        "add",
        &[
            OsStr::new("-b"),
            short_name.as_os_str(),
            OsStr::new("--"),
            path.as_os_str(),
            OsStr::new(&base),
        ],
    )?;
    gix::open(path)?
        .worktree()
        .and_then(|worktree| worktree.id().map(ToOwned::to_owned))
        .context("git registered the new checkout as a linked worktree")
}

fn git_worktree(repo: &gix::Repository, subcommand: &str, args: &[&OsStr]) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    // These would override `-C`.
    for var in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
        cmd.env_remove(var);
    }
    let output = cmd
        .arg("-C")
        .arg(repo.workdir().unwrap_or(repo.common_dir()))
        .args(["worktree", subcommand])
        .args(args)
        .output()
        .with_context(|| format!("Failed to run `git worktree {subcommand}`"))?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// The outcome of [`move_uncommitted_changes()`].
#[derive(Debug, Clone, Copy)]
pub struct MoveUncommittedChangesOutcome {
    /// Conflict markers were written into `main_repo`'s working directory and index.
    pub conflict_occurred: bool,
}

/// Move some or all of the uncommitted changes of the linked worktree `worktree_repo` into the
/// uncommitted changes of `main_repo`.
///
/// `worktree_repo` must share `main_repo`'s object database and have no object memory, as
/// returned by [`open_worktree_repo()`] - the same requirement `ChangeSource::Worktree`
/// (`crate::commit`) has, since this writes loose objects through it that `main_repo` must see
/// immediately.
///
/// If `selection` is `Some`, only those changes are moved - matched against the worktree's
/// current uncommitted changes the way `DiffSpec` selections are matched elsewhere, with
/// `context_lines` used to re-derive hunks and required to match whatever produced the
/// `DiffSpec`s. If `None`, every uncommitted change in the worktree is moved and `context_lines`
/// is unused.
pub fn move_uncommitted_changes(
    main_repo: &gix::Repository,
    worktree_repo: &gix::Repository,
    selection: Option<Vec<DiffSpec>>,
    context_lines: u32,
) -> anyhow::Result<MoveUncommittedChangesOutcome> {
    let worktree_head_tree = worktree_repo.head_tree_id_or_empty()?.detach();

    let moved_tree = match selection {
        None =>
        {
            #[expect(deprecated)]
            worktree_repo.create_wd_tree(0)?
        }
        Some(selection) => {
            if selection.is_empty() {
                bail!("No changes were selected to move");
            }
            let outcome = but_core::tree::create_tree(
                worktree_repo,
                worktree_head_tree,
                selection,
                context_lines,
            )?;
            let rejected: Vec<_> = outcome
                .rejected_specs
                .iter()
                .map(|(reason, spec)| format!("{reason:?}: {}", spec.path))
                .collect();
            if !rejected.is_empty() {
                bail!(
                    "Some selected changes no longer match the worktree's uncommitted changes: {}",
                    rejected.join(", ")
                );
            }
            outcome.destination_tree.context("No changes to move")?
        }
    };
    if moved_tree == worktree_head_tree {
        bail!("No changes to move");
    }

    let destination_outcome = but_core::worktree::safe_checkout_from_head(
        moved_tree,
        main_repo,
        but_core::worktree::checkout::Options {
            merge_base_override: Some(worktree_head_tree),
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    but_core::worktree::safe_checkout_from_head(
        worktree_head_tree,
        worktree_repo,
        but_core::worktree::checkout::Options {
            merge_base_override: Some(moved_tree),
            allow_uncommitted_changes_to_conflict_with_new_head: true,
            ..Default::default()
        },
    )?;

    Ok(MoveUncommittedChangesOutcome {
        conflict_occurred: destination_outcome.conflict_occurred,
    })
}
