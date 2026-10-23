//! Refusals that keep a checkout from losing what the user has: staged content, and worktree
//! edits made after the snapshot.

use std::collections::{BTreeMap, BTreeSet};

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
    pub reason: Reason,
    /// The objects in it that were checked against the snapshot, with their stat back then.
    observed: Vec<Observed>,
}

/// An object checked against the snapshot, and its stat when it was checked.
struct Observed {
    rela_path: BString,
    stat: gix::index::entry::Stat,
}

/// Why checkout moves an object aside.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reason {
    /// The target deletes it.
    Deleted,
    /// It's in the way of a file the target writes.
    InTheWay,
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
    let mut removals = objects_to_remove(&mut judge, plan)?;
    refuse_unless_empty(
        judge_each_once(&mut judge, &mut removals)?,
        "Uncommitted files would be overwritten by checkout",
    )?;
    Ok(clearances(removals))
}

/// Judge the objects of `removals`, recording the stat of those that are the snapshot's with
/// their removal, and return the paths of those that changed.
///
/// An object can be removed for several reasons, like being deleted and in the way of the files
/// that replace it, or being in a directory that is removed too. It's judged once.
fn judge_each_once(judge: &mut Judge<'_>, removals: &mut [Removal]) -> anyhow::Result<Vec<String>> {
    let (mut judged, mut changed) = (BTreeSet::new(), Vec::new());
    for removal in removals {
        let new: Vec<BString> = std::mem::take(&mut removal.judged)
            .into_iter()
            .filter(|rela_path| judged.insert(rela_path.clone()))
            .collect();
        for rela_path in new {
            match judge.judge(rela_path.as_ref())? {
                Verdict::Snapshotted(stat) => removal.observed.push(Observed { rela_path, stat }),
                Verdict::Ignored => {}
                Verdict::Changed(path) => changed.push(format!("{path:?}")),
            }
        }
    }
    Ok(changed)
}

/// Fail if an object checked against the snapshot changed since, which no filter run in
/// between can then hide: an edit saved while the check ran clean filters on other files.
/// Gone objects lose nothing.
pub(super) fn still_as_checked(
    workdir: &std::path::Path,
    clearances: &[Clearance],
) -> anyhow::Result<()> {
    let all_fields = gix::index::entry::stat::Options {
        trust_ctime: true,
        check_stat: true,
        use_nsec: true,
        use_stdev: false,
    };
    let mut changed = Vec::new();
    for Observed { rela_path, stat } in clearances.iter().flat_map(|c| &c.observed) {
        let path = workdir.join(gix::path::from_bstr(rela_path.as_bstr())?);
        let now = match gix::index::fs::Metadata::from_path_no_follow(&path) {
            Ok(md) => gix::index::entry::Stat::from_fs(&md)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        };
        if !stat.matches(&now, all_fields) {
            changed.push(format!("{rela_path:?}"));
        }
    }
    refuse_unless_empty(
        changed,
        "Uncommitted files would be overwritten by checkout",
    )
}

/// An object checkout removes, and the files in it whose content must be the snapshot's.
struct Removal {
    rela_path: BString,
    reason: Reason,
    judged: Vec<BString>,
    /// The directories walked to find what to judge, and the judged objects that matched the
    /// snapshot, with their stat when seen.
    observed: Vec<Observed>,
}

/// What an object about to be removed is, compared with the snapshot.
enum Verdict {
    /// It has the snapshot's content, and had this stat when it was checked.
    Snapshotted(gix::index::entry::Stat),
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

/// The objects to move aside.
fn clearances(removals: Vec<Removal>) -> Vec<Clearance> {
    removals
        .into_iter()
        .map(
            |Removal {
                 rela_path,
                 reason,
                 observed,
                 ..
             }| Clearance {
                rela_path,
                reason,
                observed,
            },
        )
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
        let path = self.path(rela_path)?;
        let (judged, observed) = match self.metadata(rela_path)? {
            Some(md) if md.is_dir() && !is_empty_dir(&path)? => return Ok(None),
            // Nothing to judge in it, but what appears in it later shows in its stat.
            Some(md) if md.is_dir() => (
                Vec::new(),
                vec![Observed {
                    rela_path: rela_path.to_owned(),
                    stat: gix::index::entry::Stat::from_fs(
                        &gix::index::fs::Metadata::from_path_no_follow(&path)?,
                    )?,
                }],
            ),
            Some(_) => (vec![rela_path.to_owned()], Vec::new()),
            None => return Ok(None),
        };
        Ok(Some(Removal {
            rela_path: rela_path.to_owned(),
            reason: Reason::Deleted,
            judged,
            observed,
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
        let (judged, observed) = match md.is_dir() {
            // A submodule the snapshot has is judged as a whole.
            true if self.baseline_entry(in_the_way.as_ref()).is_some() => {
                (vec![in_the_way.clone()], Vec::new())
            }
            true => self.files_below(in_the_way.as_ref())?,
            false => (vec![in_the_way.clone()], Vec::new()),
        };
        Ok(Some(Removal {
            rela_path: in_the_way,
            reason: Reason::InTheWay,
            judged,
            observed,
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

    /// The files and symlinks below `rela_dir`, and the directories walked to find them, as
    /// seen before walking them, so a file added to one later shows in its stat.
    fn files_below(&self, rela_dir: &BStr) -> anyhow::Result<(Vec<BString>, Vec<Observed>)> {
        let (mut files, mut dirs) = (Vec::new(), Vec::new());
        let mut pending = vec![rela_dir.to_owned()];
        while let Some(dir) = pending.pop() {
            let path = self.path(dir.as_ref())?;
            let stat = gix::index::entry::Stat::from_fs(
                &gix::index::fs::Metadata::from_path_no_follow(&path)?,
            )?;
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let mut below = dir.clone();
                below.push(b'/');
                below.extend_from_slice(gix::path::os_str_into_bstr(&entry.file_name())?);
                match entry.file_type()?.is_dir() {
                    true => pending.push(below),
                    false => files.push(below),
                }
            }
            dirs.push(Observed {
                rela_path: dir,
                stat,
            });
        }
        Ok((files, dirs))
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
                || self.unchanged_since_indexed(rela_path, expected, md)
                || self.content_matches(rela_path, expected, md)?);
        Ok(match same {
            true => Verdict::Snapshotted(gix::index::entry::Stat::from_fs(md)?),
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

    /// Whether the index entry at `rela_path` has the content `expected` of the snapshot, and a
    /// stat that still matches the file, so the file has that content. Checkout keeps the index
    /// locked, but the file can change: a file the snapshot found edited can be reverted to the
    /// indexed content and stat.
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Clearance, Observed, Reason};

    fn stat(path: &std::path::Path) -> anyhow::Result<gix::index::entry::Stat> {
        Ok(gix::index::entry::Stat::from_fs(
            &gix::index::fs::Metadata::from_path_no_follow(path)?,
        )?)
    }

    fn set_modified(path: &std::path::Path, time: SystemTime) -> std::io::Result<()> {
        std::fs::File::options()
            .write(true)
            .open(path)?
            .set_modified(time)
    }

    #[test]
    fn an_edit_in_the_same_second_after_the_check_is_refused() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().join("p");
        std::fs::write(&path, "h\n")?;
        let checked_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        set_modified(&path, checked_at)?;
        let clearances = [Clearance {
            rela_path: "p".into(),
            reason: Reason::Deleted,
            observed: vec![Observed {
                rela_path: "p".into(),
                stat: stat(&path)?,
            }],
        }];
        // An editor saving within the same second, keeping the size and the file.
        std::fs::write(&path, "w\n")?;
        set_modified(&path, checked_at + Duration::from_micros(1))?;

        let err = super::still_as_checked(tmp.path(), &clearances)
            .expect_err("the edit came after the check");

        assert_eq!(
            err.to_string(),
            "Uncommitted files would be overwritten by checkout: \"p\""
        );
        Ok(())
    }
}
