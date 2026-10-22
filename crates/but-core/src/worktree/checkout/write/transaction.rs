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

use super::{
    plan::Plan,
    protect::{self, Clearance, Reason},
};
use crate::index::Stat;

/// Files checked out into a staging directory, ready to be moved into the worktree.
pub(super) struct Staged {
    /// Removed when dropped, along with the objects moved aside into it, unless undoing the
    /// commit couldn't put one back.
    dir: tempfile::TempDir,
    files: Vec<StagedFile>,
    submodules: Vec<BString>,
}

/// A file checked out into the staging directory, with the stat it got there.
struct StagedFile {
    rela_path: BString,
    stat: gix::index::entry::Stat,
}

impl Staged {
    fn new_files(&self) -> PathBuf {
        self.dir.path().join("new")
    }

    fn aside(&self) -> PathBuf {
        self.dir.path().join("old")
    }
}

/// What the index should record about each file checkout placed in the worktree.
#[derive(Default)]
pub(super) struct Placed {
    stats: BTreeMap<BString, Stat>,
}

impl Placed {
    /// What the index should record about the file at `path`.
    pub fn stat(&self, path: &BStr) -> Stat {
        self.stats.get(path).copied().unwrap_or(Stat::Unknown)
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
    let dir = staging_dir(repo)?;
    let new_files = dir.path().join("new");
    std::fs::create_dir(&new_files)?;
    let mut index = checkout_index(target, plan);
    let opts = repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    gix::worktree::state::checkout(
        &mut index,
        new_files,
        repo.objects.clone().into_arc()?,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )?;
    let (submodules, files) = plan
        .writes()
        .partition::<Vec<_>, _>(|(_, content)| content.is_submodule());
    Ok(Staged {
        dir,
        files: files
            .into_iter()
            .map(|(path, _)| StagedFile {
                rela_path: path.to_owned(),
                stat: index
                    .entry_by_path(path)
                    .expect("written by the checkout")
                    .stat,
            })
            .collect(),
        submodules: submodules
            .into_iter()
            .map(|(path, _)| path.to_owned())
            .collect(),
    })
}

/// Move the `clearances` aside and the staged files into place. On failure, undo every step and
/// return the error, leaving the worktree as it was, except for what changed meanwhile.
pub(super) fn commit(
    repo: &gix::Repository,
    staged: Staged,
    clearances: &[Clearance],
) -> anyhow::Result<Placed> {
    let workdir = repo.workdir().expect("checked by the caller");
    protect::still_as_checked(workdir, clearances)?;
    let mut journal = Journal::new(workdir);
    match apply(&mut journal, workdir, &staged, clearances) {
        Ok(placed) => Ok(placed),
        Err(err) => Err(journal.undo(staged, err)),
    }
}

fn apply(
    journal: &mut Journal,
    workdir: &Path,
    staged: &Staged,
    clearances: &[Clearance],
) -> anyhow::Result<Placed> {
    // Like Git, remove the directories that the deletions leave empty while the files to be
    // replaced are still in place, so a directory the target still needs keeps its permissions,
    // and one it names differently, or needs as a submodule, is created afresh.
    let with_reason = |reason| clearances.iter().filter(move |c| c.reason == reason);
    let aside = staged.aside();
    for Clearance { rela_path, .. } in with_reason(Reason::Deleted) {
        journal.move_aside(&join(workdir, rela_path)?, &aside)?;
    }
    for Clearance { rela_path, .. } in with_reason(Reason::Deleted) {
        journal.remove_emptied_parents(workdir, &join(workdir, rela_path)?)?;
    }
    for Clearance { rela_path, .. } in with_reason(Reason::InTheWay) {
        journal.move_aside(&join(workdir, rela_path)?, &aside)?;
    }
    let mut stats = BTreeMap::new();
    for StagedFile { rela_path, stat } in &staged.files {
        let dest = join(workdir, rela_path)?;
        journal.place(&join(&staged.new_files(), rela_path)?, &dest, stat)?;
        stats.insert(rela_path.clone(), trusted_stat(stat, &dest)?);
    }
    for rela_path in &staged.submodules {
        journal.ensure_dir(&join(workdir, rela_path)?)?;
    }
    Ok(Placed { stats })
}

/// What the commit did, in order, so it can be undone.
struct Journal {
    workdir: PathBuf,
    steps: Vec<Step>,
}

enum Step {
    MovedAside {
        from: PathBuf,
        to: PathBuf,
    },
    RemovedDir {
        path: PathBuf,
        permissions: std::fs::Permissions,
    },
    Placed {
        path: PathBuf,
        staged: gix::index::entry::Stat,
    },
    CreatedDir(PathBuf),
}

impl Journal {
    fn new(workdir: &Path) -> Self {
        Journal {
            workdir: workdir.to_owned(),
            steps: Vec::new(),
        }
    }

    /// Move the object at `from` to `to`. An object that is gone loses nothing.
    /// Move the object at `from` into its own place in the directory `aside`, so objects moved
    /// aside never collide, even if one was inside another. An object that is gone loses
    /// nothing.
    fn move_aside(&mut self, from: &Path, aside: &Path) -> anyhow::Result<()> {
        if !self.has_real_leading_dirs(from)? {
            return Ok(());
        }
        std::fs::create_dir_all(aside)?;
        let to = aside.join(self.steps.len().to_string());
        match std::fs::rename(from, &to) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err).with_context(|| format!("moving {from:?} aside")),
        }
        self.steps.push(Step::MovedAside {
            from: from.to_owned(),
            to,
        });
        Ok(())
    }

    /// Move the `staged` file to `dest`, where nothing is left after the clearances moved aside,
    /// unless something was created there after the worktree was checked.
    /// Move the file at `staged`, with the `stat`, to `dest`, unless something appeared there.
    fn place(
        &mut self,
        staged: &Path,
        dest: &Path,
        stat: &gix::index::entry::Stat,
    ) -> anyhow::Result<()> {
        self.create_parents(dest)?;
        match rename_no_replace(staged, dest) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(self.appeared_after_the_check(dest));
            }
            Err(err) => return Err(err).with_context(|| format!("placing {dest:?}")),
        }
        self.steps.push(Step::Placed {
            path: dest.to_owned(),
            staged: *stat,
        });
        Ok(())
    }

    /// Create the directory `dir` and its parents, unless it exists.
    fn ensure_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        self.create_parents(dir)?;
        self.ensure_one_dir(dir)
    }

    /// Create the missing directories leading to `path`, from the worktree's root down.
    fn create_parents(&mut self, path: &Path) -> anyhow::Result<()> {
        for dir in leading_dirs(&self.workdir.clone(), path) {
            self.ensure_one_dir(dir)?;
        }
        Ok(())
    }

    /// Whether the directories leading to `path` are all directories, so the object there can be
    /// moved without following a symlink, maybe out of the worktree. A missing one means the
    /// object is gone. A file or symlink appeared after the worktree was checked, so refuse.
    fn has_real_leading_dirs(&self, path: &Path) -> anyhow::Result<bool> {
        for dir in leading_dirs(&self.workdir, path) {
            match dir.symlink_metadata() {
                Ok(md) if md.is_dir() => {}
                Ok(_) => return Err(self.appeared_after_the_check(dir)),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(err) => return Err(err.into()),
            }
        }
        Ok(true)
    }

    /// Create the directory `dir` unless it is one. A file or symlink there appeared after the
    /// worktree was checked, so refuse rather than write through it, maybe outside the worktree.
    fn ensure_one_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        match dir.symlink_metadata() {
            Ok(md) if md.is_dir() => Ok(()),
            Ok(_) => Err(self.appeared_after_the_check(dir)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(dir).with_context(|| format!("creating {dir:?}"))?;
                self.steps.push(Step::CreatedDir(dir.to_owned()));
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    fn appeared_after_the_check(&self, path: &Path) -> anyhow::Error {
        let rela_path = path.strip_prefix(&self.workdir).unwrap_or(path);
        anyhow::anyhow!(
            but_error::Context::new(format!(
                "Uncommitted files would be overwritten by checkout: {:?}",
                rela_path.display().to_string()
            ))
            .with_code(but_error::Code::PreconditionFailed)
        )
    }

    /// Remove the directories leading to `path` that are now empty, as Git does.
    fn remove_emptied_parents(&mut self, workdir: &Path, path: &Path) -> anyhow::Result<()> {
        for dir in path.ancestors().skip(1).take_while(|dir| *dir != workdir) {
            let permissions = match std::fs::symlink_metadata(dir) {
                Ok(md) => md.permissions(),
                // Emptied by another deletion, and removed with what it left empty above it.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                Err(err) => return Err(err.into()),
            };
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
            self.steps.push(Step::RemovedDir {
                path: dir.to_owned(),
                permissions,
            });
        }
        Ok(())
    }

    /// Undo every step in reverse. Failures are logged, as there is nothing left to fall back to.
    /// Undo every step, latest first, and return `err`. What changed since a step stays: an
    /// object moved aside whose place was taken meanwhile stays in `staged`, which is then kept,
    /// and `err` says where.
    fn undo(self, staged: Staged, err: anyhow::Error) -> anyhow::Error {
        let workdir = self.workdir.clone();
        let unrestored = self.undo_steps();
        if unrestored.is_empty() {
            return err;
        }
        let _kept = staged.dir.keep();
        let kept: Vec<String> = unrestored
            .iter()
            .map(|(from, to)| {
                let rela_path = from.strip_prefix(&workdir).unwrap_or(from);
                format!("{:?} at {:?}", rela_path.display().to_string(), to)
            })
            .collect();
        err.context(format!(
            "Checkout failed and was undone, but these paths changed meanwhile, so their previous versions were kept elsewhere: {}",
            kept.join(", ")
        ))
    }

    /// Undo every step, latest first, and return the objects moved aside that couldn't be put
    /// back, from where to where.
    fn undo_steps(self) -> Vec<(PathBuf, PathBuf)> {
        let mut unrestored = Vec::new();
        for step in self.steps.into_iter().rev() {
            let undone = match &step {
                Step::Placed { path, staged } => remove_unless_changed(path, staged),
                Step::CreatedDir(dir) => std::fs::remove_dir(dir),
                Step::RemovedDir { path, permissions } => std::fs::create_dir(path)
                    .and_then(|()| std::fs::set_permissions(path, permissions.clone())),
                Step::MovedAside { from, to } => rename_no_replace(to, from),
            };
            if let Err(err) = undone {
                tracing::error!(?err, "could not undo a checkout step");
                if let Step::MovedAside { from, to } = step {
                    unrestored.push((from, to));
                }
            }
        }
        unrestored
    }
}

/// Remove the file checkout placed at `path` with the `staged` stat, unless it changed since.
fn remove_unless_changed(path: &Path, staged: &gix::index::entry::Stat) -> std::io::Result<()> {
    match trusted_stat(staged, path) {
        Ok(Stat::Known(_)) => gix::fs::symlink::remove(path),
        Ok(Stat::Unknown) | Err(_) => Ok(()),
    }
}

/// Rename `from` to `to` unless something is there, failing with `AlreadyExists`, so nothing
/// that appeared there meanwhile is replaced. Where the system can't check and rename in one
/// step, like on Windows or for directories on some filesystems, it checks first.
fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        match rename_no_replace_in_one_step(from, to) {
            Err(err) if err.kind() != std::io::ErrorKind::AlreadyExists => {}
            done => return done,
        }
    }
    if to.symlink_metadata().is_ok() {
        return Err(std::io::ErrorKind::AlreadyExists.into());
    }
    std::fs::rename(from, to)
}

#[cfg(unix)]
fn rename_no_replace_in_one_step(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut path = tempfile::TempPath::try_from_path(from)?;
    // `from` isn't temporary, so keep it if renaming fails.
    path.disable_cleanup(true);
    path.persist_noclobber(to).map_err(|err| err.error)
}

/// The directories between `workdir` and `path`, from the root down.
fn leading_dirs<'a>(workdir: &Path, path: &'a Path) -> Vec<&'a Path> {
    let mut dirs: Vec<&Path> = path
        .ancestors()
        .skip(1)
        .take_while(|dir| *dir != workdir)
        .collect();
    dirs.reverse();
    dirs
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

/// What the index may record about the file placed at `dest`, which had the `staged` stat in the
/// staging directory.
///
/// Renaming keeps everything but the ctime, which is why the stat is read again. A file that
/// doesn't match its staged stat otherwise was changed after it was placed, so its stat can't
/// vouch for the staged content, and Git has to compare the content instead.
fn trusted_stat(staged: &gix::index::entry::Stat, dest: &Path) -> anyhow::Result<Stat> {
    let md = gix::index::fs::Metadata::from_path_no_follow(dest)?;
    let placed = gix::index::entry::Stat::from_fs(&md)?;
    let all_but_ctime = gix::index::entry::stat::Options {
        trust_ctime: false,
        check_stat: true,
        use_nsec: true,
        use_stdev: false,
    };
    Ok(match staged.matches(&placed, all_but_ctime) {
        true => Stat::Known(placed),
        false => Stat::Unknown,
    })
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

#[cfg(test)]
mod tests {
    use std::{path::Path, time::SystemTime};

    use crate::index::Stat;

    fn set_mtime(path: &Path, mtime: SystemTime) -> std::io::Result<()> {
        std::fs::File::options()
            .write(true)
            .open(path)?
            .set_modified(mtime)
    }

    fn stat(path: &Path) -> anyhow::Result<gix::index::entry::Stat> {
        Ok(gix::index::entry::Stat::from_fs(
            &gix::index::fs::Metadata::from_path_no_follow(path)?,
        )?)
    }

    /// A file staged at a whole second, then placed at `placed`, and its staged stat.
    fn place(dir: &Path) -> anyhow::Result<(std::path::PathBuf, gix::index::entry::Stat)> {
        let staged_path = dir.join("staged");
        std::fs::write(&staged_path, "staged\n")?;
        set_mtime(
            &staged_path,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
        )?;
        let staged = stat(&staged_path)?;
        let placed = dir.join("placed");
        std::fs::rename(&staged_path, &placed)?;
        Ok((placed, staged))
    }

    #[test]
    fn a_placed_file_is_trusted_with_its_new_stat() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, staged) = place(tmp.path())?;

        assert!(
            matches!(super::trusted_stat(&staged, &placed)?, Stat::Known(_)),
            "renaming keeps the file's identity"
        );
        Ok(())
    }

    #[test]
    fn a_placed_file_edited_in_the_same_second_is_not_trusted() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, staged) = place(tmp.path())?;
        std::fs::write(&placed, "edited\n")?;
        set_mtime(
            &placed,
            SystemTime::UNIX_EPOCH + std::time::Duration::new(1_000_000, 1_000),
        )?;

        assert!(
            matches!(super::trusted_stat(&staged, &placed)?, Stat::Unknown),
            "an edit after placing, like an editor saving the file, makes its stat untrustworthy"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn a_placed_file_replaced_by_a_new_file_is_not_trusted() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, staged) = place(tmp.path())?;
        // Editors often save by writing a new file and renaming it over the old one.
        let saved = tmp.path().join("saved");
        std::fs::write(&saved, "edited\n")?;
        set_mtime(
            &saved,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
        )?;
        std::fs::rename(&saved, &placed)?;

        assert!(
            matches!(super::trusted_stat(&staged, &placed)?, Stat::Unknown),
            "a new file with the same size and time is still another file"
        );
        Ok(())
    }

    /// A worktree with the file `a`, and a staging directory to check out into it.
    fn worktree_and_staging() -> anyhow::Result<(tempfile::TempDir, super::Staged)> {
        let workdir = tempfile::tempdir()?;
        std::fs::write(workdir.path().join("a"), "before\n")?;
        let staged = super::Staged {
            dir: tempfile::tempdir()?,
            files: Vec::new(),
            submodules: Vec::new(),
        };
        std::fs::create_dir(staged.new_files())?;
        Ok((workdir, staged))
    }

    /// Stage `content` as the new file `name`, and return where it is and its stat.
    fn stage(
        staged: &super::Staged,
        name: &str,
        content: &str,
    ) -> anyhow::Result<(std::path::PathBuf, gix::index::entry::Stat)> {
        let path = staged.new_files().join(name);
        std::fs::write(&path, content)?;
        Ok((path.clone(), stat(&path)?))
    }

    #[test]
    fn undo_keeps_a_file_saved_where_one_was_moved_aside_and_says_where_that_one_is()
    -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let a = workdir.path().join("a");
        let (new_a, new_stat) = stage(&staged, "a", "target\n")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.move_aside(&a, &staged.aside())?;
        std::fs::write(&a, "saved meanwhile\n")?;

        let refused = journal
            .place(&new_a, &a, &new_stat)
            .expect_err("`a` appeared after the check");
        let (staging_dir, aside) = (staged.dir.path().to_owned(), staged.aside());
        let err = journal.undo(staged, refused);

        assert_eq!(std::fs::read_to_string(&a)?, "saved meanwhile\n");
        let kept = std::fs::read_dir(&aside)?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            kept.len(),
            1,
            "the staging directory is kept with the backup"
        );
        let backup = kept[0].path();
        assert_eq!(std::fs::read_to_string(&backup)?, "before\n");
        assert_eq!(
            format!("{err:#}"),
            format!(
                "Checkout failed and was undone, but these paths changed meanwhile, so their \
                 previous versions were kept elsewhere: \"a\" at {backup:?}: Uncommitted files \
                 would be overwritten by checkout: \"a\""
            )
        );
        std::fs::remove_dir_all(staging_dir)?;
        Ok(())
    }

    #[test]
    fn undo_keeps_a_placed_file_changed_since() -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let b = workdir.path().join("b");
        let (new_b, new_stat) = stage(&staged, "b", "target\n")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.place(&new_b, &b, &new_stat)?;
        std::fs::write(&b, "saved after placing\n")?;

        journal.undo(staged, anyhow::anyhow!("a later step failed"));

        assert_eq!(std::fs::read_to_string(&b)?, "saved after placing\n");
        Ok(())
    }
}
