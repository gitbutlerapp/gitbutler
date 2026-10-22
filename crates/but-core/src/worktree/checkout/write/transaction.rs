//! Writing a checkout's changes to the worktree.

use std::collections::BTreeMap;

use bstr::{BStr, BString};

use super::{is_symlinked_parent_error, plan::Plan};
use crate::index::Stat;

/// The stat of each file checkout placed in the worktree.
#[derive(Default)]
pub(super) struct Placed {
    stats: BTreeMap<BString, gix::index::entry::Stat>,
}

impl Placed {
    /// What the index should record about the file at `path`.
    pub fn stat(&self, path: &BStr) -> Stat {
        match self.stats.get(path) {
            Some(stat) => Stat::Known(*stat),
            None => Stat::Unknown,
        }
    }
}

/// Delete and write the files `plan` changes, in place.
pub(super) fn write_in_place(
    repo: &gix::Repository,
    target: &gix::index::State,
    plan: &Plan,
) -> anyhow::Result<Placed> {
    let workdir = repo.workdir().expect("checked by the caller");
    let mut path_check = gix::status::plumbing::SymlinkCheck::new(workdir.to_owned());
    for path in plan.deletions() {
        remove_from_worktree(&mut path_check, path)?;
    }
    if plan.writes().next().is_none() {
        return Ok(Placed::default());
    }
    let mut index = checkout_index(target, plan);
    unlink_files_to_replace(&mut path_check, plan)?;
    let mut opts =
        repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    opts.overwrite_existing = true;
    // Without `keep_going`, files that fail to be written, or that a filter delays and never
    // delivers, fail the checkout here, before the index records them.
    gix::worktree::state::checkout(
        &mut index,
        workdir,
        repo.objects.clone().into_arc()?,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )?;
    Ok(placed(&index, plan))
}

/// The full `target`, with every entry `plan` doesn't write flagged so gix's checkout skips it.
/// The other entries still provide `.gitattributes` to the filters.
fn checkout_index(target: &gix::index::State, plan: &Plan) -> gix::index::State {
    let mut index = target.clone();
    for (entry, path) in index.entries_mut_with_paths() {
        if plan.write_at(path).is_none() {
            entry.flags.insert(gix::index::entry::Flags::SKIP_WORKTREE);
        }
    }
    index
}

/// The stat gix recorded for each file it wrote into `index`.
fn placed(index: &gix::index::State, plan: &Plan) -> Placed {
    let stats = plan
        .writes()
        .filter_map(|(path, _)| {
            index
                .entry_by_path(path)
                .map(|entry| (path.to_owned(), entry.stat))
        })
        .collect();
    Placed { stats }
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

/// Remove the files about to be replaced.
///
/// gix writes over an existing file in place. Like Git, which writes a new file instead, this
/// leaves other hard links with their content, and replaces read-only files the directory allows
/// to be replaced.
#[cfg(unix)]
fn unlink_files_to_replace(
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    plan: &Plan,
) -> anyhow::Result<()> {
    for (rela_path, _) in plan.writes() {
        let Ok(path) = path_check.verified_path(rela_path) else {
            continue;
        };
        if std::fs::symlink_metadata(path).is_ok_and(|md| md.is_file()) {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn unlink_files_to_replace(
    _path_check: &mut gix::status::plumbing::SymlinkCheck,
    _plan: &Plan,
) -> anyhow::Result<()> {
    Ok(())
}
