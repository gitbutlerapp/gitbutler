//! Writing a checkout's changes so that they either all happen or none do.
//!
//! Files are first checked out into a staging directory, where the worktree filters run. Only
//! then are the objects in their way moved aside and the staged files copied into place, with each
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

/// Files checked out into a staging directory, ready to be copied into the worktree.
pub(super) struct Staged {
    /// Removed when dropped, along with the objects moved aside into it, unless undoing the
    /// commit couldn't put one back.
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
        files: files.into_iter().map(|(path, _)| path.to_owned()).collect(),
        submodules: submodules
            .into_iter()
            .map(|(path, _)| path.to_owned())
            .collect(),
    })
}

/// Move the `clearances` aside and copy the staged files into place. On failure, undo every step
/// and return the error, leaving the worktree as it was, except for what changed meanwhile.
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
        journal.move_emptied_parents_aside(&join(workdir, rela_path)?, &aside)?;
    }
    for Clearance { rela_path, .. } in with_reason(Reason::InTheWay) {
        journal.move_aside(&join(workdir, rela_path)?, &aside)?;
    }
    let mut stats = BTreeMap::new();
    for rela_path in &staged.files {
        let dest = join(workdir, rela_path)?;
        let stat = journal.place(&join(&staged.new_files(), rela_path)?, &dest)?;
        stats.insert(rela_path.clone(), stat);
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
    /// Where objects that can't be moved into the staging directory, as they're on another
    /// filesystem, are moved aside instead, by the top of their filesystem in the worktree.
    asides_elsewhere: BTreeMap<PathBuf, tempfile::TempDir>,
}

enum Step {
    MovedAside {
        from: PathBuf,
        to: PathBuf,
    },
    Placed {
        path: PathBuf,
        stat: gix::index::entry::Stat,
    },
    CreatedDir(PathBuf),
}

impl Step {
    /// The path in the worktree the step changed.
    fn path(&self) -> &Path {
        match self {
            Step::MovedAside { from, .. } => from,
            Step::Placed { path, .. } => path,
            Step::CreatedDir(dir) => dir,
        }
    }
}

impl Journal {
    fn new(workdir: &Path) -> Self {
        Journal {
            workdir: workdir.to_owned(),
            steps: Vec::new(),
            asides_elsewhere: BTreeMap::new(),
        }
    }

    /// Move the object at `from` to `to`. An object that is gone loses nothing.
    /// Move the object at `from` into its own place in the directory `aside`, so objects moved
    /// aside never collide, even if one was inside another. An object that is gone loses
    /// nothing.
    fn move_aside(&mut self, from: &Path, aside: &Path) -> anyhow::Result<()> {
        match leading_dirs_of(&self.workdir, from)? {
            Leading::Dirs => {}
            // Nothing is there.
            Leading::Missing => return Ok(()),
            Leading::Obstructed(dir) => return Err(self.appeared_after_the_check(&dir)),
        }
        let to = match self.rename_aside(from, aside) {
            Ok(to) => to,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err).with_context(|| format!("moving {from:?} aside")),
        };
        self.steps.push(Step::MovedAside {
            from: from.to_owned(),
            to,
        });
        Ok(())
    }

    /// Move `from` into its own place in `aside`, or if that's on another filesystem, in a
    /// directory on the filesystem of `from`, and return where it went.
    fn rename_aside(&mut self, from: &Path, aside: &Path) -> std::io::Result<PathBuf> {
        let place = self.steps.len().to_string();
        std::fs::create_dir_all(aside)?;
        let to = aside.join(&place);
        match std::fs::rename(from, &to) {
            Err(err) if err.kind() == std::io::ErrorKind::CrossesDevices => {
                let to = self.aside_on_the_filesystem_of(from)?.join(place);
                std::fs::rename(from, &to).map(|()| to)
            }
            moved => moved.map(|()| to),
        }
    }

    /// A private directory at the top of the filesystem `path` is on in the worktree.
    fn aside_on_the_filesystem_of(&mut self, path: &Path) -> std::io::Result<PathBuf> {
        let top = top_of_filesystem(&self.workdir, path)?;
        if let Some(dir) = self.asides_elsewhere.get(&top) {
            return Ok(dir.path().to_owned());
        }
        let dir = tempfile::Builder::new()
            .prefix(".gitbutler-checkout-")
            .tempdir_in(&top)?;
        let path = dir.path().to_owned();
        self.asides_elsewhere.insert(top, dir);
        Ok(path)
    }

    /// Move the `staged` file to `dest`, where nothing is left after the clearances moved aside,
    /// unless something was created there after the worktree was checked.
    /// Create a copy of the file or symlink at `staged` at `dest`, unless something appeared
    /// there, and return what the index may record about it.
    fn place(&mut self, staged: &Path, dest: &Path) -> anyhow::Result<Stat> {
        self.create_parents(dest)?;
        let md = gix::index::fs::Metadata::from_path_no_follow(staged)?;
        let placed = match md.is_symlink() {
            true => self.create_symlink(&std::fs::read_link(staged)?, dest),
            false => std::fs::File::open(staged)
                .and_then(|mut file| self.create_file(&mut file, md.is_executable(), dest)),
        };
        match placed {
            Ok(stat) => Ok(stat),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(self.appeared_after_the_check(dest))
            }
            Err(err) => Err(err).with_context(|| format!("placing {dest:?}")),
        }
    }

    /// Create a file with `content` at `dest` like Git creates files, so it gets what its
    /// directory gives new files, like a default ACL or its group. It's written privately next to
    /// `dest`, then moved there unless something appeared, so `dest` only ever has all of it, and
    /// nothing written into it meanwhile is taken for checkout's.
    fn create_file(
        &mut self,
        content: &mut impl std::io::Read,
        executable: bool,
        dest: &Path,
    ) -> std::io::Result<Stat> {
        let mut private = private_file_next_to(dest, executable)?;
        std::io::copy(content, private.as_file_mut())?;
        let written = stat_of(&gix::index::fs::Metadata::from_file(private.as_file())?)?;
        let mut private = private.into_temp_path();
        rename_no_replace(&private, dest)?;
        private.disable_cleanup(true);
        self.placed(dest, &written)
    }

    fn create_symlink(&mut self, target: &Path, dest: &Path) -> std::io::Result<Stat> {
        gix::fs::symlink::create(target, dest)?;
        let written = stat_of(&gix::index::fs::Metadata::from_path_no_follow(dest)?)?;
        self.placed(dest, &written)
    }

    /// Journal the file or symlink placed at `path`, which had the `written` stat when it was
    /// written, and return what the index may record about it. Undo removes it only while it has
    /// the stat it had once placed, ctime included, which any later change to it changes, like a
    /// new mode. One that changed while it was placed stays.
    fn placed(&mut self, path: &Path, written: &gix::index::entry::Stat) -> std::io::Result<Stat> {
        let trusted = trusted_stat(written, path)?;
        if let Stat::Known(placed) = trusted {
            self.steps.push(Step::Placed {
                path: path.to_owned(),
                stat: placed,
            });
        }
        Ok(trusted)
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
    /// Like Git, remove the directories leading to the deleted `path` that it left empty. They're
    /// moved `aside`, so undo puts each back itself, with all it had, like its ACLs.
    fn move_emptied_parents_aside(&mut self, path: &Path, aside: &Path) -> anyhow::Result<()> {
        let workdir = self.workdir.clone();
        for dir in path.ancestors().skip(1).take_while(|dir| *dir != workdir) {
            // A missing one was emptied by another deletion, and moved aside with what it left
            // empty above it.
            if !is_empty_dir(dir)? {
                break;
            }
            self.move_aside(dir, aside)?;
        }
        Ok(())
    }

    /// Undo every step in reverse. Failures are logged, as there is nothing left to fall back to.
    /// Undo every step, latest first, and return `err`. What changed since a step stays: an
    /// object moved aside whose place was taken meanwhile stays where it was moved, which is then
    /// kept, and `err` says where.
    fn undo(mut self, staged: Staged, err: anyhow::Error) -> anyhow::Error {
        let workdir = self.workdir.clone();
        let asides_elsewhere = std::mem::take(&mut self.asides_elsewhere);
        let unrestored = self.undo_steps();
        if unrestored.is_empty() {
            return err;
        }
        for dir in std::iter::once(staged.dir).chain(asides_elsewhere.into_values()) {
            let _kept = dir.keep();
        }
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
            let undone = in_worktree(&self.workdir, step.path()).and_then(|()| match &step {
                Step::Placed { path, stat } => remove_unless_changed(path, stat),
                Step::CreatedDir(dir) => std::fs::remove_dir(dir),
                Step::MovedAside { from, to } => rename_no_replace(to, from),
            });
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

/// Every stat field that changes when a file's content does, except the ctime, which moving it
/// into place may change.
const ALL_BUT_CTIME: gix::index::entry::stat::Options = gix::index::entry::stat::Options {
    trust_ctime: false,
    check_stat: true,
    use_nsec: true,
    use_stdev: false,
};

/// Every stat field that changes when a file does, which includes the ctime when its mode or
/// owner changes.
const ALL_FIELDS: gix::index::entry::stat::Options = gix::index::entry::stat::Options {
    trust_ctime: true,
    ..ALL_BUT_CTIME
};

/// A new file next to `dest` that only checkout knows, created with the permissions Git asks for,
/// which the umask or a default ACL then reduce. It's removed when dropped.
#[cfg_attr(not(unix), allow(unused_variables))]
fn private_file_next_to(dest: &Path, executable: bool) -> std::io::Result<tempfile::NamedTempFile> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".gitbutler-checkout-");
    #[cfg(unix)]
    builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(
        if executable { 0o777 } else { 0o666 },
    ));
    builder.tempfile_in(dest.parent().expect("in the worktree"))
}

/// What the index may record about the file placed at `dest`, which had the `written` stat when
/// it was written. Moving it into place may change only the ctime, which is why the stat is read
/// again. A file that differs otherwise changed after it was written, so its stat can't vouch for
/// the content, and Git has to compare the content instead.
fn trusted_stat(written: &gix::index::entry::Stat, dest: &Path) -> std::io::Result<Stat> {
    let placed = stat_of(&gix::index::fs::Metadata::from_path_no_follow(dest)?)?;
    Ok(match written.matches(&placed, ALL_BUT_CTIME) {
        true => Stat::Known(placed),
        false => Stat::Unknown,
    })
}

fn stat_of(md: &gix::index::fs::Metadata) -> std::io::Result<gix::index::entry::Stat> {
    gix::index::entry::Stat::from_fs(md).map_err(std::io::Error::other)
}

/// Remove the file or symlink checkout placed at `path`, which then had the `placed` stat, unless
/// it changed since.
fn remove_unless_changed(path: &Path, placed: &gix::index::entry::Stat) -> std::io::Result<()> {
    let Ok(md) = gix::index::fs::Metadata::from_path_no_follow(path) else {
        return Ok(());
    };
    match placed.matches(&stat_of(&md)?, ALL_FIELDS) {
        true => gix::fs::symlink::remove(path),
        false => Ok(()),
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

/// What the directories leading to a path in the worktree are.
enum Leading {
    /// All directories, so the path is in the worktree.
    Dirs,
    /// One is missing, so nothing is at the path.
    Missing,
    /// This one is a file or symlink, which appeared after the worktree was checked.
    Obstructed(PathBuf),
}

/// What the directories leading to `path` in `workdir` are, without following symlinks, which
/// could lead out of the worktree.
fn leading_dirs_of(workdir: &Path, path: &Path) -> std::io::Result<Leading> {
    for dir in leading_dirs(workdir, path) {
        match dir.symlink_metadata() {
            Ok(md) if md.is_dir() => {}
            Ok(_) => return Ok(Leading::Obstructed(dir.to_owned())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Leading::Missing),
            Err(err) => return Err(err),
        }
    }
    Ok(Leading::Dirs)
}

/// Fail unless the directories leading to `path` in `workdir` are all directories.
fn in_worktree(workdir: &Path, path: &Path) -> std::io::Result<()> {
    match leading_dirs_of(workdir, path)? {
        Leading::Dirs => Ok(()),
        Leading::Missing | Leading::Obstructed(_) => Err(std::io::Error::other(format!(
            "the directories leading to {path:?} changed"
        ))),
    }
}

/// Whether `path` is a directory, not a symlink to one, with nothing in it.
fn is_empty_dir(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(md) if md.is_dir() => Ok(std::fs::read_dir(path)?.next().is_none()),
        Ok(_) => Ok(false),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
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

/// A new staging directory in the Git directory.
fn staging_dir(repo: &gix::Repository) -> anyhow::Result<tempfile::TempDir> {
    let in_git_dir = repo.git_dir().join("gitbutler");
    std::fs::create_dir_all(&in_git_dir)?;
    Ok(tempfile::Builder::new()
        .prefix(".gitbutler-checkout-")
        .tempdir_in(in_git_dir)?)
}

/// The directory nearest the root of `workdir`, `workdir` included, among those leading to
/// `path` that are on the filesystem `path` is on.
fn top_of_filesystem(workdir: &Path, path: &Path) -> std::io::Result<PathBuf> {
    let device = |dir: &Path| gix::index::fs::Metadata::from_path_no_follow(dir).map(|md| md.dev());
    let parent = path.parent().expect("in the worktree");
    let on = device(parent)?;
    let mut top = parent;
    for dir in parent.ancestors().skip(1) {
        if !dir.starts_with(workdir) || device(dir)? != on {
            break;
        }
        top = dir;
    }
    Ok(top.to_owned())
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

    /// A file written at a whole second, then moved to `placed`, and its stat when written.
    fn place(dir: &Path) -> anyhow::Result<(std::path::PathBuf, gix::index::entry::Stat)> {
        let private = dir.join("private");
        std::fs::write(&private, "written\n")?;
        set_mtime(
            &private,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
        )?;
        let written = stat(&private)?;
        let placed = dir.join("placed");
        std::fs::rename(&private, &placed)?;
        Ok((placed, written))
    }

    #[test]
    fn a_placed_file_is_trusted_with_its_new_stat() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, written) = place(tmp.path())?;

        assert!(
            matches!(super::trusted_stat(&written, &placed)?, Stat::Known(_)),
            "moving keeps the file's identity"
        );
        Ok(())
    }

    #[test]
    fn a_placed_file_edited_in_the_same_second_is_not_trusted() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, written) = place(tmp.path())?;
        std::fs::write(&placed, "edited\n")?;
        set_mtime(
            &placed,
            SystemTime::UNIX_EPOCH + std::time::Duration::new(1_000_000, 1_000),
        )?;

        assert!(
            matches!(super::trusted_stat(&written, &placed)?, Stat::Unknown),
            "an edit after placing, like an editor saving the file, makes its stat untrustworthy"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn a_placed_file_replaced_by_a_new_file_is_not_trusted() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let (placed, written) = place(tmp.path())?;
        // Editors often save by writing a new file and renaming it over the old one.
        let saved = tmp.path().join("saved");
        std::fs::write(&saved, "edited\n")?;
        set_mtime(
            &saved,
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
        )?;
        std::fs::rename(&saved, &placed)?;

        assert!(
            matches!(super::trusted_stat(&written, &placed)?, Stat::Unknown),
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

    /// Stage `content` as the new file `name`, and return where it is.
    fn stage(
        staged: &super::Staged,
        name: &str,
        content: &str,
    ) -> anyhow::Result<std::path::PathBuf> {
        let path = staged.new_files().join(name);
        std::fs::write(&path, content)?;
        Ok(path)
    }

    #[test]
    fn undo_keeps_a_file_saved_where_one_was_moved_aside_and_says_where_that_one_is()
    -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let a = workdir.path().join("a");
        let new_a = stage(&staged, "a", "target\n")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.move_aside(&a, &staged.aside())?;
        std::fs::write(&a, "saved meanwhile\n")?;

        let refused = journal
            .place(&new_a, &a)
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
        let new_b = stage(&staged, "b", "target\n")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.place(&new_b, &b)?;
        std::fs::write(&b, "saved after placing\n")?;

        journal.undo(staged, anyhow::anyhow!("a later step failed"));

        assert_eq!(std::fs::read_to_string(&b)?, "saved after placing\n");
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn undo_keeps_a_placed_file_made_executable_since() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let (workdir, staged) = worktree_and_staging()?;
        let b = workdir.path().join("b");
        let new_b = stage(&staged, "b", "target\n")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.place(&new_b, &b)?;
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o755))?;

        journal.undo(staged, anyhow::anyhow!("a later step failed"));

        assert!(b.exists(), "the new mode is kept, and the file with it");
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn undo_keeps_a_backup_whose_directory_became_a_symlink() -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let d = workdir.path().join("d");
        std::fs::create_dir(&d)?;
        std::fs::write(d.join("a"), "before\n")?;
        std::fs::write(d.join("other"), "")?;
        let mut journal = super::Journal::new(workdir.path());
        journal.move_aside(&d.join("a"), &staged.aside())?;
        let outside = tempfile::tempdir()?;
        std::fs::rename(&d, outside.path().join("d"))?;
        std::os::unix::fs::symlink(outside.path().join("d"), &d)?;
        let staging_dir = staged.dir.path().to_owned();

        let err = journal.undo(staged, anyhow::anyhow!("a later step failed"));

        assert!(
            !outside.path().join("d/a").exists(),
            "nothing is restored outside the worktree"
        );
        assert!(err.to_string().contains("\"d/a\" at"), "{err}");
        std::fs::remove_dir_all(staging_dir)?;
        Ok(())
    }

    /// Content that can't be read, after an editor saved the file at the given path.
    enum FailingRead {
        Plainly,
        AfterSavingOver(std::path::PathBuf),
        AfterWritingInto(std::path::PathBuf),
    }

    impl std::io::Read for FailingRead {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            match std::mem::replace(self, FailingRead::Plainly) {
                FailingRead::Plainly => {}
                FailingRead::AfterSavingOver(dest) => {
                    let saved = dest.with_file_name("saved");
                    std::fs::write(&saved, "saved\n")?;
                    std::fs::rename(&saved, &dest)?;
                }
                FailingRead::AfterWritingInto(dest) => std::fs::write(&dest, "saved\n")?,
            }
            Err(std::io::Error::other("no space left"))
        }
    }

    /// Fail to write `b` while reading the `content` made for its path, undo, and return what's
    /// in the worktree then.
    fn fail_to_write_b(
        content: impl FnOnce(std::path::PathBuf) -> FailingRead,
    ) -> anyhow::Result<Vec<(String, String)>> {
        let (workdir, staged) = worktree_and_staging()?;
        let b = workdir.path().join("b");
        let mut content = content(b.clone());
        let mut journal = super::Journal::new(workdir.path());
        let err = journal.create_file(&mut content, false, &b);
        journal.undo(staged, anyhow::anyhow!("placing failed"));
        assert!(err.is_err());
        let mut files = std::fs::read_dir(workdir.path())?
            .map(|entry| {
                let path = entry?.path();
                let name = path
                    .file_name()
                    .expect("a file")
                    .to_string_lossy()
                    .into_owned();
                Ok((name, std::fs::read_to_string(&path)?))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        files.sort();
        Ok(files)
    }

    #[test]
    fn a_file_that_failed_to_be_written_leaves_nothing_behind() -> anyhow::Result<()> {
        let files = fail_to_write_b(|_| FailingRead::Plainly)?;
        assert_eq!(files, [("a".into(), "before\n".into())]);
        Ok(())
    }

    #[test]
    fn undo_keeps_a_file_saved_where_one_failed_to_be_written() -> anyhow::Result<()> {
        for save in [FailingRead::AfterSavingOver, FailingRead::AfterWritingInto] {
            let files = fail_to_write_b(save)?;
            assert_eq!(
                files,
                [
                    ("a".into(), "before\n".into()),
                    ("b".into(), "saved\n".into())
                ]
            );
        }
        Ok(())
    }
}
