//! Refusals that keep a checkout from losing what the user has: staged content, and worktree
//! edits made after the snapshot.

use std::collections::BTreeSet;

use bstr::{BStr, BString, ByteSlice as _};

use super::{Trees, plan::Plan};

/// Fail if the index has content the user staged, which differs from `head` and isn't in the
/// worktree (`baseline`) either, at a path whose index entry checkout replaces.
pub(super) fn staged_content(
    repo: &gix::Repository,
    trees: &Trees,
    plan: &Plan,
) -> anyhow::Result<()> {
    let index = repo.index_or_empty()?;
    let lost = index
        .entries()
        .iter()
        .filter(|entry| {
            let path = entry.path(&index);
            only_staged(trees, path, entry) && replaced(plan, path, entry)
        })
        .map(|entry| format!("{:?}", entry.path(&index)))
        .collect();
    refuse_unless_empty(lost, "Staged changes would be overwritten by checkout")
}

/// Whether `entry` holds content found in neither `head` nor the worktree.
fn only_staged(trees: &Trees, path: &BStr, entry: &gix::index::Entry) -> bool {
    let same = |state: &gix::index::State| {
        state
            .entry_by_path(path)
            .is_some_and(|other| other.id == entry.id && other.mode == entry.mode)
    };
    !same(&trees.head) && !same(&trees.baseline)
}

/// Whether checkout replaces `entry` at `path`: it records other content there, deletes it,
/// records a conflict there, turns it into a directory, or writes a file in place of a
/// directory leading to it.
fn replaced(plan: &Plan, path: &BStr, entry: &gix::index::Entry) -> bool {
    let rewritten = plan
        .updated(path)
        .is_some_and(|content| content.id != entry.id || content.mode != entry.mode);
    let below_updated_file = path
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'/')
        .any(|(at, _)| plan.updated(path[..at].as_bstr()).is_some());
    rewritten
        || plan.is_deleted(path)
        || plan.is_conflicted(path)
        || plan.updates_below(path)
        || below_updated_file
}

/// Fail if a file that is about to be written or deleted changed after the worktree was
/// snapshotted into `baseline`, so the checkout doesn't replace an edit it never saw.
/// Like Git, a change of type or executable bit counts, and files are hashed through the worktree
/// filters, like the snapshot. A missing file loses nothing. Files `baseline` doesn't have that a
/// written path would replace appeared after the snapshot, unless they're ignored, which Git
/// replaces too.
pub(super) fn newer_edits(
    repo: &gix::Repository,
    baseline: &gix::index::State,
    plan: &Plan,
) -> anyhow::Result<()> {
    let workdir = repo.workdir().expect("checked by the caller").to_owned();
    let mut path_check = gix::status::plumbing::SymlinkCheck::new(workdir.clone());
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
    let written: Vec<&BStr> = plan.writes().map(|(path, _)| path).collect();
    // On a case-insensitive filesystem, a file of the snapshot can be found under another case.
    let baseline_folded: Option<BTreeSet<BString>> = fs.ignore_case.then(|| {
        baseline
            .entries()
            .iter()
            .map(|entry| entry.path(baseline).to_lowercase().into())
            .collect()
    });
    let in_baseline = |rela_path: &BStr| match &baseline_folded {
        Some(folded) => folded.contains(rela_path.to_lowercase().as_bstr()),
        None => baseline.entry_by_path(rela_path).is_some(),
    };
    let mut changed = Vec::new();
    for rela_path in &written {
        collect_files_created_in_the_way(
            &workdir,
            rela_path,
            &in_baseline,
            &mut excludes,
            &mut changed,
        )?;
    }
    for rela_path in written.iter().copied().chain(plan.deletions()) {
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
        // gix doesn't see an executable entry turn into a symlink without `core.fileMode`.
        let file_is_gone = matches!(
            expected.mode,
            gix::index::entry::Mode::FILE | gix::index::entry::Mode::FILE_EXECUTABLE
        ) && !md.is_file();
        if file_is_gone
            || expected
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
    refuse_unless_empty(
        changed,
        "Uncommitted files would be overwritten by checkout",
    )
}

/// Collect files and symlinks that aren't `in_baseline`, the snapshot, that aren't ignored, and
/// that writing `rela_path` would remove: a leading component that isn't a directory, the path
/// itself, or anything below it if it's a directory.
fn collect_files_created_in_the_way(
    workdir: &std::path::Path,
    rela_path: &BStr,
    in_baseline: &dyn Fn(&BStr) -> bool,
    excludes: &mut gix::AttributeStack<'_>,
    changed: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut is_new = |rela_path: &BStr| -> anyhow::Result<bool> {
        Ok(!in_baseline(rela_path) && !excludes.at_entry(rela_path, None)?.is_excluded())
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

/// Refuse with `message` and the sorted `paths`, unless there are none.
fn refuse_unless_empty(mut paths: Vec<String>, message: &str) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    paths.sort();
    paths.dedup();
    but_error::bail_precondition!("{message}: {}", paths.join(", "))
}
