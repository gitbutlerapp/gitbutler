use std::collections::BTreeSet;

use anyhow::Context as _;
use bstr::{BStr, BString};
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

    refuse_to_lose_staged_content(repo, &head, &baseline, &target, &deleted, conflicts)?;

    let mut path_check = gix::status::plumbing::SymlinkCheck::new(workdir.to_owned());
    for path in &deleted {
        remove_from_worktree(&mut path_check, path.as_ref())?;
    }
    if num_to_write > 0 {
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
    let same = |a: Option<&gix::index::Entry>, b: &gix::index::Entry| {
        a.is_some_and(|a| a.id == b.id && a.mode == b.mode)
    };
    let already_in_worktree: Vec<_> = target
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        })
        .filter(|entry| {
            let path = entry.path(&target);
            let current = index.entry_by_path(path);
            let nothing_staged = match (current, head.entry_by_path(path)) {
                (None, None) => true,
                (Some(current), head) => same(head, current),
                (None, Some(_)) => false,
            };
            nothing_staged && same(new_head.entry_by_path(path), entry)
        })
        .map(|entry| (entry.path(&target).to_owned(), entry.id, entry.mode))
        .collect();
    let mut edit = Edit::new(&mut index);
    for (path, id, mode) in &already_in_worktree {
        let stat = gix::index::fs::Metadata::from_path_no_follow(
            &workdir.join(gix::path::from_bstr(AsRef::<BStr>::as_ref(path))?),
        )
        .ok()
        .and_then(|md| gix::index::entry::Stat::from_fs(&md).ok())
        .map_or(Stat::Unknown, Stat::Known);
        edit.upsert(path.as_ref(), *id, *mode, stat);
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
/// into a directory, or below a file that is written.
fn refuse_to_lose_staged_content(
    repo: &gix::Repository,
    head: &gix::index::State,
    baseline: &gix::index::State,
    target: &gix::index::State,
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
            .is_some_and(|entries| !entries.is_empty());
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
