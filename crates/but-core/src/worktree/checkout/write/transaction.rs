//! Writing a checkout's changes so that they either all happen or none do.
//!
//! Files are first checked out into a staging directory, where the worktree filters run. Only
//! then are the objects in their way moved aside and the staged files moved into place, with each
//! step journaled so a failure can put everything back.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use bstr::{BStr, BString};

use super::{plan::Plan, protect::Clearance};
use crate::index::Stat;

/// Files checked out into a staging directory, ready to be moved into the worktree.
pub(super) struct Staged {
    /// Removed when dropped, along with the objects moved aside into it.
    dir: tempfile::TempDir,
    files: Vec<BString>,
    submodules: Vec<BString>,
}

impl Staged {
    fn new_files(&self) -> PathBuf {
        self.dir.path().join("new")
    }

    fn aside(&self) -> PathBuf {
        self.dir.path().join("old")
    }
}

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

/// Check out the files `plan` writes into a staging directory, running the worktree filters.
///
/// Without `keep_going`, a file that fails to be written, or that a filter delays and never
/// delivers, fails here, before the worktree is touched.
pub(super) fn stage(
    repo: &gix::Repository,
    target: &gix::index::State,
    plan: &Plan,
) -> anyhow::Result<Staged> {
    let (files, submodules) = plan
        .writes()
        .map(|(path, content)| (path.to_owned(), content.is_submodule()))
        .partition::<Vec<_>, _>(|(_, is_submodule)| !is_submodule);
    let staged = Staged {
        dir: staging_dir(repo)?,
        files: files.into_iter().map(|(path, _)| path).collect(),
        submodules: submodules.into_iter().map(|(path, _)| path).collect(),
    };
    std::fs::create_dir(staged.new_files())?;
    let opts = repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    gix::worktree::state::checkout(
        &mut checkout_index(target, plan),
        staged.new_files(),
        repo.objects.clone().into_arc()?,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )?;
    Ok(staged)
}

/// Move the `clearances` aside and the staged files into place. On failure, undo every step and
/// return the error, leaving the worktree as it was.
pub(super) fn commit(
    repo: &gix::Repository,
    staged: Staged,
    clearances: &[Clearance],
) -> anyhow::Result<Placed> {
    let workdir = repo.workdir().expect("checked by the caller");
    let mut journal = Journal::default();
    if let Err(err) = apply(&mut journal, workdir, &staged, clearances) {
        journal.undo();
        return Err(err);
    }
    let placed = placed(workdir, &staged)?;
    for clearance in clearances {
        remove_emptied_parents(workdir, clearance.rela_path.as_ref())?;
    }
    Ok(placed)
}

fn apply(
    journal: &mut Journal,
    workdir: &Path,
    staged: &Staged,
    clearances: &[Clearance],
) -> anyhow::Result<()> {
    for Clearance { rela_path } in clearances {
        journal.move_aside(
            &join(workdir, rela_path)?,
            &join(&staged.aside(), rela_path)?,
        )?;
    }
    for rela_path in &staged.files {
        let dest = join(workdir, rela_path)?;
        journal.place(&join(&staged.new_files(), rela_path)?, &dest)?;
    }
    for rela_path in &staged.submodules {
        journal.ensure_dir(&join(workdir, rela_path)?)?;
    }
    Ok(())
}

/// What the commit did, in order, so it can be undone.
#[derive(Default)]
struct Journal {
    steps: Vec<Step>,
}

enum Step {
    MovedAside { from: PathBuf, to: PathBuf },
    Placed(PathBuf),
    CreatedDir(PathBuf),
}

impl Journal {
    /// Move the object at `from` to `to`. An object that is gone loses nothing.
    fn move_aside(&mut self, from: &Path, to: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(to.parent().expect("below the staging directory"))?;
        match std::fs::rename(from, to) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err).with_context(|| format!("moving {from:?} aside")),
        }
        self.steps.push(Step::MovedAside {
            from: from.to_owned(),
            to: to.to_owned(),
        });
        Ok(())
    }

    /// Move the `staged` file to `dest`, where nothing is left after the clearances moved aside.
    fn place(&mut self, staged: &Path, dest: &Path) -> anyhow::Result<()> {
        self.create_parents(dest)?;
        std::fs::rename(staged, dest).with_context(|| format!("placing {dest:?}"))?;
        self.steps.push(Step::Placed(dest.to_owned()));
        Ok(())
    }

    /// Create the directory `dir` and its parents, unless it exists.
    fn ensure_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        self.create_parents(dir)?;
        if !dir.is_dir() {
            std::fs::create_dir(dir).with_context(|| format!("creating {dir:?}"))?;
            self.steps.push(Step::CreatedDir(dir.to_owned()));
        }
        Ok(())
    }

    fn create_parents(&mut self, path: &Path) -> anyhow::Result<()> {
        let missing: Vec<&Path> = path
            .ancestors()
            .skip(1)
            .take_while(|dir| dir.symlink_metadata().is_err())
            .collect();
        for dir in missing.into_iter().rev() {
            std::fs::create_dir(dir).with_context(|| format!("creating {dir:?}"))?;
            self.steps.push(Step::CreatedDir(dir.to_owned()));
        }
        Ok(())
    }

    /// Undo every step in reverse. Failures are logged, as there is nothing left to fall back to.
    fn undo(self) {
        for step in self.steps.into_iter().rev() {
            let undone = match &step {
                Step::Placed(path) => gix::fs::symlink::remove(path),
                Step::CreatedDir(dir) => std::fs::remove_dir(dir),
                Step::MovedAside { from, to } => std::fs::rename(to, from),
            };
            if let Err(err) = undone {
                tracing::error!(?err, "could not undo a checkout step");
            }
        }
    }
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

/// The stat of each placed file. Renaming may change a file's ctime, so it's read again.
fn placed(workdir: &Path, staged: &Staged) -> anyhow::Result<Placed> {
    let mut stats = BTreeMap::new();
    for rela_path in &staged.files {
        let md = gix::index::fs::Metadata::from_path_no_follow(&join(workdir, rela_path)?)?;
        stats.insert(rela_path.clone(), gix::index::entry::Stat::from_fs(&md)?);
    }
    Ok(Placed { stats })
}

/// Remove the directories leading to `rela_path` that are now empty, as Git does.
fn remove_emptied_parents(workdir: &Path, rela_path: &BStr) -> anyhow::Result<()> {
    let path = join(workdir, rela_path)?;
    for dir in path.ancestors().skip(1).take_while(|dir| *dir != workdir) {
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
    }
    Ok(())
}

/// A new staging directory on the worktree's filesystem, so files can be renamed into place.
/// It goes into the Git directory, or into the worktree if that is on another filesystem.
fn staging_dir(repo: &gix::Repository) -> anyhow::Result<tempfile::TempDir> {
    let workdir = repo.workdir().expect("checked by the caller");
    let in_git_dir = repo.git_dir().join("gitbutler");
    std::fs::create_dir_all(&in_git_dir)?;
    let parent = match same_filesystem(&in_git_dir, workdir)? {
        true => in_git_dir.as_path(),
        false => workdir,
    };
    Ok(tempfile::Builder::new()
        .prefix(".gitbutler-checkout-")
        .tempdir_in(parent)?)
}

#[cfg(unix)]
fn same_filesystem(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(a.metadata()?.dev() == b.metadata()?.dev())
}

/// Different drives are different filesystems.
#[cfg(not(unix))]
fn same_filesystem(a: &Path, b: &Path) -> std::io::Result<bool> {
    let prefix = |path: &Path| -> std::io::Result<_> {
        Ok(gix::path::realpath(path)
            .map_err(std::io::Error::other)?
            .components()
            .next()
            .map(|c| c.as_os_str().to_owned()))
    };
    Ok(prefix(a)? == prefix(b)?)
}

fn join(dir: &Path, rela_path: impl AsRef<BStr>) -> anyhow::Result<PathBuf> {
    Ok(dir.join(gix::path::from_bstr(rela_path.as_ref())?))
}
