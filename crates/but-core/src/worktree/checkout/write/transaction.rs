//! Writing a checkout's changes once the worktree filters that can fail ran.
//!
//! Only filter drivers, like Git LFS or git-crypt, can fail to convert a file, so the files they
//! convert are first checked out into a staging directory. Only then are the objects in the way
//! removed, the other files written in place, and the staged files copied into place.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use bstr::{BStr, BString};

use super::{
    plan::Plan,
    protect::{Clearance, Reason},
};
use crate::index::Stat;

/// What checkout writes into the worktree: files a filter driver converted, checked out into a
/// staging directory and ready to be copied into place, and the other files to write in place.
pub(super) struct Staged {
    /// Removed when dropped.
    dir: tempfile::TempDir,
    files: Vec<BString>,
    submodules: Vec<BString>,
    /// The full target, with every entry but the files to write in place flagged so gix's
    /// checkout skips it.
    plain: gix::index::State,
}

impl Staged {
    fn new_files(&self) -> PathBuf {
        self.dir.path().join("new")
    }
}

/// What the index should record about each file checkout placed in the worktree.
#[derive(Default, Debug)]
pub(super) struct Placed {
    stats: BTreeMap<BString, Stat>,
}

impl Placed {
    /// What the index should record about the file at `path`.
    pub fn stat(&self, path: &BStr) -> Stat {
        self.stats.get(path).copied().unwrap_or(Stat::Unknown)
    }
}

/// Check out the files `plan` writes that a filter driver converts into a staging directory,
/// running their filters, and set aside the other files to write in place once the worktree was
/// checked.
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
    let filtered = converted_by_drivers(repo, target, plan)?;
    if !filtered.is_empty() {
        let mut index = checkout_index(target, |path| filtered.contains(path));
        gix::worktree::state::checkout(
            &mut index,
            new_files,
            repo.objects.clone().into_arc()?,
            &gix::progress::Discard,
            &gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED,
            repo.checkout_options(ATTRIBUTES)?,
        )?;
    }
    // An interrupted checkout stops writing files, but gix still reports success.
    if gix::interrupt::is_triggered() {
        anyhow::bail!("Checkout was interrupted");
    }
    let (submodules, files) = plan
        .writes()
        .partition::<Vec<_>, _>(|(_, content)| content.is_submodule());
    let plain = checkout_index(target, |path| {
        plan.write_at(path)
            .is_some_and(|content| !content.is_submodule())
            && !filtered.contains(path)
    });
    Ok(Staged {
        dir,
        files: files
            .into_iter()
            .map(|(path, _)| path.to_owned())
            .filter(|path| filtered.contains(path))
            .collect(),
        submodules: submodules
            .into_iter()
            .map(|(path, _)| path.to_owned())
            .collect(),
        plain,
    })
}

/// Where checkout reads `.gitattributes` from: the target it checks out.
const ATTRIBUTES: gix::worktree::stack::state::attributes::Source =
    gix::worktree::stack::state::attributes::Source::IdMapping;

/// The files `plan` writes whose `filter` attribute names a configured driver that converts
/// files for the worktree. Symlinks are never converted. Only such a driver can fail to convert a file: the other conversions
/// can't, or are skipped when they fail.
fn converted_by_drivers(
    repo: &gix::Repository,
    target: &gix::index::State,
    plan: &Plan,
) -> anyhow::Result<BTreeSet<BString>> {
    let drivers: Vec<BString> = gix::filter::Pipeline::options(repo)?
        .drivers
        .into_iter()
        .filter(|driver| driver.smudge.is_some() || driver.process.is_some())
        .map(|driver| driver.name)
        .collect();
    let mut converted = BTreeSet::new();
    if drivers.is_empty() {
        return Ok(converted);
    }
    let mut attributes = repo.attributes_only(target, ATTRIBUTES)?;
    let mut filter = attributes.selected_attribute_matches(Some("filter"));
    for (path, content) in plan.writes() {
        if content.is_submodule() || content.mode == gix::index::entry::Mode::SYMLINK {
            continue;
        }
        let at_path = attributes.at_entry(path, Some(content.mode))?;
        if at_path.matching_attributes(&mut filter)
            && filter.iter().any(|attribute| {
                attribute
                    .assignment
                    .state
                    .as_bstr()
                    .is_some_and(|name| drivers.iter().any(|driver| driver == name))
            })
        {
            converted.insert(path.to_owned());
        }
    }
    Ok(converted)
}

/// Remove the `clearances`, copy the staged files into place and write the plain files in place,
/// last, so that the directories symlinks lead to exist when gix creates them. Like Git, a failure
/// partway leaves what was done so far.
pub(super) fn commit(
    repo: &gix::Repository,
    mut staged: Staged,
    clearances: &[Clearance],
) -> anyhow::Result<Placed> {
    let writer = Writer {
        workdir: repo.workdir().expect("checked by the caller").to_owned(),
    };
    writer.remove_all(clearances)?;
    let plain = std::mem::replace(
        &mut staged.plain,
        gix::index::State::new(repo.object_hash()),
    );
    let mut placed = writer.place_all(&staged)?;
    placed
        .stats
        .extend(writer.write_in_place(repo, plain)?.stats);
    Ok(placed)
}

/// Writes to the worktree at `workdir`, never through a symlink that appeared after the check.
struct Writer {
    workdir: PathBuf,
}

impl Writer {
    /// Remove the `clearances`. Like Git, remove the directories that the deletions leave empty
    /// while the files to be replaced are still in place, so a directory the target still needs
    /// keeps its permissions, and one it names differently, or needs as a submodule, is created
    /// afresh.
    fn remove_all(&self, clearances: &[Clearance]) -> anyhow::Result<()> {
        let with_reason = |reason| clearances.iter().filter(move |c| c.reason == reason);
        for Clearance { rela_path, reason } in with_reason(Reason::Deleted) {
            self.remove(&join(&self.workdir, rela_path)?, *reason)?;
        }
        for Clearance { rela_path, .. } in with_reason(Reason::Deleted) {
            self.remove_emptied_parents(&join(&self.workdir, rela_path)?);
        }
        for Clearance { rela_path, reason } in with_reason(Reason::InTheWay) {
            self.remove(&join(&self.workdir, rela_path)?, *reason)?;
        }
        Ok(())
    }

    /// Write the files `index` doesn't skip in place with gix, and return what the index may record
    /// about them. Nothing at their places is replaced, as anything there appeared after the
    /// worktree was checked, and no directory leading to them is a symlink.
    fn write_in_place(
        &self,
        repo: &gix::Repository,
        mut index: gix::index::State,
    ) -> anyhow::Result<Placed> {
        let skipped = |entry: &gix::index::Entry| {
            entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        };
        if index.entries().iter().all(skipped) {
            return Ok(Placed::default());
        }
        let mut opts = repo.checkout_options(ATTRIBUTES)?;
        opts.destination_is_initially_empty = true;
        opts.overwrite_existing = false;
        opts.keep_going = true;
        let outcome = gix::worktree::state::checkout(
            &mut index,
            self.workdir.clone(),
            repo.objects.clone().into_arc()?,
            &gix::progress::Discard,
            &gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED,
            opts,
        )?;
        if gix::interrupt::is_triggered() {
            anyhow::bail!("Checkout was interrupted");
        }
        if !outcome.collisions.is_empty() {
            let mut paths: Vec<String> = outcome
                .collisions
                .iter()
                .map(|collision| format!("{:?}", collision.path.to_string()))
                .collect();
            paths.sort();
            return Err(crate::worktree::checkout::UncommittedChangesInTheWay::refusal(&paths));
        }
        if let Some(failed) = outcome.errors.first() {
            return Err(anyhow::anyhow!("{:#}", failed.error))
                .with_context(|| format!("writing {:?}", failed.path));
        }
        let stats = index
            .entries()
            .iter()
            .filter(|entry| !skipped(entry))
            .map(|entry| (entry.path(&index).to_owned(), Stat::Known(entry.stat)))
            .collect();
        Ok(Placed { stats })
    }

    /// Copy the staged files into place, and create the directories of staged submodules.
    fn place_all(&self, staged: &Staged) -> anyhow::Result<Placed> {
        let mut stats = BTreeMap::new();
        for rela_path in &staged.files {
            let dest = join(&self.workdir, rela_path)?;
            let stat = self.place(&join(&staged.new_files(), rela_path)?, &dest)?;
            stats.insert(rela_path.clone(), stat);
        }
        for rela_path in &staged.submodules {
            self.ensure_dir(&join(&self.workdir, rela_path)?)?;
        }
        Ok(Placed { stats })
    }

    /// Remove the object at `path`, which checkout removes for `reason`. A directory in the way
    /// goes with everything in it, which was checked. A directory the target deletes was empty when
    /// checked, so like Git, it only goes if it still is and can be removed, keeping anything
    /// created in it since. A missing
    /// directory leading to it leaves nothing to remove, and a file or symlink there appeared after
    /// the worktree was checked, so refuse rather than remove through it, maybe outside the
    /// worktree.
    fn remove(&self, path: &Path, reason: Reason) -> anyhow::Result<()> {
        match leading_dirs_of(&self.workdir, path)? {
            Leading::Dirs => {}
            Leading::Missing => return Ok(()),
            Leading::Obstructed(dir) => return Err(self.appeared_after_the_check(&dir)),
        }
        let removed = match std::fs::symlink_metadata(path) {
            Ok(md) if md.is_dir() => match reason {
                Reason::InTheWay => std::fs::remove_dir_all(path),
                Reason::Deleted => {
                    std::fs::remove_dir(path).ok();
                    Ok(())
                }
            },
            Ok(md) if md.is_symlink() => gix::fs::symlink::remove(path),
            Ok(_) => std::fs::remove_file(path),
            Err(err) => Err(err),
        };
        match removed {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            removed => removed.with_context(|| format!("removing {path:?}")),
        }
    }

    /// Like Git, remove the directories leading to the deleted `path` that it left empty, as far as
    /// it can: one that can't be removed, like in a read-only directory, stays.
    fn remove_emptied_parents(&self, path: &Path) {
        for dir in path
            .ancestors()
            .skip(1)
            .take_while(|dir| *dir != self.workdir)
        {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }

    /// Create a copy of the file or symlink at `staged` at `dest`, unless something appeared
    /// there, and return what the index may record about it. The staged copy is removed once
    /// placed, so staging doesn't hold a second copy of every file it placed.
    fn place(&self, staged: &Path, dest: &Path) -> anyhow::Result<Stat> {
        self.create_parents(dest)?;
        let md = gix::index::fs::Metadata::from_path_no_follow(staged)?;
        let placed = match md.is_symlink() {
            true => self.create_symlink(&std::fs::read_link(staged)?, dest),
            false => std::fs::File::open(staged)
                .and_then(|mut file| self.create_file(&mut file, md.is_executable(), dest)),
        };
        match placed {
            Ok(stat) => {
                std::fs::remove_file(staged).ok();
                Ok(stat)
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(self.appeared_after_the_check(dest))
            }
            Err(err) => Err(err).with_context(|| format!("placing {dest:?}")),
        }
    }

    /// Create a file with `content` at `dest` like Git creates files, so it gets what its
    /// directory gives new files, like a default ACL or its group. It's written privately next to
    /// `dest` and [installed](Self::install), so `dest` only ever has all of it, and nothing written
    /// into it meanwhile is taken for checkout's.
    fn create_file(
        &self,
        content: &mut impl std::io::Read,
        executable: bool,
        dest: &Path,
    ) -> std::io::Result<Stat> {
        let mut private = private_file_next_to(dest, executable)?;
        std::io::copy(
            &mut std::io::BufReader::with_capacity(1 << 20, content),
            private.as_file_mut(),
        )?;
        let written = stat_of(&gix::index::fs::Metadata::from_file(private.as_file())?)?;
        self.install(private.into_temp_path(), &written, dest)
    }

    /// Create a symlink to `target` at `dest`, privately next to it first like a file.
    fn create_symlink(&self, target: &Path, dest: &Path) -> std::io::Result<Stat> {
        let private = private_builder()
            .make_in(parent_of(dest), |path| {
                gix::fs::symlink::create(target, path)
            })?
            .into_temp_path();
        let written = stat_of(&gix::index::fs::Metadata::from_path_no_follow(&private)?)?;
        self.install(private, &written, dest)
    }

    /// Move the `private` file or symlink, which had the `written` stat before anything else could
    /// see it, to `dest` unless something appeared there, and return what the index may record
    /// about it.
    fn install(
        &self,
        mut private: tempfile::TempPath,
        written: &gix::index::entry::Stat,
        dest: &Path,
    ) -> std::io::Result<Stat> {
        rename_no_replace(&private, dest)?;
        private.disable_cleanup(true);
        trusted_stat(written, dest)
    }

    /// Create the directory `dir` and its parents, unless it exists.
    fn ensure_dir(&self, dir: &Path) -> anyhow::Result<()> {
        self.create_parents(dir)?;
        self.ensure_one_dir(dir)
    }

    /// Create the missing directories leading to `path`, from the worktree's root down.
    fn create_parents(&self, path: &Path) -> anyhow::Result<()> {
        for dir in leading_dirs(&self.workdir, path) {
            self.ensure_one_dir(dir)?;
        }
        Ok(())
    }

    /// Create the directory `dir` unless it is one. A file or symlink there appeared after the
    /// worktree was checked, so refuse rather than write through it, maybe outside the worktree.
    fn ensure_one_dir(&self, dir: &Path) -> anyhow::Result<()> {
        match dir.symlink_metadata() {
            Ok(md) if md.is_dir() => Ok(()),
            Ok(_) => Err(self.appeared_after_the_check(dir)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(dir).with_context(|| format!("creating {dir:?}"))
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
}

/// Every stat field that changes when a file's content does, except the ctime, which moving it
/// into place may change.
const ALL_BUT_CTIME: gix::index::entry::stat::Options = gix::index::entry::stat::Options {
    trust_ctime: false,
    check_stat: true,
    use_nsec: true,
    use_stdev: false,
};

/// A new file next to `dest` that only checkout knows, created with the permissions Git asks for,
/// which the umask or a default ACL then reduce. It's removed when dropped.
#[cfg_attr(not(unix), allow(unused_variables, unused_mut))]
fn private_file_next_to(dest: &Path, executable: bool) -> std::io::Result<tempfile::NamedTempFile> {
    let mut builder = private_builder();
    #[cfg(unix)]
    builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(
        if executable { 0o777 } else { 0o666 },
    ));
    builder.tempfile_in(parent_of(dest))
}

/// A builder of checkout's private files and directories, which are named alike.
fn private_builder() -> tempfile::Builder<'static, 'static> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".gitbutler-checkout-");
    builder
}

fn parent_of(path: &Path) -> &Path {
    path.parent().expect("in the worktree")
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

/// The full `target`, with every entry but those `checked_out` flagged so gix's checkout skips
/// it. The other entries still provide `.gitattributes` to the filters.
fn checkout_index(
    target: &gix::index::State,
    checked_out: impl Fn(&BStr) -> bool,
) -> gix::index::State {
    let mut index = target.clone();
    for (entry, path) in index.entries_mut_with_paths() {
        if !checked_out(path) {
            entry.flags.insert(gix::index::entry::Flags::SKIP_WORKTREE);
        }
    }
    index
}

/// A new staging directory in the Git directory.
fn staging_dir(repo: &gix::Repository) -> anyhow::Result<tempfile::TempDir> {
    let in_git_dir = repo.git_dir().join("gitbutler");
    std::fs::create_dir_all(&in_git_dir)?;
    Ok(private_builder().tempdir_in(in_git_dir)?)
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

    #[test]
    fn placing_a_file_frees_its_staged_copy() -> anyhow::Result<()> {
        let (workdir, staging) = (tempfile::tempdir()?, tempfile::tempdir()?);
        let staged = staging.path().join("a");
        std::fs::write(&staged, "a\n")?;
        let writer = super::Writer {
            workdir: workdir.path().to_owned(),
        };

        writer.place(&staged, &workdir.path().join("a"))?;

        assert_eq!(std::fs::read(workdir.path().join("a"))?, b"a\n");
        assert!(
            !staged.exists(),
            "staging holds no second copy of a file once it's in place"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn a_deleted_directory_that_cannot_be_removed_stays() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let workdir = tempfile::tempdir()?;
        let parent = workdir.path().join("p");
        std::fs::create_dir_all(parent.join("e"))?;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555))?;
        let writer = super::Writer {
            workdir: workdir.path().to_owned(),
        };

        let removed = writer.remove_all(&[super::Clearance {
            rela_path: "p/e".into(),
            reason: super::Reason::Deleted,
        }]);
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755))?;

        removed?;
        assert!(
            parent.join("e").is_dir(),
            "like Git, an empty directory checkout can't remove stays"
        );
        Ok(())
    }

    #[test]
    fn a_deleted_directory_keeps_a_file_created_in_it_after_the_check() -> anyhow::Result<()> {
        let workdir = tempfile::tempdir()?;
        std::fs::create_dir(workdir.path().join("e"))?;
        std::fs::write(workdir.path().join("e/new"), "new\n")?;
        let writer = super::Writer {
            workdir: workdir.path().to_owned(),
        };

        writer.remove_all(&[super::Clearance {
            rela_path: "e".into(),
            reason: super::Reason::Deleted,
        }])?;

        assert_eq!(
            std::fs::read_to_string(workdir.path().join("e/new"))?,
            "new\n",
            "the check found `e` empty, so a file in it now was created since"
        );
        Ok(())
    }

    /// A repository in a new worktree, and what checkout writes there in place: `content` at
    /// `rela_path`.
    fn plain_write(
        rela_path: &str,
        content: &str,
    ) -> anyhow::Result<(tempfile::TempDir, gix::Repository, super::Staged)> {
        let tmp = tempfile::tempdir()?;
        let repo = gix::init(tmp.path())?;
        let id = repo.write_blob(content)?.detach();
        let mut plain = gix::index::State::new(repo.object_hash());
        plain.dangerously_push_entry(
            Default::default(),
            id,
            gix::index::entry::Flags::empty(),
            gix::index::entry::Mode::FILE,
            rela_path.into(),
        );
        let staged = super::Staged {
            dir: tempfile::tempdir()?,
            files: Vec::new(),
            submodules: Vec::new(),
            plain,
        };
        Ok((tmp, repo, staged))
    }

    #[test]
    fn a_symlink_written_in_place_leads_to_a_directory_of_staged_files() -> anyhow::Result<()> {
        let (tmp, repo, mut staged) = plain_write("link", "assets")?;
        staged.plain.entries_mut()[0].mode = gix::index::entry::Mode::SYMLINK;
        std::fs::create_dir_all(staged.new_files().join("assets"))?;
        std::fs::write(staged.new_files().join("assets/data.bin"), "data\n")?;
        staged.files.push("assets/data.bin".into());

        super::commit(&repo, staged, &[])?;

        assert!(
            std::fs::metadata(tmp.path().join("link"))?.is_dir(),
            "on Windows, a link made before the directory exists is a file link, which can't lead to it"
        );
        Ok(())
    }

    #[test]
    fn writing_in_place_records_what_the_index_may_trust() -> anyhow::Result<()> {
        let (tmp, repo, staged) = plain_write("d/a", "new\n")?;

        let placed = super::commit(&repo, staged, &[])?;

        assert_eq!(std::fs::read_to_string(tmp.path().join("d/a"))?, "new\n");
        assert!(matches!(placed.stat("d/a".into()), Stat::Known(_)));
        Ok(())
    }

    #[test]
    fn writing_in_place_refuses_a_file_already_there() -> anyhow::Result<()> {
        let (tmp, repo, staged) = plain_write("a", "new\n")?;
        std::fs::write(tmp.path().join("a"), "mine\n")?;

        let err = super::commit(&repo, staged, &[]).expect_err("something appeared where `a` goes");

        assert!(
            err.to_string()
                .contains("Uncommitted files would be overwritten by checkout"),
            "{err}"
        );
        assert_eq!(std::fs::read_to_string(tmp.path().join("a"))?, "mine\n");
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn writing_in_place_refuses_to_write_through_a_symlinked_directory() -> anyhow::Result<()> {
        let (tmp, repo, staged) = plain_write("d/a", "new\n")?;
        let outside = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("d"))?;

        super::commit(&repo, staged, &[]).expect_err("`d` appeared as a symlink");

        assert!(
            !outside.path().join("a").exists(),
            "nothing is written outside the worktree"
        );
        Ok(())
    }

    /// Content that can't be read.
    struct FailingRead;

    impl std::io::Read for FailingRead {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("no space left"))
        }
    }

    #[test]
    fn a_file_that_failed_to_be_written_leaves_nothing_behind() -> anyhow::Result<()> {
        let workdir = tempfile::tempdir()?;
        let writer = super::Writer {
            workdir: workdir.path().to_owned(),
        };

        writer
            .create_file(&mut FailingRead, false, &workdir.path().join("b"))
            .expect_err("reading fails");

        assert_eq!(
            std::fs::read_dir(workdir.path())?.count(),
            0,
            "neither `b` nor a private file is left"
        );
        Ok(())
    }
}
