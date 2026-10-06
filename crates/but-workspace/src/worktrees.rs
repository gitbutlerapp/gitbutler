//! Helpers for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration, archived-state reconciliation, and `HEAD` resolution are
//! centralized in `but-ctx`, keeping this crate independent of it.

#[cfg(feature = "worktree-cow")]
use std::fs;
use std::{ffi::OsStr, path::Path};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString};
use but_core::{DiffSpec, RepositoryExt};
pub use but_graph::workspace::WorktreeBase;

#[cfg(feature = "worktree-cow")]
use gix::utils::AsBStr;

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
    add_inner(repo, path, branch, base, false)
}

fn add_inner(
    repo: &gix::Repository,
    path: &Path,
    branch: &gix::refs::FullNameRef,
    base: gix::ObjectId,
    no_checkout: bool,
) -> anyhow::Result<BString> {
    if path.exists() {
        bail!("'{}' already exists", path.display());
    }
    let short_name = gix::path::from_bstr(branch.shorten());
    let base = base.to_string();

    let mut args = vec![];
    if no_checkout {
        args.push(OsStr::new("--no-checkout"));
    }
    args.extend([
        OsStr::new("-b"),
        short_name.as_os_str(),
        OsStr::new("--"),
        path.as_os_str(),
        OsStr::new(&base),
    ]);

    git_worktree(repo, "add", &args)?;
    gix::open(path)?
        .worktree()
        .and_then(|worktree| worktree.id().map(ToOwned::to_owned))
        .context("git registered the new checkout as a linked worktree")
}

/// Create a linked worktree by cloning the main worktree.
/// Uses copy-on-write on macOS and a full-copy development mock on Linux.
#[cfg(feature = "worktree-cow")]
pub fn add_cow(
    repo: &gix::Repository,
    path: &Path,
    branch: &gix::refs::FullNameRef,
    base: gix::ObjectId,
) -> anyhow::Result<BString> {
    let Some(workdir) = repo.workdir() else {
        anyhow::bail!("Cannot use COW worktree mode on bare repository");
    };
    if let Some(mut submodules) = repo.submodules()? {
        anyhow::ensure!(
            submodules.next().is_none(),
            "COW mode is not supported for submodules"
        );
    }

    let worktree_name = add_inner(repo, path, branch, base, true)?;

    // If we fail after this point, we must try to remove the created worktree, so all further
    // actions are encapsulated in this awkward closure :)
    //
    // We intentionally do not remove any potentially created branch. It's not a big deal if we
    // leave it dangling around and it's better to not remove the branch than to accidentally remove
    // one that wasn't created in this worktree creation.
    let fill_worktree = || -> anyhow::Result<()> {
        let source_directory = gix::path::realpath(workdir)?;
        let destination = gix::path::realpath(path)?;
        let worktree_repo = open_worktree_repo(repo, worktree_name.as_bstr())?;
        let worktree_root_tree = worktree_repo.head_commit()?.tree()?;

        anyhow::ensure!(
            !destination.starts_with(&source_directory),
            "Nested worktrees not allowed for COW"
        );

        clone_worktree_files(&source_directory, &destination, &worktree_root_tree)?;

        // As the worktree is built with --no-checkout, the index is empty. Rebuilding the index for
        // the target tree is important to prevent libgit2 from forcibly overwriting all cloned
        // files.
        let mut index = worktree_repo.index_from_tree(&worktree_repo.head_tree_id_or_empty()?)?;
        index.write(Default::default())?;

        let checkout_repo = git2::Repository::open(&destination)?;
        // Restore tracked files first, including dirty or missing .gitignores. Doing this first is
        // important to correctly compute untracked and ignored files in the next stage.
        checkout_repo
            .checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .context("Failed to restore tracked worktree files")?;
        // Remove untracked files now that the target state's gitignores are in place.
        //
        // Note: This has a bug where a directory that contains ignored files and at least one
        // untracked file gets wiped completely instead of the expected outcome of just having the
        // untracked file(s) be removed. We don't think that's a huge deal.
        checkout_repo
            .checkout_head(Some(
                git2::build::CheckoutBuilder::new()
                    .force()
                    .remove_untracked(true)
                    .remove_ignored(false),
            ))
            .context("Failed to remove untracked worktree files")?;

        Ok(())
    };

    if let Err(err) = fill_worktree() {
        let err = err.context("Failed to clone files into worktree");

        let err = if let Err(remove_err) = remove(repo, path, true) {
            err.context(remove_err)
        } else {
            err
        };

        return Err(err);
    }

    Ok(worktree_name)
}

/// Clone all from the `working_directory` into `destination`, assuming that `target_root_tree`
/// corresponds to the desired Git state of the destination.
///
/// The root `.git` directory, `.gitignore` files that are not in `target_root_tree` and special
/// files such as FIFOs are skipped.
///
/// Note that this is _not_ safe if the `working_directory` contains submodules!
#[cfg(feature = "worktree-cow")]
fn clone_worktree_files(
    working_directory: &Path,
    destination: &Path,
    target_root_tree: &gix::Tree<'_>,
) -> anyhow::Result<()> {
    let ignored_paths = [working_directory.join(".git")];
    let filter_path = |path: &Path| {
        if ignored_paths.iter().any(|p| p == path) {
            return Ok(false);
        }

        if !path.ends_with(".gitignore") {
            return Ok(true);
        }

        // Gitignores absent from the target tree would survive the first checkout and
        // incorrectly influence which artifacts the second checkout preserves.
        let relpath = path.strip_prefix(working_directory)?;
        Ok(target_root_tree
            .lookup_entry_by_path(relpath)
            .map(|lookup| lookup.is_some())?)
    };

    clone_directory(working_directory, destination, &filter_path)
}

#[cfg(feature = "worktree-cow")]
fn clone_directory(
    source: &Path,
    destination: &Path,
    filter_path: &impl Fn(&Path) -> anyhow::Result<bool>,
) -> anyhow::Result<()> {
    anyhow::ensure!(source.is_dir(), "Source must be a directory");

    for entry in
        fs::read_dir(source).with_context(|| format!("Failed to read '{}'", source.display()))?
    {
        let entry = entry?;

        let source = entry.path();
        if !filter_path(&source)? {
            continue;
        }

        let destination = destination.join(entry.file_name());

        let kind = entry.file_type()?;
        if kind.is_dir() {
            fs::create_dir(&destination)
                .with_context(|| format!("Failed to create '{}'", destination.display()))?;
            clone_directory(&source, &destination, filter_path)?;
            fs::set_permissions(&destination, entry.metadata()?.permissions())?;
        } else if source.is_symlink() || source.is_file() {
            clone_file(&source, &destination).with_context(|| {
                format!(
                    "Failed to clone file from '{}' to '{}'",
                    source.display(),
                    destination.display()
                )
            })?;
        } else {
            tracing::debug!(
                "Cannot clone special file '{}' - skipping",
                source.display()
            )
        }
    }

    Ok(())
}

#[cfg(all(
    test,
    feature = "worktree-cow",
    any(target_os = "linux", target_os = "macos")
))]
mod tests {
    #[test]
    fn clone_copies_dirty_tracked_gitignore_before_restoration() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let repo = gix::init(&source)?;
        let target_ignore = repo.write_blob(b"/target-only\n")?;
        let target_tree = repo
            .write_object(&gix::objs::Tree {
                entries: vec![gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryKind::Blob.into(),
                    filename: ".gitignore".into(),
                    oid: target_ignore.detach(),
                }],
            })?
            .object()?
            .into_tree();

        std::fs::write(source.join(".gitignore"), "/source-only\n")?;
        std::fs::create_dir(source.join("untracked"))?;
        std::fs::write(source.join("untracked/.gitignore"), "/untracked-only\n")?;
        std::fs::write(source.join("ordinary"), "copied\n")?;
        let source = gix::path::realpath(&source)?;
        let destination = tempfile::tempdir_in(temp.path())?;

        // Exercise the production cloning stage without checkout.
        super::clone_worktree_files(&source, destination.path(), &target_tree)?;

        assert_eq!(
            std::fs::read_to_string(destination.path().join(".gitignore"))
                .expect("tracked .gitignore must be copied before restoration"),
            "/source-only\n",
            "copied contents come from the dirty source, not the target tree"
        );
        assert!(
            !destination.path().join("untracked/.gitignore").exists(),
            "gitignores absent from the target tree must not be copied"
        );
        assert!(
            !destination.path().join(".git").exists(),
            "repository metadata must not be copied"
        );
        assert_eq!(
            std::fs::read_to_string(destination.path().join("ordinary"))?,
            "copied\n",
            "ordinary files are still copied"
        );
        Ok(())
    }
}

/// Test-only mock of COW cloning on Linux: copies files without sharing storage.
/// Not intended for published Linux builds.
#[cfg(all(feature = "worktree-cow", target_os = "linux"))]
fn clone_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if source.is_symlink() {
        let target = fs::read_link(source)?;
        std::os::unix::fs::symlink(&target, destination)?;
    } else if source.is_file() {
        fs::copy(source, destination)?;
    } else {
        bail!(
            "Invalid source type '{}' for clone_file",
            source
                .metadata()
                .map(|meta| format!("{:?}", meta.file_type()))
                .unwrap_or("UNKNOWN".to_string())
        )
    }

    Ok(())
}

#[cfg(all(
    feature = "worktree-cow",
    not(any(target_os = "macos", target_os = "linux"))
))]
fn clone_file(_source: &Path, _destination: &Path) -> anyhow::Result<()> {
    bail!("COW worktrees are only supported on macOS and Linux")
}

#[cfg(all(feature = "worktree-cow", target_os = "macos"))]
fn clone_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // Defined by <sys/clonefile.h>, but not exported by libc.
    const CLONE_NOFOLLOW: u32 = 0x0001;
    // SAFETY: Both pointers reference live, NUL-terminated path strings. CLONE_NOFOLLOW
    // clones symlinks themselves, including dangling links, rather than their targets.
    let result = unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), CLONE_NOFOLLOW) };
    if result != 0 {
        return Err(std::io::Error::last_os_error()).context(
            "clonefile(2) failed; source and destination must share a filesystem supporting copy-on-write cloning (no full-copy fallback)",
        );
    }
    Ok(())
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
