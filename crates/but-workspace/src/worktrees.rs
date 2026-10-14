//! Helpers for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration, archived-state reconciliation, and `HEAD` resolution are
//! centralized in `but-ctx`, keeping this crate independent of it.

use std::{path::Path, sync::atomic::AtomicBool};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString};
use but_core::{DiffSpec, RepositoryExt};
pub use but_graph::workspace::WorktreeBase;

use crate::ref_info::{Lane, LocalCommit, Segment};

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
    /// What the worktree rests on, if that is the target and it has a branch or commits of its
    /// own to rebase there.
    pub fn rebasable_base(&self) -> Option<gix::ObjectId> {
        match self.base {
            Some(WorktreeBase::Outside(base))
                if self.ref_name.is_some() || self.commits().next().is_some() =>
            {
                Some(base)
            }
            _ => None,
        }
    }

    /// The worktree as a lane, which rests on another lane only when based inside the workspace.
    pub fn lane(&self) -> Lane<'_> {
        Lane {
            segments: &self.segments,
            rests_on: match self.base {
                Some(WorktreeBase::InWorkspace(id)) => Some(id),
                Some(WorktreeBase::Outside(_)) | None => None,
            },
            worktree: Some(self.name.as_ref()),
        }
    }

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
        .find(|proxy| proxy.id().is_ok_and(|id| id == name))
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

/// Remove the linked worktree named `name`, refusing a dirty checkout unless `force`.
/// Uses gix's bounded retries to handle files created during deletion.
/// Forced removal overrides locks; otherwise a lock is released and restored if removal is refused.
pub fn remove(repo: &gix::Repository, name: &BStr, force: bool) -> anyhow::Result<()> {
    use gix::worktree::remove::{Error, Force};
    let worktree = repo
        .worktree_proxy_by_id(name)?
        .with_context(|| format!("Worktree {name} does not exist"))?;
    let remove = || {
        let result = worktree.clone().remove(
            if force {
                Force::OverrideLock
            } else {
                Force::Never
            },
            gix::progress::Discard,
        );
        match result {
            Err(err)
                if matches!(
                    err.downcast_any_ref::<Error>(),
                    Some(Error::Dirty { .. } | Error::ContainsSubmodule { .. })
                ) =>
            {
                Err(anyhow::Error::from(err))
                    .context("Worktree contains changes; use --force to discard them")
            }
            result => result.map_err(Into::into),
        }
    };
    if force || !worktree.is_locked() {
        return remove();
    }
    let lock_reason = worktree
        .lock_reason()
        .context("Could not read the worktree lock reason")?;
    worktree
        .set_lock(None)
        .context("Could not unlock the worktree")?;
    remove().map_err(|err| match worktree.set_lock(Some(lock_reason.as_ref())) {
        Ok(()) => err,
        Err(relock_err) => err.context(relock_err),
    })
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
    anyhow::ensure!(
        branch.category() == Some(gix::refs::Category::LocalBranch),
        "Worktree branch must be a local branch"
    );
    let mut source = repo.clone();
    source.clear_namespace();
    source
        .find_commit(base)
        .context("Worktree base must be an existing commit")?;
    source.reference(
        branch,
        base,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "branch: Created for linked worktree",
    )?;
    let prepared = source.prepare_add_worktree(
        path,
        gix::worktree::add::Head::Attached(branch.to_owned()),
        &AtomicBool::default(),
    )?;
    let (created, outcome) = prepared.checkout(gix::progress::Discard, &AtomicBool::default())?;
    // Like Git, tolerate path collisions on case-insensitive filesystems.
    if !outcome.errors.is_empty()
        || !outcome.delayed_paths_unknown.is_empty()
        || !outcome.delayed_paths_unprocessed.is_empty()
    {
        let checkout_error = incomplete_checkout_error(&outcome);
        let worktree = created.worktree().context("created linked worktree")?;
        let name = worktree
            .id()?
            .context("gix registered the new checkout as a linked worktree")?;
        return match remove(&source, name, true) {
            Ok(()) => Err(checkout_error),
            Err(cleanup) => Err(checkout_error.context(cleanup)),
        };
    }
    // Git's post-checkout hook runs after registration succeeds, and failure retains the worktree.
    let output = std::process::Command::new(gix::path::env::exe_invocation())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-C")
        .arg(path)
        .args(["hook", "run", "--ignore-missing", "post-checkout", "--"])
        .arg(repo.object_hash().null().to_string())
        .arg(base.to_string())
        .arg("1")
        .output()
        .context("Failed to run the worktree post-checkout hook")?;
    anyhow::ensure!(
        output.status.success(),
        "Worktree post-checkout hook failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    created
        .worktree()
        .context("created linked worktree")?
        .id()?
        .map(ToOwned::to_owned)
        .context("gix registered the new checkout as a linked worktree")
}

fn incomplete_checkout_error(outcome: &gix::worktree::state::checkout::Outcome) -> anyhow::Error {
    let mut message = format!(
        "Worktree checkout was incomplete: {} errors, {} unknown delayed paths, {} unprocessed delayed paths",
        outcome.errors.len(),
        outcome.delayed_paths_unknown.len(),
        outcome.delayed_paths_unprocessed.len()
    );
    if !outcome.errors.is_empty() {
        message.push_str("\nCheckout errors:");
        for record in &outcome.errors {
            message.push_str(&format!("\n{:?}: {:#}", record.path, record.error));
        }
    }
    if !outcome.delayed_paths_unknown.is_empty() {
        message.push_str(&format!(
            "\nUnknown delayed paths: {:?}",
            outcome.delayed_paths_unknown
        ));
    }
    if !outcome.delayed_paths_unprocessed.is_empty() {
        message.push_str(&format!(
            "\nUnprocessed delayed paths: {:?}",
            outcome.delayed_paths_unprocessed
        ));
    }
    anyhow::anyhow!(message)
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

#[cfg(test)]
mod checkout_tests {
    use gix::error::ErrorExt as _;
    use gix::worktree::state::checkout::{ErrorRecord, Outcome};
    use snapbox::IntoData as _;

    #[test]
    fn incomplete_checkout_reports_paths_and_error_causes() {
        let outcome = Outcome {
            errors: vec![
                ErrorRecord {
                    path: "filtered.txt".into(),
                    error: gix::error::message("smudge filter failed").raise(),
                },
                ErrorRecord {
                    path: "unwritable.txt".into(),
                    error: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "write denied",
                    )
                    .and_raise(gix::error::message("Could not write file")),
                },
            ],
            delayed_paths_unknown: vec!["unknown.txt".into()],
            delayed_paths_unprocessed: vec!["unprocessed.txt".into(), b"non-utf8-\xff.txt".into()],
            ..Default::default()
        };

        // Checkout diagnostics must survive removal of the incomplete worktree.
        snapbox::assert_data_eq!(
            super::incomplete_checkout_error(&outcome).to_string(),
            snapbox::str![[r#"
Worktree checkout was incomplete: 2 errors, 1 unknown delayed paths, 2 unprocessed delayed paths
Checkout errors:
"filtered.txt": smudge filter failed
"unwritable.txt": Could not write file: I/O error (PermissionDenied): write denied
Unknown delayed paths: ["unknown.txt"]
Unprocessed delayed paths: ["unprocessed.txt", "non-utf8-\xff.txt"]
"#]]
            .raw()
        );
    }
}
