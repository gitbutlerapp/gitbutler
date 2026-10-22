//! Refusals that keep a checkout from losing what the user has: staged content, and worktree
//! edits made after the snapshot.

use std::collections::BTreeMap;

use bstr::{BStr, BString, ByteSlice as _};

use super::{
    Trees,
    plan::{Content, Plan},
    transaction::Placed,
};

/// Fail if the index has content the user staged, which differs from `head` and isn't in the
/// worktree (`baseline`) either, that recording the checkout in the index would replace or drop.
pub(super) fn staged_content(
    index: &gix::index::State,
    trees: &Trees,
    plan: &Plan,
) -> anyhow::Result<()> {
    let mut recorded = index.clone();
    plan.edit(&mut recorded, &Placed::default());
    let lost = index
        .entries()
        .iter()
        .filter(|entry| {
            let path = entry.path(index);
            only_staged(trees, path, entry) && !kept(&recorded, path, entry)
        })
        .map(|entry| format!("{:?}", entry.path(index)))
        .collect();
    refuse_unless_empty(lost, "Staged changes would be overwritten by checkout")
}

/// Whether `recorded` still has `entry`'s content at `path`.
fn kept(recorded: &gix::index::State, path: &BStr, entry: &gix::index::Entry) -> bool {
    Content::at(recorded, path) == Some(Content::of(entry))
}

/// Whether `entry` holds content found in neither `head` nor the worktree.
fn only_staged(trees: &Trees, path: &BStr, entry: &gix::index::Entry) -> bool {
    let content = Some(Content::of(entry));
    Content::at(&trees.head, path) != content && Content::at(&trees.baseline, path) != content
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
    Ok(clearances(removals))
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
        removals.extend(judge.object_in_the_way(rela_path, content)?);
    }
    Ok(removals)
}

/// The objects to move aside, a directory before anything in it, which moves along with it.
/// Shallower paths go first, so this holds even when a case-insensitive filesystem finds the
/// directory under a name that sorts after what's in it.
fn clearances(mut removals: Vec<Removal>) -> Vec<Clearance> {
    let depth = |removal: &Removal| {
        removal
            .rela_path
            .iter()
            .filter(|byte| **byte == b'/')
            .count()
    };
    removals.sort_by(|a, b| {
        depth(a)
            .cmp(&depth(b))
            .then_with(|| a.rela_path.cmp(&b.rela_path))
    });
    removals
        .into_iter()
        .map(|Removal { rela_path, .. }| Clearance { rela_path })
        .collect()
}

/// What judging worktree objects against the snapshot needs.
struct Judge<'a> {
    repo: &'a gix::Repository,
    workdir: std::path::PathBuf,
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

    /// The object at the deleted `rela_path`. Like Git, a path behind a missing directory, or
    /// a file or symlink where a directory should be, isn't in the worktree, and a directory, like a
    /// submodule's, only goes if empty.
    fn deleted_object(&self, rela_path: &BStr) -> anyhow::Result<Option<Removal>> {
        if self.blocking_ancestor(rela_path)?.is_some() {
            return Ok(None);
        }
        let judged = match self.metadata(rela_path)? {
            Some(md) if md.is_dir() && !is_empty_dir(&self.path(rela_path)?)? => return Ok(None),
            Some(md) if md.is_dir() => Vec::new(),
            Some(_) => vec![rela_path.to_owned()],
            None => return Ok(None),
        };
        Ok(Some(Removal {
            rela_path: rela_path.to_owned(),
            judged,
        }))
    }

    /// What writing `content` at `rela_path` removes: a leading component that isn't a
    /// directory, or the object at the path itself, including all files below it if it's a
    /// directory. Like Git, a submodule keeps the directory it finds, with everything in it.
    fn object_in_the_way(
        &mut self,
        rela_path: &BStr,
        content: Content,
    ) -> anyhow::Result<Option<Removal>> {
        let in_the_way = self
            .blocking_ancestor(rela_path)?
            .unwrap_or_else(|| rela_path.to_owned());
        let Some(md) = self.metadata(in_the_way.as_ref())? else {
            return Ok(None);
        };
        if md.is_dir() && content.is_submodule() {
            return Ok(None);
        }
        let judged = match md.is_dir() {
            // A submodule the snapshot has is judged as a whole.
            true if self.baseline_entry(in_the_way.as_ref()).is_some() => vec![in_the_way.clone()],
            true => self.files_below(in_the_way.as_ref())?,
            false => vec![in_the_way.clone()],
        };
        Ok(Some(Removal {
            rela_path: in_the_way,
            judged,
        }))
    }

    /// The first leading component of `rela_path` that is a file or symlink, which a directory
    /// must replace. A missing component ends the search, as nothing below it exists.
    fn blocking_ancestor(&self, rela_path: &BStr) -> anyhow::Result<Option<BString>> {
        let separators = rela_path
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/');
        for (at, _) in separators {
            let ancestor = rela_path[..at].as_bstr();
            match self.metadata(ancestor)? {
                Some(md) if md.is_dir() => continue,
                Some(_) => return Ok(Some(ancestor.to_owned())),
                None => return Ok(None),
            }
        }
        Ok(None)
    }

    /// The files and symlinks below `rela_dir`.
    fn files_below(&self, rela_dir: &BStr) -> anyhow::Result<Vec<BString>> {
        let mut files = Vec::new();
        let mut pending = vec![rela_dir.to_owned()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(self.path(dir.as_ref())?)? {
                let entry = entry?;
                let mut below = dir.clone();
                below.push(b'/');
                below.extend_from_slice(gix::path::os_str_into_bstr(&entry.file_name())?);
                match entry.file_type()?.is_dir() {
                    true => pending.push(below),
                    false => files.push(below),
                }
            }
        }
        Ok(files)
    }

    fn judge(&mut self, rela_path: &BStr) -> anyhow::Result<Verdict> {
        let md = gix::index::fs::Metadata::from_path_no_follow(&self.path(rela_path)?)?;
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
        // A submodule's directory must still be one, but what's in it isn't checkout's to judge.
        let same = !type_or_mode_changed(expected, md, &self.fs)
            && (expected.mode == gix::index::entry::Mode::COMMIT
                || self.unchanged_since_indexed(rela_path, md)
                || self.content_matches(rela_path, expected, md)?);
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

    /// Whether the index entry at `rela_path` has a stat that still matches the file, so the
    /// file can't have changed since it was hashed. The snapshot took such a file's id from the
    /// index, which checkout keeps locked, so the file is the snapshot's.
    fn unchanged_since_indexed(&self, rela_path: &BStr, md: &gix::index::fs::Metadata) -> bool {
        self.index.entry_by_path(rela_path).is_some_and(|entry| {
            !entry
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
