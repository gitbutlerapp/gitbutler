use std::{fs, path::Path};

use anyhow::{Context as _, bail, ensure};
use bstr::BString;

/// Create a linked worktree without checkout, then clone the primary worktree's files and
/// copy its index. `repo` must be the primary repository and `base` must be its HEAD.
///
/// Only supported on macOS filesystems implementing clonefile(2). Every entry except the
/// root `.git` is included, even ignored files; symlinks are not followed. Special files
/// are refused. Concurrent writers must be avoided as this is not an atomic snapshot.
/// On cloning/index failure, remove the new worktree and branch, reporting cleanup errors.
pub fn add_cow(
    repo: &gix::Repository,
    path: &Path,
    branch: &gix::refs::FullNameRef,
    base: gix::ObjectId,
) -> anyhow::Result<BString> {
    ensure!(
        cfg!(target_os = "macos"),
        "--cow is only supported on macOS"
    );
    ensure!(
        repo.git_dir() == repo.common_dir(),
        "Cloning requires the primary worktree"
    );
    ensure!(
        repo.head_id()?.detach() == base,
        "Cloning requires the primary worktree's HEAD"
    );
    let source = fs::canonicalize(
        repo.workdir()
            .context("The primary repository has no worktree")?,
    )?;
    // Normally the destination is under .git, which is excluded from the walk. Refuse other
    // descendants (e.g. an unusual separate git-dir inside the worktree) to avoid recursion.
    let destination = gix::path::realpath(path)?;
    ensure!(
        !destination.starts_with(&source) || destination.starts_with(source.join(".git")),
        "The clone destination must be outside the primary worktree or inside its .git directory"
    );
    let mut index = index_for_clone(repo)?;
    let name = super::add_inner(repo, path, branch, base, true)?;
    let mut populate = || -> anyhow::Result<()> {
        clone_directory(&source, path, true)?;
        index.set_path(super::open_worktree_repo(repo, name.as_ref())?.index_path());
        index
            .write(gix::index::write::Options {
                // Filesystem caches refer to the source, not this checkout.
                extensions: gix::index::write::Extensions::None,
                ..Default::default()
            })
            .context("Failed to write the cloned worktree's index")?;
        Ok(())
    };
    if let Err(err) = populate() {
        let cleanup = || -> anyhow::Result<()> {
            super::remove(repo, path, true)?;
            let reference = repo.find_reference(branch)?;
            ensure!(
                reference.id().detach() == base,
                "The new branch moved; leaving it intact"
            );
            reference.delete()?;
            Ok(())
        };
        if let Err(cleanup_err) = cleanup() {
            return Err(err.context(format!(
                "Cloning failed; cleanup of '{}' also failed: {cleanup_err:#}",
                path.display()
            )));
        }
        return Err(err);
    }
    Ok(name)
}

fn index_for_clone(repo: &gix::Repository) -> anyhow::Result<gix::index::File> {
    // gix dissolves split indexes on read, so the destination does not depend on the source's
    // sharedindex files. Keep staged entries, conflicts, intent-to-add and skip-worktree bits.
    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    for entry in index.entries_mut() {
        entry.stat = Default::default();
        entry
            .flags
            .remove(gix::index::entry::Flags::FSMONITOR_VALID);
    }
    Ok(index)
}

fn clone_directory(source: &Path, destination: &Path, root: bool) -> anyhow::Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("Failed to read '{}'", source.display()))?
    {
        let entry = entry?;
        if root && entry.file_name() == ".git" {
            continue;
        }
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            fs::create_dir(&destination)
                .with_context(|| format!("Failed to create '{}'", destination.display()))?;
            clone_directory(&source, &destination, false)?;
            // Apply permissions after populating read-only directories.
            fs::set_permissions(&destination, entry.metadata()?.permissions())?;
        } else if kind.is_file() || kind.is_symlink() {
            clone_file(&source, &destination).with_context(|| {
                format!(
                    "Failed to clone '{}' to '{}'",
                    source.display(),
                    destination.display()
                )
            })?;
        } else {
            bail!("Cannot clone special file '{}'", source.display());
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
fn clone_file(_source: &Path, _destination: &Path) -> anyhow::Result<()> {
    bail!("--cow is only supported on macOS")
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_testsupport::invoke_bash_at_dir;

    #[test]
    fn no_checkout_and_index_copy_preserve_split_index_entries() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        invoke_bash_at_dir(
            r#"
            git init -q source
            cd source
            git config user.name Test
            git config user.email test@example.com
            printf 'original\n' > tracked
            printf 'delete\n' > deleted
            printf 'skipped\n' > skipped
            git add .
            git commit -qm initial
            printf 'staged\n' > tracked
            git add tracked
            printf 'unstaged\n' >> tracked
            git rm -q deleted
            printf 'intent\n' > intent
            git add -N intent
            git update-index --skip-worktree skipped
            blob=$(git rev-parse HEAD:tracked)
            printf '100644 %s 1\tconflicted\n100644 %s 2\tconflicted\n100644 %s 3\tconflicted\n' "$blob" "$blob" "$blob" | git update-index --index-info
            git update-index --split-index
            "#,
            tmp.path(),
        );
        let repo = gix::open(tmp.path().join("source"))?;
        let branch: &gix::refs::FullNameRef = "refs/heads/cloned".try_into()?;
        let destination = tmp.path().join("cloned");
        let name =
            super::super::add_inner(&repo, &destination, branch, repo.head_id()?.detach(), true)?;
        assert!(
            destination.join(".git").is_file(),
            "Git registered the worktree"
        );
        assert!(
            !destination.join("tracked").exists(),
            "no checkout took place"
        );
        let mut index = index_for_clone(&repo)?;
        let original = repo.index()?;
        assert_eq!(
            index.entries().len(),
            original.entries().len(),
            "split index was fully expanded"
        );
        for (cloned, original) in index.entries().iter().zip(original.entries()) {
            assert_eq!(cloned.id, original.id, "staged blob IDs survive");
            assert_eq!(
                cloned.flags, original.flags,
                "intent-to-add, skip-worktree and conflict stage flags survive"
            );
        }
        index.set_path(super::super::open_worktree_repo(&repo, name.as_ref())?.index_path());
        index.write(gix::index::write::Options {
            extensions: gix::index::write::Extensions::None,
            ..Default::default()
        })?;
        invoke_bash_at_dir(
            r#"
            git -C source ls-files --stage -v > before
            # Removing the shared index proves the copied index is self-contained.
            rm source/.git/sharedindex.*
            git -C cloned ls-files --stage -v > after
            diff -u before after
            "#,
            tmp.path(),
        );
        Ok(())
    }
}
