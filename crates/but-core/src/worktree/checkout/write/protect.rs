//! Refusals that keep a checkout from losing what the user has: staged content, and worktree
//! edits made after the snapshot.

use std::collections::BTreeMap;

use bstr::{BStr, BString, ByteSlice as _};

use super::{Trees, is_symlinked_parent_error, plan::Plan};

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

/// A filesystem object the commit moves aside to make room for the target.
pub(super) struct Clearance {
    pub rela_path: BString,
}

/// Fail if anything checkout removes from the worktree changed after the worktree was
/// snapshotted into `baseline`, so checkout doesn't replace an edit it never saw. Otherwise,
/// return the objects to move aside.
///
/// Like Git, a change of type or executable bit counts, files are hashed through the worktree
/// filters like the snapshot, and a missing file loses nothing. Files the snapshot doesn't have
/// appeared after it, unless they're ignored, which Git replaces too.
pub(super) fn worktree(
    repo: &gix::Repository,
    baseline: &gix::index::State,
    plan: &Plan,
) -> anyhow::Result<Vec<Clearance>> {
    let (pipeline, index) = repo.filter_pipeline(None)?;
    let mut judge = Judge::new(repo, baseline, &index, pipeline)?;
    let removals = objects_to_remove(&mut judge, plan)?;
    let mut changed = Vec::new();
    for rela_path in removals.iter().flat_map(|removal| &removal.judged) {
        if let Verdict::Changed(path) = judge.judge(rela_path.as_ref())? {
            changed.push(format!("{path:?}"));
        }
    }
    refuse_unless_empty(
        changed,
        "Uncommitted files would be overwritten by checkout",
    )?;
    Ok(outermost(removals))
}

/// An object checkout removes, and the files in it whose content must be the snapshot's.
struct Removal {
    rela_path: BString,
    judged: Vec<BString>,
}

/// What an object about to be removed is, compared with the snapshot.
enum Verdict {
    /// It has the snapshot's content.
    Snapshotted,
    /// The snapshot doesn't have it, and Git may replace it.
    Ignored,
    /// It's gone, so removing it loses nothing.
    Missing,
    /// It changed after the snapshot, which knows it as the given path.
    Changed(BString),
}

/// Everything checkout removes: the object at each deleted path, and whatever is in the way of
/// each written file.
fn objects_to_remove(judge: &mut Judge<'_>, plan: &Plan) -> anyhow::Result<Vec<Removal>> {
    let mut removals = Vec::new();
    for rela_path in plan.deletions() {
        removals.extend(judge.deleted_object(rela_path)?);
    }
    for (rela_path, content) in plan.writes() {
        if content.mode != gix::index::entry::Mode::COMMIT {
            removals.extend(judge.object_in_the_way(rela_path)?);
        }
    }
    Ok(removals)
}

/// The removals whose paths aren't inside another removal, which moves them along.
fn outermost(mut removals: Vec<Removal>) -> Vec<Clearance> {
    removals.sort_by(|a, b| a.rela_path.cmp(&b.rela_path));
    let mut clearances: Vec<Clearance> = Vec::new();
    for Removal { rela_path, .. } in removals {
        let inside_previous = clearances
            .last()
            .is_some_and(|previous| is_inside(rela_path.as_ref(), previous.rela_path.as_ref()));
        if !inside_previous {
            clearances.push(Clearance { rela_path });
        }
    }
    clearances
}

/// Whether `path` is `dir` or below it.
fn is_inside(path: &BStr, dir: &BStr) -> bool {
    path == dir || (path.starts_with(dir) && path.get(dir.len()) == Some(&b'/'))
}

/// What judging worktree objects against the snapshot needs.
struct Judge<'a> {
    repo: &'a gix::Repository,
    workdir: std::path::PathBuf,
    path_check: gix::status::plumbing::SymlinkCheck,
    baseline: &'a gix::index::State,
    /// Baseline paths by their lowercase form, if the filesystem ignores case.
    baseline_folded: Option<BTreeMap<BString, BString>>,
    index: &'a gix::index::State,
    pipeline: gix::filter::Pipeline<'a>,
    excludes: gix::AttributeStack<'a>,
    fs: gix::fs::Capabilities,
    stat_options: gix::index::entry::stat::Options,
    buf: Vec<u8>,
}

impl<'a> Judge<'a> {
    fn new(
        repo: &'a gix::Repository,
        baseline: &'a gix::index::State,
        index: &'a gix::index::State,
        pipeline: gix::filter::Pipeline<'a>,
    ) -> anyhow::Result<Self> {
        let workdir = repo.workdir().expect("checked by the caller").to_owned();
        let fs = repo.filesystem_options()?;
        Ok(Judge {
            path_check: gix::status::plumbing::SymlinkCheck::new(workdir.clone()),
            workdir,
            baseline_folded: fs.ignore_case.then(|| folded_paths(baseline)),
            excludes: repo.excludes(
                index,
                None,
                gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
            )?,
            stat_options: repo
                .checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?
                .stat_options,
            repo,
            baseline,
            index,
            pipeline,
            fs,
            buf: Vec::new(),
        })
    }

    /// The object at the deleted `rela_path`. Like Git, a path behind a missing or symlinked
    /// directory isn't in the worktree, and a directory, like a submodule's, only goes if empty.
    fn deleted_object(&mut self, rela_path: &BStr) -> anyhow::Result<Option<Removal>> {
        let md = match self.path_check.verified_path(rela_path) {
            Ok(path) => path.symlink_metadata(),
            Err(err) if is_not_in_worktree(&err) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let judged = match md {
            Ok(md) if md.is_dir() && !is_empty_dir(&self.path(rela_path.as_ref())?)? => {
                return Ok(None);
            }
            Ok(md) if md.is_dir() => Vec::new(),
            Ok(_) => vec![rela_path.to_owned()],
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        Ok(Some(Removal {
            rela_path: rela_path.to_owned(),
            judged,
        }))
    }

    /// What writing `rela_path` removes: a leading component that isn't a directory, or the
    /// object at the path itself, including all files below it if it's a directory.
    fn object_in_the_way(&mut self, rela_path: &BStr) -> anyhow::Result<Option<Removal>> {
        let in_the_way = match self.blocking_ancestor(rela_path)? {
            Ancestors::Directories => rela_path.to_owned(),
            Ancestors::Missing => return Ok(None),
            Ancestors::Blocked(ancestor) => ancestor,
        };
        let Some(md) = self.metadata(in_the_way.as_ref())? else {
            return Ok(None);
        };
        let judged = match md.is_dir() {
            // A submodule the snapshot has is judged as a whole.
            true if self.baseline_entry(in_the_way.as_ref()).is_some() => vec![in_the_way.clone()],
            true if self.is_ignored(in_the_way.as_ref())? => Vec::new(),
            true => self.files_below(in_the_way.as_ref())?,
            false => vec![in_the_way.clone()],
        };
        Ok(Some(Removal {
            rela_path: in_the_way,
            judged,
        }))
    }

    /// Classify the leading components of `rela_path`.
    fn blocking_ancestor(&self, rela_path: &BStr) -> anyhow::Result<Ancestors> {
        let separators = rela_path
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/');
        for (at, _) in separators {
            let ancestor = rela_path[..at].as_bstr();
            match self.metadata(ancestor)? {
                Some(md) if md.is_dir() => continue,
                Some(_) => return Ok(Ancestors::Blocked(ancestor.to_owned())),
                None => return Ok(Ancestors::Missing),
            }
        }
        Ok(Ancestors::Directories)
    }

    /// The files and symlinks below `rela_dir`, skipping ignored directories, which Git replaces.
    fn files_below(&mut self, rela_dir: &BStr) -> anyhow::Result<Vec<BString>> {
        let mut files = Vec::new();
        let mut pending = vec![rela_dir.to_owned()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(self.path(dir.as_ref())?)? {
                let entry = entry?;
                let mut below = dir.clone();
                below.push(b'/');
                below.extend_from_slice(gix::path::os_str_into_bstr(&entry.file_name())?);
                match entry.file_type()?.is_dir() {
                    true if self.is_ignored(below.as_ref())? => {}
                    true => pending.push(below),
                    false => files.push(below),
                }
            }
        }
        Ok(files)
    }

    fn judge(&mut self, rela_path: &BStr) -> anyhow::Result<Verdict> {
        let Some(md) = self.fs_metadata(rela_path)? else {
            return Ok(Verdict::Missing);
        };
        match self.baseline_entry(rela_path) {
            Some((path, expected)) => self.compare(rela_path, path, &expected, &md),
            None if self.is_ignored(rela_path)? => Ok(Verdict::Ignored),
            None => Ok(Verdict::Changed(rela_path.to_owned())),
        }
    }

    /// Compare the object at `rela_path` with `expected`, which the snapshot has at `snapshot_path`.
    fn compare(
        &mut self,
        rela_path: &BStr,
        snapshot_path: BString,
        expected: &gix::index::Entry,
        md: &gix::index::fs::Metadata,
    ) -> anyhow::Result<Verdict> {
        let same = expected.mode == gix::index::entry::Mode::COMMIT
            || (!type_or_mode_changed(expected, md, &self.fs)
                && (self.unchanged_since_indexed(rela_path, expected, md)
                    || self.content_matches(rela_path, expected, md)?));
        Ok(match same {
            true => Verdict::Snapshotted,
            false => Verdict::Changed(snapshot_path),
        })
    }

    /// The snapshot's entry for the object at `rela_path`, and its path there. On a
    /// case-insensitive filesystem, it may know the object under another case.
    fn baseline_entry(&self, rela_path: &BStr) -> Option<(BString, gix::index::Entry)> {
        let path = match (
            self.baseline.entry_by_path(rela_path),
            &self.baseline_folded,
        ) {
            (Some(_), _) => rela_path.to_owned(),
            (None, Some(folded)) => folded.get(rela_path.to_lowercase().as_bstr())?.clone(),
            (None, None) => return None,
        };
        let entry = self.baseline.entry_by_path(path.as_ref())?.clone();
        Some((path, entry))
    }

    /// Whether the index entry at `rela_path` has `expected`'s id and a stat that still
    /// matches the file, so the file can't have changed since it was hashed.
    fn unchanged_since_indexed(
        &self,
        rela_path: &BStr,
        expected: &gix::index::Entry,
        md: &gix::index::fs::Metadata,
    ) -> bool {
        self.index.entry_by_path(rela_path).is_some_and(|entry| {
            entry.id == expected.id
                && !entry
                    .stat
                    .is_racy(self.index.timestamp(), self.stat_options)
                && gix::index::entry::Stat::from_fs(md)
                    .is_ok_and(|on_disk| entry.stat.matches(&on_disk, self.stat_options))
        })
    }

    /// Whether the file at `rela_path`, cleaned like the snapshot cleaned it, is `expected`.
    fn content_matches(
        &mut self,
        rela_path: &BStr,
        expected: &gix::index::Entry,
        md: &gix::index::fs::Metadata,
    ) -> anyhow::Result<bool> {
        let path = self.path(rela_path)?;
        crate::worktree::worktree_file_to_git_in_buf(
            &mut self.buf,
            md,
            rela_path,
            &path,
            &mut self.pipeline,
            self.index,
        )?;
        let id =
            gix::objs::compute_hash(self.repo.object_hash(), gix::object::Kind::Blob, &self.buf)?;
        Ok(id == expected.id)
    }

    fn is_ignored(&mut self, rela_path: &BStr) -> anyhow::Result<bool> {
        Ok(self.excludes.at_entry(rela_path, None)?.is_excluded())
    }

    fn path(&self, rela_path: &BStr) -> anyhow::Result<std::path::PathBuf> {
        Ok(self.workdir.join(gix::path::from_bstr(rela_path)?))
    }

    fn metadata(&self, rela_path: &BStr) -> anyhow::Result<Option<std::fs::Metadata>> {
        match std::fs::symlink_metadata(self.path(rela_path)?) {
            Ok(md) => Ok(Some(md)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn fs_metadata(&self, rela_path: &BStr) -> anyhow::Result<Option<gix::index::fs::Metadata>> {
        match gix::index::fs::Metadata::from_path_no_follow(&self.path(rela_path)?) {
            Ok(md) => Ok(Some(md)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

/// The leading components of a path, as found in the worktree.
enum Ancestors {
    /// They are all directories.
    Directories,
    /// One is missing, so nothing below it exists.
    Missing,
    /// This one is a file or symlink, which a directory must replace.
    Blocked(BString),
}

/// The baseline's paths by their lowercase form.
fn folded_paths(baseline: &gix::index::State) -> BTreeMap<BString, BString> {
    baseline
        .entries()
        .iter()
        .map(|entry| entry.path(baseline))
        .map(|path| (path.to_lowercase().into(), path.to_owned()))
        .collect()
}

/// Like Git, a type change or a change of the executable bit counts as a change.
fn type_or_mode_changed(
    expected: &gix::index::Entry,
    md: &gix::index::fs::Metadata,
    fs: &gix::fs::Capabilities,
) -> bool {
    // gix doesn't see an executable entry turn into a symlink without `core.fileMode`.
    let file_is_gone = matches!(
        expected.mode,
        gix::index::entry::Mode::FILE | gix::index::entry::Mode::FILE_EXECUTABLE
    ) && !md.is_file();
    file_is_gone
        || expected
            .mode
            .change_to_match_fs(md, fs.symlink, fs.executable_bit)
            .is_some()
}

/// `SymlinkCheck` errors for paths that aren't in the worktree: missing or symlinked parents.
fn is_not_in_worktree(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::NotFound || is_symlinked_parent_error(err)
}

fn is_empty_dir(path: &std::path::Path) -> anyhow::Result<bool> {
    Ok(std::fs::read_dir(path)?.next().is_none())
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
