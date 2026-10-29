//! Helpers for linked git worktrees (experimental).
//!
//! Linked worktrees are identified by their stable *name*, i.e. the directory name
//! under `$GIT_COMMON_DIR/worktrees/`, which survives `git worktree move`.
//! Enumeration, archived-state reconciliation, and `HEAD` resolution are
//! centralized in `but-ctx`, keeping this crate independent of it.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::PathBuf;
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

/// The destination was occupied before worktree creation made any changes.
/// Callers may safely retry creation at a different path.
#[derive(Debug)]
pub struct DestinationExists(pub std::path::PathBuf);

impl std::fmt::Display for DestinationExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' already exists", self.0.display())
    }
}

impl std::error::Error for DestinationExists {}

/// The branch a new linked worktree checks out.
#[derive(Debug, Clone, Copy)]
pub enum WorktreeBranch<'a> {
    /// Create a branch the way `git worktree add -b` does, which refuses an existing branch.
    New {
        /// The branch to create.
        name: &'a gix::refs::FullNameRef,
        /// The commit the branch starts at.
        base: gix::ObjectId,
    },
    /// Check out the existing branch, which git refuses if any worktree has it checked out.
    Existing(&'a gix::refs::FullNameRef),
}

impl WorktreeBranch<'_> {
    /// The branch the worktree checks out.
    pub fn name(&self) -> &gix::refs::FullNameRef {
        match self {
            WorktreeBranch::New { name, .. } | WorktreeBranch::Existing(name) => name,
        }
    }
}

/// Create a linked worktree at `path` on `branch`.
///
/// An existing `path` is refused up front, as git would only notice it after creating the branch.
/// Returns the stable name git gave the worktree, normally the last component of `path`.
pub fn add(
    repo: &gix::Repository,
    path: &Path,
    branch: WorktreeBranch<'_>,
) -> anyhow::Result<BString> {
    match path.symlink_metadata() {
        Ok(_) => return Err(DestinationExists(path.to_owned()).into()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    anyhow::ensure!(
        branch.name().category() == Some(gix::refs::Category::LocalBranch),
        "Worktree branch must be a local branch"
    );
    let mut source = repo.clone();
    source.clear_namespace();
    if let WorktreeBranch::New { name, base } = branch {
        source
            .find_commit(base)
            .context("Worktree base must be an existing commit")?;
        source.reference(
            name,
            base,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "branch: Created for linked worktree",
        )?;
    }
    let prepared = source.prepare_add_worktree(
        path,
        gix::worktree::add::Head::Attached(branch.name().to_owned()),
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
    but_core::hooks::post_checkout(
        &created,
        repo.object_hash().null(),
        created.head_id()?.detach(),
    )?;
    if let Err(err) = handle_worktreeinclude(repo, &created) {
        // This is most likely not a fatal issue, the worktree checkout is almost certainly still
        // usable. So we only log the error.
        tracing::error!(?err, "Failed to copy .worktreeinclude files");
    }

    created
        .worktree()
        .context("created linked worktree")?
        .id()?
        .map(ToOwned::to_owned)
        .context("gix registered the new checkout as a linked worktree")
}

enum CopyMode {
    /// Copy files wholesale.
    Copy,
    #[cfg(target_os = "macos")]
    /// Attempt to clone files, falling back to copying if cloning fails.
    CloneWithFallback,
}

/// Handle the .worktreeinclude rules and copy any file from the source worktree into the
/// destination worktree that satisfies the following criteria.
///
/// 1. Is NOT a special git file
/// 2. Is NOT tracked in either source or destination worktrees
/// 3. Is either a symlink or a regular file - symlinks are copied verbatim, not followed
/// 4. Is gitignored in the destination
/// 5. Is NOT a prefix path of a tracked file in the destination worktree
/// 6. Is matched by any pattern in the source's .worktreeinclude
/// 7. Is NOT inside the destination worktree, if destination is nested inside source
///
/// The destination directory must contain a complete, clean, non-sparse checkout.
///
/// Permissions on traversed directories are retained so long as they were not part of the
/// destination checkout.
///
/// IMPORTANT: Several safety properties of the algorithm, especially relating to not following
/// symlinks, rely on there being no concurrent modifications of the source or destination
/// directories mid execution.
///
/// There are two main issues related to symlinks:
///
/// 1. A leaf node (file) change from a file into a symlink between metadata lookup and copying.
///    This is a minor risk in practice as the time between metadata lookup and file copying is
///    small.
/// 2. An ancestor node of the currently visited node might be replaced by a symlink mid-traversal,
///    which invalidates the invariant that no ancestors are symlinks. Copying a descendant of that
///    tree would follow the symlink.
///
/// It's difficult to get this bulletproof across operating systems, and most likely, the current
/// setup will work well in practice. If it turns out to be a larger issue we'll have to invest some
/// more time into making this concurrency safe.
fn handle_worktreeinclude(
    repo: &gix::Repository,
    worktree_repo: &gix::Repository,
) -> Result<(), anyhow::Error> {
    let src_dir = repo
        .workdir()
        .context("source worktree must have workdir")?;
    let worktree_include_path = src_dir.join(".worktreeinclude");
    if !worktree_include_path.exists() {
        return Ok(());
    }

    let dst_dir = worktree_repo
        .workdir()
        .context("destination worktree must have workdir")?;

    // Resolve aliases and relative paths before checking containment. Never traverse the new
    // checkout itself, as copying while walking could repeatedly copy our own output.
    let source_root = fs::canonicalize(src_dir)?;
    let destination_root = fs::canonicalize(dst_dir)?;
    let nested_destination = destination_root
        .strip_prefix(&source_root)
        .ok()
        .map(|relative| src_dir.join(relative));

    let worktree_include = fs::read(&worktree_include_path)?;
    let worktree_include_roots: Vec<_> = gix::ignore::parse(&worktree_include, false)
        .filter_map(|(pattern, _, _)| {
            if !pattern.has_wildcard() && !pattern.is_negative() {
                Some(pattern.text)
            } else {
                None
            }
        })
        .filter_map(|root| gix::path::from_bstr(&root).ok().map(PathBuf::from))
        .collect();

    let src_index = repo.index()?;
    let dst_index = worktree_repo.index()?;

    let mut dst_ignore = worktree_repo.excludes(
        &dst_index,
        None,
        gix::worktree::stack::state::ignore::Source::IdMapping,
    )?;

    let mut destination_dir_is_created = HashSet::new();
    let mut created_directory_permissions = Vec::new();

    #[cfg(target_os = "macos")]
    let mut copy_mode = CopyMode::CloneWithFallback;
    #[cfg(not(target_os = "macos"))]
    let copy_mode = CopyMode::Copy;

    let mut visit = |path: &Path| {
        if nested_destination.as_deref() == Some(path) {
            return Ok(false);
        }

        let is_special_git_file = path
            .file_name()
            .map(|name| name == ".git" || name == ".gitignore" || name == ".gitattributes")
            .unwrap_or_default();
        if is_special_git_file {
            return Ok(false);
        }

        let relpath = path.strip_prefix(src_dir)?;
        let index_path = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(relpath)?);
        let tracked_in_destination = dst_index.entry_by_path(&index_path).is_some();
        let tracked_in_source = src_index.entry_by_path(&index_path).is_some();

        if tracked_in_destination || tracked_in_source {
            return Ok(false);
        }

        // Assuming there aren't that many inclusion rules, this wasteful iteration is fine.
        let mut in_inclusion = false;
        let mut toward_inclusion = false;
        for root in &worktree_include_roots {
            if relpath.starts_with(root) {
                in_inclusion = true;
                break;
            }
            if root.starts_with(relpath) {
                toward_inclusion = true
            }
        }

        if !in_inclusion && !toward_inclusion {
            // We can prune here, there's no chance to encounter an included file after this point.
            return Ok(false);
        }

        let metadata = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => bail!(err),
        };
        let kind = metadata.file_type();

        // Due to case folding quirks in different file systems, we can't rely entirely on index
        // lookups to determine if a path is tracked (or a prefix of a tracked entry). We must
        // check each intermediate destination path, and can only allow further traversal if it
        // either doesn't exist, or both the source and destination are directories (which are
        // definitionally not tracked).
        //
        // Note that the case-insensitive gix index only supports ASCII folding, so it cannot be
        // relied upon to spot case aliases that fold in other ways.
        //
        // This is very important to protect against accidentally following symlinks in the
        // destination, and also prevents us from attempting to copy a source file into a path
        // that's already occupied by a checked out directory.
        match fs::symlink_metadata(dst_dir.join(relpath)) {
            Ok(meta) => {
                if !(meta.is_dir() && kind.is_dir()) {
                    return Ok(false);
                }
            }
            Err(err) if err.kind() != io::ErrorKind::NotFound => bail!(err),
            _ => (),
        }

        let mode = if kind.is_dir() {
            return Ok(in_inclusion || toward_inclusion);
        } else if kind.is_symlink() {
            gix::index::entry::Mode::SYMLINK
        } else if kind.is_file() {
            gix::index::entry::Mode::FILE
        } else {
            return Ok(false);
        };

        if !in_inclusion {
            return Ok(false);
        }

        if !dst_ignore.at_path(relpath, Some(mode))?.is_excluded() {
            return Ok(false);
        }

        let dst_path = dst_dir.join(relpath);
        let dst_parent = dst_path
            .parent()
            .context("Destination path must have parent")?;

        if !destination_dir_is_created.contains(dst_parent) {
            let missing = find_missing_directories_child_first(dst_parent)?;
            for directory in missing.into_iter().rev() {
                let source_directory = src_dir.join(directory.strip_prefix(dst_dir)?);
                let permissions = fs::symlink_metadata(source_directory)?.permissions();
                create_writable_dir(directory)?;
                created_directory_permissions.push((directory.to_owned(), permissions));
                destination_dir_is_created.insert(directory.to_owned());
            }
            destination_dir_is_created.insert(dst_parent.to_owned());
        }

        match copy_mode {
            CopyMode::Copy => copy_file(path, &dst_path),
            #[cfg(target_os = "macos")]
            CopyMode::CloneWithFallback => match clone_file(path, &dst_path) {
                Ok(()) => Ok(()),
                Err(CloneError::Unsupported) | Err(CloneError::NotSameFilesystem) => {
                    copy_mode = CopyMode::Copy;
                    copy_file(path, &dst_path)
                }
                Err(CloneError::Other(err)) => Err(err),
            },
        }?;

        Ok(false)
    };

    let mut result = walk(src_dir, &mut visit);

    // Creation order is parent-first; restore children before making parents read-only.
    // Also restore permissions after partial failure, without changing existing directories.
    for (directory, permissions) in created_directory_permissions.into_iter().rev() {
        let restore = fs::set_permissions(&directory, permissions)
            .with_context(|| format!("Failed to restore permissions on '{}'", directory.display()));
        result = result.and(restore);
    }

    result
}

/// Recursively walk that visits every file under `path` using `visit`.
///
/// If `visit` returns `Ok(true)` for a `path`, `walk` treats that `path` as a directory to walk
/// into. Otherwise, `walk` stops at `path`.
fn walk(path: &Path, visit: &mut impl FnMut(&Path) -> anyhow::Result<bool>) -> anyhow::Result<()> {
    if !visit(path)? {
        return Ok(());
    }

    for entry in
        fs::read_dir(path).with_context(|| format!("Failed to read '{}'", path.display()))?
    {
        let entry = entry?;
        walk(&entry.path(), visit)?;
    }

    Ok(())
}

/// Create a directory that's writeable for the current user.
fn create_writable_dir(directory: &Path) -> Result<(), anyhow::Error> {
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = fs::DirBuilder::new();
    builder.create(directory)?;
    Ok(())
}

fn find_missing_directories_child_first(path: &Path) -> anyhow::Result<Vec<&Path>> {
    let mut missing = Vec::new();
    for parent in path.ancestors() {
        match fs::symlink_metadata(parent) {
            Ok(metadata) => {
                anyhow::ensure!(metadata.is_dir(), "Destination parent must be a directory");
                break;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => missing.push(parent),
            Err(err) => return Err(err.into()),
        }
    }
    Ok(missing)
}

#[cfg(unix)]
fn copy_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if source.is_symlink() {
        let target = fs::read_link(source)?;
        std::os::unix::fs::symlink(&target, destination)?;
    } else if source.is_file() {
        fs::copy(source, destination)?;
    } else {
        bail!(
            "Invalid source type '{}' for copy_file",
            source
                .metadata()
                .map(|meta| format!("{:?}", meta.file_type()))
                .unwrap_or("UNKNOWN".to_string())
        )
    }

    Ok(())
}

#[cfg(windows)]
fn copy_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
    use std::os::windows::fs::{FileTypeExt, symlink_dir, symlink_file};

    // Classify the link itself so dangling links retain their file/directory kind.
    let kind = fs::symlink_metadata(source)?.file_type();
    if kind.is_symlink_dir() {
        symlink_dir(fs::read_link(source)?, destination)?;
    } else if kind.is_symlink_file() {
        symlink_file(fs::read_link(source)?, destination)?;
    } else if kind.is_file() {
        fs::copy(source, destination)?;
    } else {
        bail!("Unsupported source type for copying '{}'", source.display());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
enum CloneError {
    /// Cloning is not supported on the underlying file system. Corresponds to ENOTSUP.
    Unsupported,
    /// Source and destination reside on different file systems. Corresponds to EXDEV.
    NotSameFilesystem,
    /// All other errors that might occur while cloning for which we have no particular handling.
    Other(anyhow::Error),
}

/// COW-clone a file or symlink without following it. Destination must not exist.
/// Source and destination must share a filesystem supporting COW; no full-copy fallback.
#[cfg(target_os = "macos")]
fn clone_file(source: &Path, destination: &Path) -> Result<(), CloneError> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source =
        CString::new(source.as_os_str().as_bytes()).map_err(|err| CloneError::Other(err.into()))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|err| CloneError::Other(err.into()))?;
    // Defined by <sys/clonefile.h>, but not exported by libc.
    const CLONE_NOFOLLOW: u32 = 0x0001;
    // SAFETY: Both pointers reference live, NUL-terminated path strings. CLONE_NOFOLLOW
    // clones symlinks themselves, including dangling links, rather than their targets.
    let result = unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), CLONE_NOFOLLOW) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(code) if code == libc::ENOTSUP => Err(CloneError::Unsupported),
            Some(code) if code == libc::EXDEV => Err(CloneError::NotSameFilesystem),
            _ => Err(CloneError::Other(err.into())),
        }?
    }
    Ok(())
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn copying_regular_files_is_independent_and_rejects_directories() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::write(&source, "original")?;
        copy_file(&source, &destination)?;
        assert_eq!(
            fs::read(&destination)?,
            b"original",
            "copy preserves contents"
        );
        fs::write(&destination, "changed")?;
        assert_eq!(
            fs::read(&source)?,
            b"original",
            "copy has independent contents"
        );
        assert!(
            copy_file(temp.path(), &temp.path().join("directory-copy")).is_err(),
            "directories must be traversed, not copied as files"
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn copying_windows_symlinks_preserves_targets_and_kinds() -> anyhow::Result<()> {
        use std::os::windows::fs::{FileTypeExt, symlink_dir, symlink_file};

        // Requires Developer Mode or permission to create symlinks, like Git's symlink tests.
        let temp = tempfile::tempdir()?;
        fs::write(temp.path().join("file"), "contents")?;
        fs::create_dir(temp.path().join("directory"))?;
        for (name, target, directory) in [
            ("file-link", "file", false),
            ("directory-link", "directory", true),
            ("dangling-file-link", "missing-file", false),
            ("dangling-directory-link", "missing-directory", true),
        ] {
            let source = temp.path().join(name);
            let destination = temp.path().join(format!("{name}-copy"));
            if directory {
                symlink_dir(target, &source)?;
            } else {
                symlink_file(target, &source)?;
            }
            copy_file(&source, &destination)?;
            assert_eq!(
                fs::read_link(&destination)?,
                Path::new(target),
                "relative targets must be preserved without following links"
            );
            let kind = fs::symlink_metadata(&destination)?.file_type();
            assert_eq!(
                kind.is_symlink_dir(),
                directory,
                "directory link kind is preserved"
            );
            assert_eq!(
                kind.is_symlink_file(),
                !directory,
                "file link kind is preserved"
            );
        }
        Ok(())
    }
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
