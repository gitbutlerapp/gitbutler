use std::{collections::BTreeSet, path::Path};

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
pub(super) fn write_to_worktree(
    repo: &gix::Repository,
    baseline: gix::ObjectId,
    target: gix::ObjectId,
    conflicts: Option<&gix::merge::tree::Outcome<'_>>,
) -> anyhow::Result<()> {
    let workdir = repo.workdir().context("checkout needs a worktree")?;
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

    for path in &deleted {
        remove_from_worktree(workdir, path.as_ref())?;
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
    let mut edit = Edit::new(&mut index);
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
fn remove_from_worktree(workdir: &Path, rela_path: &BStr) -> anyhow::Result<()> {
    let path = workdir.join(gix::path::from_bstr(rela_path)?);
    match std::fs::symlink_metadata(&path) {
        Ok(md) if md.is_dir() => {
            // A submodule or a directory that already became one; only remove it when empty.
            std::fs::remove_dir(&path).ok();
        }
        Ok(_) => std::fs::remove_file(&path)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let mut dir = path.parent();
    while let Some(parent) = dir.filter(|dir| *dir != workdir) {
        if std::fs::remove_dir(parent).is_err() {
            break;
        }
        dir = parent.parent();
    }
    Ok(())
}
