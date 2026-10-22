use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use bstr::{BStr, BString, ByteSlice as _};
use gix::merge::tree::TreatAsUnresolved;

use crate::index::{ConflictStage, Edit, Stat};

/// Turn the worktree of `repo`, which matched `baseline` (in Git form) when it was snapshotted,
/// into `target`, writing only the entries that differ.
///
/// Files are written through the repository's worktree filters, so clean/smudge drivers like
/// git-crypt or Git LFS see the same conversions as with Git.
/// `.git/index` is updated for the written and deleted paths only, keeping all other entries as they are.
/// If `conflicts` is set, its unresolved conflicts are recorded as index stages.
/// Like Git, it refuses before changing anything if a path it would write or delete has staged
/// content that differs from `head`, the tree the checkout leaves, and isn't in the worktree either.
/// Paths the worktree already has as `new_head` has them aren't written, but their index entries
/// follow `new_head` unless the user staged something there.
pub(super) fn write_to_worktree(
    repo: &gix::Repository,
    head: gix::ObjectId,
    new_head: gix::ObjectId,
    baseline: gix::ObjectId,
    target: gix::ObjectId,
    conflicts: Option<&gix::merge::tree::Outcome<'_>>,
) -> anyhow::Result<()> {
    let workdir = repo.workdir().context("checkout needs a worktree")?;
    let head = repo.index_from_tree(&head)?;
    let new_head = repo.index_from_tree(&new_head)?;
    let baseline = repo.index_from_tree(&baseline)?;
    let mut target = repo.index_from_tree(&target)?;

    // gix's checkout skips entries flagged `SKIP_WORKTREE`, so only changed entries are written.
    // The same flag later tells which entries to record in the index.
    let mut num_to_write = 0;
    for (entry, path) in target.entries_mut_with_paths() {
        let unchanged = baseline
            .entry_by_path(path)
            .is_some_and(|b| b.id == entry.id && b.mode == entry.mode);
        if unchanged {
            entry.flags.insert(gix::index::entry::Flags::SKIP_WORKTREE);
        } else {
            num_to_write += 1;
        }
    }
    let deleted: BTreeSet<BString> = baseline
        .entries()
        .iter()
        .map(|entry| entry.path(&baseline))
        .filter(|path| target.entry_by_path(path).is_none())
        .map(ToOwned::to_owned)
        .collect();

    // Paths the worktree already has as `new_head` has them aren't written, but their index entries
    // follow `new_head`, unless the user staged something there or the entry already matches.
    let current_index = repo.index_or_empty()?;
    let same = |a: Option<&gix::index::Entry>, b: &gix::index::Entry| {
        a.is_some_and(|a| a.id == b.id && a.mode == b.mode)
    };
    let already_in_worktree: BTreeMap<BString, (gix::ObjectId, gix::index::entry::Mode)> = target
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        })
        .filter(|entry| {
            let path = entry.path(&target);
            let current = current_index.entry_by_path(path);
            let nothing_staged = match (current, head.entry_by_path(path)) {
                (None, None) => true,
                (Some(current), head) => same(head, current),
                (None, Some(_)) => false,
            };
            nothing_staged
                && same(new_head.entry_by_path(path), entry)
                && !current.is_some_and(|current| same(Some(entry), current))
        })
        .map(|entry| (entry.path(&target).to_owned(), (entry.id, entry.mode)))
        .collect();
    drop(current_index);

    refuse_to_lose_staged_content(
        repo,
        &head,
        &baseline,
        &target,
        &already_in_worktree,
        &deleted,
        conflicts,
    )?;

    let mut path_check = gix::status::plumbing::SymlinkCheck::new(workdir.to_owned());
    refuse_to_overwrite_newer_edits(repo, &mut path_check, &baseline, &target, &deleted)?;
    for path in &deleted {
        remove_from_worktree(&mut path_check, path.as_ref())?;
    }
    if num_to_write > 0 {
        unlink_hard_linked_files(&mut path_check, &target)?;
        let mut opts =
            repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
        opts.overwrite_existing = true;
        // Without `keep_going`, files that fail to be written, or that a filter delays and never
        // delivers, fail the checkout here, before the index records them.
        gix::worktree::state::checkout(
            &mut target,
            workdir,
            repo.objects.clone().into_arc()?,
            &gix::progress::Discard,
            &gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED,
            opts,
        )?;
    }

    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    let mut edit = Edit::new(&mut index);
    // These files weren't written, so their stat may postdate the snapshot their ids come from.
    for (path, (id, mode)) in &already_in_worktree {
        edit.upsert(path.as_ref(), *id, *mode, Stat::Unknown);
    }
    for path in &deleted {
        edit.remove(path.as_ref());
    }
    for entry in target.entries() {
        if !entry
            .flags
            .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        {
            edit.upsert(
                entry.path(&target),
                entry.id,
                entry.mode,
                Stat::Known(entry.stat),
            );
        }
    }
    for conflict in conflicts
        .into_iter()
        .flat_map(|outcome| outcome.conflicts.iter())
        .filter(|conflict| conflict.is_unresolved(TreatAsUnresolved::git()))
    {
        let [base, ours, theirs] = conflict.entries();
        for (stage, entry) in [
            (ConflictStage::Base, base),
            (ConflictStage::Ours, ours),
            (ConflictStage::Theirs, theirs),
        ] {
            if let Some(entry) = entry {
                edit.set_conflict_stage(
                    conflict.ours.location(),
                    stage,
                    entry.mode.into(),
                    entry.id,
                );
            }
        }
    }
    if edit.finish() {
        index.write(Default::default())?;
    }
    Ok(())
}

/// Delete `rela_path` and the directories it leaves empty.
///
/// Like Git's checkout, delete nothing if a directory leading to `rela_path` is missing or a
/// symlink, as the file then isn't in the worktree, and deleting through the symlink would reach
/// outside of it.
fn remove_from_worktree(
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    rela_path: &BStr,
) -> anyhow::Result<()> {
    let workdir = path_check.inner.root().to_owned();
    let path = match path_check.verified_path(rela_path) {
        Ok(path) => path.to_owned(),
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound || is_symlinked_parent_error(&err) =>
        {
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    match std::fs::symlink_metadata(&path) {
        // On Windows, a symlink to a directory must be removed like a directory.
        Ok(md) if md.is_symlink() => gix::fs::symlink::remove(&path)?,
        Ok(md) if md.is_dir() => {
            // A submodule or a directory that already became one; only remove it when empty.
            std::fs::remove_dir(&path).ok();
        }
        Ok(_) => std::fs::remove_file(&path)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let mut dir = path.parent();
    while let Some(parent) = dir.filter(|dir| *dir != workdir.as_path()) {
        if std::fs::remove_dir(parent).is_err() {
            break;
        }
        dir = parent.parent();
    }
    Ok(())
}

/// `SymlinkCheck` reports a symlinked parent directory as its only validation error.
fn is_symlinked_parent_error(err: &std::io::Error) -> bool {
    err.get_ref()
        .is_some_and(|source| gix::error::classify(source).is_validation())
}

/// Fail if the index has content the user staged, which differs from `head` and isn't in the
/// worktree (`baseline`) either, at a path whose index entry the update would replace: a path that
/// is written with other content, `deleted`, recorded as one of the unresolved `conflicts`, turned
/// into a directory by a written entry, or below a file that is written.
fn refuse_to_lose_staged_content(
    repo: &gix::Repository,
    head: &gix::index::State,
    baseline: &gix::index::State,
    target: &gix::index::State,
    already_in_worktree: &BTreeMap<BString, (gix::ObjectId, gix::index::entry::Mode)>,
    deleted: &BTreeSet<BString>,
    conflicts: Option<&gix::merge::tree::Outcome<'_>>,
) -> anyhow::Result<()> {
    let index = repo.index_or_empty()?;
    let same = |a: Option<&gix::index::Entry>, b: &gix::index::Entry| {
        a.is_some_and(|a| a.id == b.id && a.mode == b.mode)
    };
    let written = |path: &BStr| {
        target.entry_by_path(path).filter(|entry| {
            !entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
                || already_in_worktree.contains_key(path)
        })
    };
    let conflicted: BTreeSet<&BStr> = conflicts
        .into_iter()
        .flat_map(|outcome| outcome.conflicts.iter())
        .filter(|conflict| conflict.is_unresolved(TreatAsUnresolved::git()))
        .map(|conflict| conflict.ours.location())
        .collect();
    let replaced = |path: &BStr, entry: &gix::index::Entry| {
        let rewritten = written(path).is_some_and(|written| !same(Some(written), entry));
        let mut below = path.to_owned();
        below.push(b'/');
        let becomes_directory = target
            .prefixed_entries(below.as_ref())
            .is_some_and(|entries| entries.iter().any(|e| written(e.path(target)).is_some()));
        let below_written_file = path
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/')
            .any(|(at, _)| written(&path[..at]).is_some());
        rewritten
            || deleted.contains(path)
            || conflicted.contains(path)
            || becomes_directory
            || below_written_file
    };
    let mut overwritten: Vec<_> = index
        .entries()
        .iter()
        .filter(|entry| {
            let path = entry.path(&index);
            !same(head.entry_by_path(path), entry)
                && !same(baseline.entry_by_path(path), entry)
                && replaced(path, entry)
        })
        .map(|entry| format!("{:?}", entry.path(&index)))
        .collect();
    if !overwritten.is_empty() {
        overwritten.sort();
        overwritten.dedup();
        but_error::bail_precondition!(
            "Staged changes would be overwritten by checkout: {}",
            overwritten.join(", ")
        );
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use gix::object::tree::EntryKind;

    #[test]
    fn deletions_do_not_follow_a_symlinked_parent_directory() -> anyhow::Result<()> {
        let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&outside)?;
        std::fs::write(outside.join("f"), "outside")?;
        let repo = gix::init(tmp.path().join("repo"))?;

        let blob = repo.write_blob("tracked")?.detach();
        let mut editor = repo.empty_tree().edit()?;
        editor.upsert("dir/f", EntryKind::Blob, blob)?;
        let baseline = editor.write()?.detach();
        let target = repo.empty_tree().id;
        // The worktree had `dir/f` when it was snapshotted, then `dir` was replaced by a symlink.
        std::os::unix::fs::symlink("../outside", repo.workdir().unwrap().join("dir"))?;

        super::write_to_worktree(&repo, baseline, target, baseline, target, None)?;

        assert!(
            outside.join("f").is_file(),
            "like Git, a file behind a symlinked directory isn't in the worktree, so it isn't deleted"
        );
        Ok(())
    }
}

/// Remove the files about to be replaced by `target` that share their content with other hard links.
///
/// gix overwrites an existing file in place, which would change all of its links. Like Git, which
/// writes a new file, this leaves the other links with their content.
#[cfg(unix)]
fn unlink_hard_linked_files(
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    target: &gix::index::State,
) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    for entry in target.entries() {
        if entry
            .flags
            .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        {
            continue;
        }
        let Ok(path) = path_check.verified_path(entry.path(target)) else {
            continue;
        };
        if std::fs::symlink_metadata(path).is_ok_and(|md| md.is_file() && md.nlink() > 1) {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn unlink_hard_linked_files(
    _path_check: &mut gix::status::plumbing::SymlinkCheck,
    _target: &gix::index::State,
) -> anyhow::Result<()> {
    Ok(())
}

/// Fail if a file that is about to be written or `deleted` changed after the worktree was
/// snapshotted into `baseline`, so the checkout doesn't replace an edit it never saw.
/// Like Git, a change of type or executable bit counts, and files are hashed through the worktree
/// filters, like the snapshot. A missing file loses nothing. Files `baseline` doesn't have that a
/// written path would replace appeared after the snapshot, unless they're ignored, which Git
/// replaces too.
fn refuse_to_overwrite_newer_edits(
    repo: &gix::Repository,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    baseline: &gix::index::State,
    target: &gix::index::State,
    deleted: &BTreeSet<BString>,
) -> anyhow::Result<()> {
    let (mut pipeline, index) = repo.filter_pipeline(None)?;
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;
    let fs = repo.filesystem_options()?;
    let current_index = repo.index_or_empty()?;
    let stat_options = repo
        .checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?
        .stat_options;
    let mut buf = Vec::new();
    let written: Vec<&BStr> = target
        .entries()
        .iter()
        .filter(|entry| {
            !entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        })
        .map(|entry| entry.path(target))
        .collect();
    let workdir = path_check.inner.root().to_owned();
    let mut changed = Vec::new();
    for rela_path in &written {
        collect_files_created_in_the_way(
            &workdir,
            rela_path,
            baseline,
            &mut excludes,
            &mut changed,
        )?;
    }
    for rela_path in written
        .iter()
        .copied()
        .chain(deleted.iter().map(|path| path.as_ref()))
    {
        let Ok(path) = path_check.verified_path(rela_path) else {
            continue;
        };
        let Ok(md) = gix::index::fs::Metadata::from_path_no_follow(path) else {
            continue;
        };
        let Some(expected) = baseline.entry_by_path(rela_path) else {
            continue;
        };
        if expected.mode == gix::index::entry::Mode::COMMIT {
            continue;
        }
        if expected
            .mode
            .change_to_match_fs(&md, fs.symlink, fs.executable_bit)
            .is_some()
        {
            changed.push(format!("{rela_path:?}"));
            continue;
        }
        let unchanged_since_indexed = current_index.entry_by_path(rela_path).is_some_and(|entry| {
            entry.id == expected.id
                && !entry.stat.is_racy(current_index.timestamp(), stat_options)
                && gix::index::entry::Stat::from_fs(&md)
                    .is_ok_and(|on_disk| entry.stat.matches(&on_disk, stat_options))
        });
        if unchanged_since_indexed {
            continue;
        }
        crate::worktree::worktree_file_to_git_in_buf(
            &mut buf,
            &md,
            rela_path,
            path,
            &mut pipeline,
            &index,
        )?;
        let on_disk = gix::objs::compute_hash(repo.object_hash(), gix::object::Kind::Blob, &buf)?;
        if on_disk != expected.id {
            changed.push(format!("{rela_path:?}"));
        }
    }
    if !changed.is_empty() {
        changed.sort();
        changed.dedup();
        but_error::bail_precondition!(
            "Uncommitted files would be overwritten by checkout: {}",
            changed.join(", ")
        );
    }
    Ok(())
}

/// Collect files and symlinks that the snapshot `baseline` doesn't have, that aren't ignored, and
/// that writing `rela_path` would remove: a leading component that isn't a directory, the path
/// itself, or anything below it if it's a directory.
fn collect_files_created_in_the_way(
    workdir: &std::path::Path,
    rela_path: &BStr,
    baseline: &gix::index::State,
    excludes: &mut gix::AttributeStack<'_>,
    changed: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut is_new = |rela_path: &BStr| -> anyhow::Result<bool> {
        Ok(baseline.entry_by_path(rela_path).is_none()
            && !excludes.at_entry(rela_path, None)?.is_excluded())
    };
    let metadata = |rela_path: &BStr| -> anyhow::Result<Option<std::fs::Metadata>> {
        match std::fs::symlink_metadata(workdir.join(gix::path::from_bstr(rela_path)?)) {
            Ok(md) => Ok(Some(md)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    };
    for (at, _) in rela_path
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'/')
    {
        let ancestor: &BStr = rela_path[..at].as_ref();
        match metadata(ancestor)? {
            Some(md) if md.is_dir() => continue,
            Some(_) => {
                if is_new(ancestor)? {
                    changed.push(format!("{ancestor:?}"));
                }
                return Ok(());
            }
            None => return Ok(()),
        }
    }
    let mut pending = vec![rela_path.to_owned()];
    while let Some(rela_path) = pending.pop() {
        let Some(md) = metadata(rela_path.as_ref())? else {
            continue;
        };
        if !is_new(rela_path.as_ref())? {
            continue;
        }
        if md.is_dir() {
            for entry in
                std::fs::read_dir(workdir.join(gix::path::from_bstr(rela_path.as_bstr())?))?
            {
                let mut below = rela_path.clone();
                below.push(b'/');
                below.extend_from_slice(gix::path::os_str_into_bstr(&entry?.file_name())?);
                pending.push(below);
            }
        } else {
            changed.push(format!("{rela_path:?}"));
        }
    }
    Ok(())
}
