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
use std::{ffi::OsStr, path::Path};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString, ByteSlice};
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

    let worktree_repo = gix::open(path)?;

    worktree_repo
        .worktree()
        .context("git registered the new checkout as linked worktree")?;

    handle_worktreeinclude(repo, worktree_repo)?;

    gix::open(path)?
        .worktree()
        .and_then(|worktree| worktree.id().map(ToOwned::to_owned))
        .context("git registered the new checkout as a linked worktree")
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
fn handle_worktreeinclude(
    repo: &gix::Repository,
    worktree_repo: gix::Repository,
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

    let worktree_include = fs::read(&worktree_include_path)?;
    let worktree_include_roots: Vec<_> = gix::ignore::parse(&worktree_include, false)
        .filter_map(|(pattern, _, _)| {
            if !pattern.has_wildcard() && !pattern.is_negative() {
                Some(pattern.text)
            } else {
                None
            }
        })
        .filter_map(|root| gix::path::try_from_bstr(&root).ok().map(PathBuf::from))
        .collect();

    let src_index = repo.index()?;
    let dst_index = worktree_repo.index()?;

    let mut dst_ignore = repo.excludes(
        &dst_index,
        None,
        gix::worktree::stack::state::ignore::Source::IdMapping,
    )?;

    // Note: May result in high memory usage with 100s of thousands of files to copy, consider
    // making lazy instead.
    let mut files_to_copy = vec![];
    let mut visit = |path: &Path| {
        let is_special_git_file = path
            .file_name()
            .map(|name| name == ".git" || name == ".gitignore" || name == ".gitattributes")
            .unwrap_or_default();
        if is_special_git_file {
            return Ok(false);
        }

        let relpath = path.strip_prefix(src_dir)?;
        let index_path = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(relpath));
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

        let metadata = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => bail!(err),
        };
        let kind = metadata.file_type();

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

        let ignored_in_dst = dst_ignore.at_path(relpath, Some(mode))?.is_excluded();

        // A relpath in the source might be a path prefix of a tracked file in the
        // destination.
        let mut index_prefix = index_path.into_owned();
        index_prefix.push(b'/');

        if ignored_in_dst && dst_index.prefixed_entries(index_prefix.as_bstr()).is_none() {
            files_to_copy.push(relpath.to_owned());
        }

        Ok(false)
    };

    walk(src_dir, &mut visit)?;

    #[cfg(target_os = "macos")]
    let cloning_supported = is_cloning_supported(src_dir, dst_dir);

    let mut dir_is_created = HashSet::new();
    for relpath in files_to_copy {
        let src_path = src_dir.join(&relpath);
        let src_parent = src_path.parent().context("Source path must have parent")?;
        let dst_path = dst_dir.join(&relpath);
        let dst_parent = dst_path
            .parent()
            .context("Destination path must have parent")?;

        if !dir_is_created.contains(dst_parent) {
            fs::create_dir_all(dst_parent)?;
            let permissions = fs::metadata(src_parent)?.permissions();

            // Note: A shortcoming here is that we don't necessarily preserve permissions for
            // intermediate parent directories. This seems mostly fine as at the very least we set
            // the correct permissions for any directory that contains files that we copy (which may
            // or may not be sensitive).
            fs::set_permissions(dst_parent, permissions)?;
            dir_is_created.insert(dst_parent.to_owned());
        }

        #[cfg(not(target_os = "macos"))]
        copy_file(&src_path, &dst_path)?;
        #[cfg(target_os = "macos")]
        if cloning_supported {
            clone_file(&src_path, &dst_path)?;
        } else {
            copy_file(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Recursively walk directory, visiting every file with `visit`.
///
/// If `visit` returns `Ok(true)` for a `path`, `walk` treats that `path` as a directory to walk
/// into. Otherwise, `walk` terminates.
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

#[cfg(unix)]
fn copy_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
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

/// Best-effort, non-mutating preflight for cloning between existing worktree directories.
/// Unknown capabilities or failed checks select ordinary copying. This cannot guarantee that
/// individual files can be cloned: permissions, nested mounts, and filesystem state can differ
/// from these roots or change after this check. Clone failures must still reach the caller.
#[cfg(target_os = "macos")]
fn is_cloning_supported(source: &Path, destination: &Path) -> bool {
    use std::{
        ffi::CString,
        mem::{MaybeUninit, size_of},
        os::unix::{ffi::OsStrExt, fs::MetadataExt},
    };

    let (Ok(source_metadata), Ok(destination_metadata)) =
        (fs::metadata(source), fs::metadata(destination))
    else {
        return false;
    };
    if !source_metadata.is_dir()
        || !destination_metadata.is_dir()
        || source_metadata.dev() != destination_metadata.dev()
    {
        return false;
    }
    let (Ok(source), Ok(destination)) = (
        CString::new(source.as_os_str().as_bytes()),
        CString::new(destination.as_os_str().as_bytes()),
    ) else {
        return false;
    };

    // SAFETY: Both C strings remain alive and are NUL-terminated. access() does not retain them.
    if unsafe { libc::access(source.as_ptr(), libc::R_OK | libc::X_OK) } != 0
        || unsafe { libc::access(destination.as_ptr(), libc::W_OK | libc::X_OK) } != 0
    {
        return false;
    }

    let mut filesystem = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: statfs receives a valid C string and storage for one statfs value.
    if unsafe { libc::statfs(destination.as_ptr(), filesystem.as_mut_ptr()) } != 0 {
        return false;
    }
    // SAFETY: Successful statfs initialized the output structure.
    let filesystem = unsafe { filesystem.assume_init() };
    if filesystem.f_flags & libc::MNT_RDONLY as u32 != 0 {
        return false;
    }

    // getattrlist packs the buffer length followed by requested attributes at 4-byte alignment.
    #[repr(C)]
    struct VolumeCapabilities {
        length: u32,
        volume: libc::vol_capabilities_attr_t,
    }
    let mut attributes = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_CAPABILITIES,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut capabilities = VolumeCapabilities {
        length: 0,
        volume: libc::vol_capabilities_attr_t {
            capabilities: [0; 4],
            valid: [0; 4],
        },
    };
    // SAFETY: The request selects only volume capabilities. The C-layout output buffer holds
    // its length and that attribute, with the alignment and size required by getattrlist.
    let result = unsafe {
        libc::getattrlist(
            destination.as_ptr(),
            (&mut attributes as *mut libc::attrlist).cast(),
            (&mut capabilities as *mut VolumeCapabilities).cast(),
            size_of::<VolumeCapabilities>(),
            0,
        )
    };
    if result != 0 || capabilities.length as usize != size_of::<VolumeCapabilities>() {
        return false;
    }
    let interfaces = libc::VOL_CAPABILITIES_INTERFACES;
    capabilities.volume.valid[interfaces] & libc::VOL_CAP_INT_CLONE != 0
        && capabilities.volume.capabilities[interfaces] & libc::VOL_CAP_INT_CLONE != 0
}

/// COW-clone a file or symlink without following it. Destination must not exist.
/// Source and destination must share a filesystem supporting COW; no full-copy fallback.
#[cfg(target_os = "macos")]
pub fn clone_file(source: &Path, destination: &Path) -> anyhow::Result<()> {
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

    #[cfg(target_os = "macos")]
    #[test]
    fn cloning_preflight_rejects_missing_paths_and_non_directories() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let file = temp.path().join("file");
        let missing = temp.path().join("missing");
        fs::write(&file, "contents")?;
        for (source, destination) in [
            (temp.path(), missing.as_path()),
            (missing.as_path(), temp.path()),
            (temp.path(), file.as_path()),
            (file.as_path(), temp.path()),
        ] {
            assert!(
                !is_cloning_supported(source, destination),
                "both roots must be existing directories"
            );
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn advertised_cloning_support_allows_an_independent_clone() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir(&source)?;
        fs::create_dir(&destination)?;
        fs::write(source.join("file"), "original")?;
        // Test also runs on volumes without clone support; those use the copy path.
        if is_cloning_supported(&source, &destination) {
            clone_file(&source.join("file"), &destination.join("file"))?;
        } else {
            copy_file(&source.join("file"), &destination.join("file"))?;
        }
        assert_eq!(
            fs::read(destination.join("file"))?,
            b"original",
            "selected operation preserves contents"
        );
        fs::write(destination.join("file"), "changed")?;
        assert_eq!(
            fs::read(source.join("file"))?,
            b"original",
            "selected operation preserves source independence"
        );
        Ok(())
    }
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
