//! Writing a checkout's changes so that they either all happen or none do.
//!
//! Only filter drivers, like Git LFS or git-crypt, can fail to convert a file, so the files they
//! convert are first checked out into a staging directory. Only then are the objects in the way
//! moved aside, or removed if they're empty directories, the other files written in place and the
//! staged files copied into place, with each step journaled so a failure can put everything back.

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
    /// Removed when dropped, along with the objects moved aside into it, unless undoing the
    /// commit couldn't put one back.
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

    fn aside(&self) -> PathBuf {
        self.dir.path().join("old")
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
    std::fs::create_dir(dir.path().join("old"))?;
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

/// Move the `clearances` aside, copy the staged files into place and write the plain files in
/// place, last, so that the directories symlinks lead to exist when gix creates them. On failure,
/// undo every step and return the error, leaving the worktree as it was, except for what changed
/// meanwhile.
pub(super) fn commit(
    repo: &gix::Repository,
    staged: Staged,
    clearances: &[Clearance],
) -> anyhow::Result<Placed> {
    let workdir = repo.workdir().expect("checked by the caller");
    let mut journal = Journal::new(workdir);
    match apply(&mut journal, repo, workdir, &staged, clearances) {
        Ok(placed) => {
            journal.complete(staged);
            Ok(placed)
        }
        Err(err) => Err(journal.undo(staged, err)),
    }
}

fn apply(
    journal: &mut Journal,
    repo: &gix::Repository,
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
        journal.clear(&join(workdir, rela_path)?, Reason::Deleted, &aside)?;
    }
    for Clearance { rela_path, .. } in with_reason(Reason::Deleted) {
        journal.remove_emptied_parents(&join(workdir, rela_path)?);
    }
    for Clearance { rela_path, .. } in with_reason(Reason::InTheWay) {
        journal.clear(&join(workdir, rela_path)?, Reason::InTheWay, &aside)?;
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
    stats.extend(journal.write_in_place(repo, &staged.plain)?);
    Ok(Placed { stats })
}

/// What the commit did, in order, so it can be undone.
struct Journal {
    workdir: PathBuf,
    steps: Vec<Step>,
    /// Where objects that can't be moved into the staging directory, as they're on another
    /// filesystem, are moved aside instead, by the directory in the worktree they're in.
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
    RemovedDir {
        path: PathBuf,
        permissions: std::fs::Permissions,
    },
}

impl Step {
    /// The path in the worktree the step changed.
    fn path(&self) -> &Path {
        match self {
            Step::MovedAside { from, .. } => from,
            Step::Placed { path, .. } => path,
            Step::CreatedDir(dir) => dir,
            Step::RemovedDir { path, .. } => path,
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

    /// Clear the object at `path`, which checkout removes for `reason`: remove it if it's an empty
    /// directory, and move it `aside` otherwise. A directory the target deletes was empty when
    /// checked, so like Git, it only goes if it still is and can be removed, keeping anything
    /// created in it since. A missing directory leading to it leaves nothing to clear, and a file or
    /// symlink there appeared after the worktree was checked, so refuse rather than move through
    /// it, maybe out of the worktree.
    fn clear(&mut self, path: &Path, reason: Reason, aside: &Path) -> anyhow::Result<()> {
        match leading_dirs_of(&self.workdir, path)? {
            Leading::Dirs => {}
            Leading::Missing => return Ok(()),
            Leading::Obstructed(dir) => return Err(self.appeared_after_the_check(&dir)),
        }
        match std::fs::symlink_metadata(path) {
            Ok(md)
                if md.is_dir()
                    && (self.remove_empty_dir(path, md.permissions())
                        || reason == Reason::Deleted) =>
            {
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => self.move_aside(path, aside),
        }
    }

    /// Remove the directory `dir` if it's empty, journaling its permissions so undo can create it
    /// again, and return whether it was removed.
    fn remove_empty_dir(&mut self, dir: &Path, permissions: std::fs::Permissions) -> bool {
        let removed = std::fs::remove_dir(dir).is_ok();
        if removed {
            self.steps.push(Step::RemovedDir {
                path: dir.to_owned(),
                permissions,
            });
        }
        removed
    }

    /// Move the object at `from` into its own place in the directory `aside`, so objects moved
    /// aside never collide, even if one was inside another. An object that is gone loses
    /// nothing.
    fn move_aside(&mut self, from: &Path, aside: &Path) -> anyhow::Result<()> {
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
        let to = aside.join(&place);
        match std::fs::rename(from, &to) {
            Err(err) if err.kind() == std::io::ErrorKind::CrossesDevices => {
                let to = self.aside_on_the_filesystem_of(from)?.join(place);
                std::fs::rename(from, &to).map(|()| to)
            }
            moved => moved.map(|()| to),
        }
    }

    /// Remove what was moved aside, now that the commit succeeded.
    fn complete(self, staged: Staged) {
        remove_asides(std::iter::once(staged.dir).chain(self.asides_elsewhere.into_values()));
    }

    /// A private directory on the filesystem `path` is on in the worktree, at its top, or if
    /// checkout may not write there, as near as it may. That's the parent of `path` at the latest,
    /// which moving `path` needs to write to anyway.
    fn aside_on_the_filesystem_of(&mut self, path: &Path) -> std::io::Result<PathBuf> {
        let mut denied = None;
        for dir in dirs_on_the_filesystem_of(&self.workdir, path)? {
            match self.aside_in(&dir) {
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    denied = Some(err)
                }
                found => return found,
            }
        }
        Err(denied.unwrap_or_else(|| std::io::ErrorKind::PermissionDenied.into()))
    }

    /// The private directory in `dir` for objects moved aside, created on first use.
    fn aside_in(&mut self, dir: &Path) -> std::io::Result<PathBuf> {
        if let Some(aside) = self.asides_elsewhere.get(dir) {
            return Ok(aside.path().to_owned());
        }
        let aside = private_builder().tempdir_in(dir)?;
        let path = aside.path().to_owned();
        self.asides_elsewhere.insert(dir.to_owned(), aside);
        Ok(path)
    }

    /// Write the files `index` doesn't skip in place with gix, journaling the directories created
    /// for them and every file gix created, even one it failed to finish, and return what the index
    /// may record about those written. Nothing at their places is replaced, as anything there
    /// appeared after the worktree was checked, and no directory leading to them is a symlink.
    fn write_in_place(
        &mut self,
        repo: &gix::Repository,
        index: &gix::index::State,
    ) -> anyhow::Result<BTreeMap<BString, Stat>> {
        let skipped = |entry: &gix::index::Entry| {
            entry
                .flags
                .contains(gix::index::entry::Flags::SKIP_WORKTREE)
        };
        if index.entries().iter().all(skipped) {
            return Ok(BTreeMap::new());
        }
        let mut dirs = BTreeSet::new();
        for entry in index.entries().iter().filter(|entry| !skipped(entry)) {
            let path = join(&self.workdir, entry.path(index))?;
            dirs.extend(
                leading_dirs(&self.workdir, &path)
                    .into_iter()
                    .map(Path::to_owned),
            );
        }
        for dir in &dirs {
            self.ensure_one_dir(dir)?;
        }
        let mut index = index.clone();
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
        let collided: BTreeSet<&BStr> = outcome
            .collisions
            .iter()
            .map(|collision| collision.path.as_ref())
            .collect();
        let mut stats = BTreeMap::new();
        for entry in index.entries().iter().filter(|entry| !skipped(entry)) {
            let rela_path = entry.path(&index);
            if collided.contains(rela_path) {
                continue;
            }
            let path = join(&self.workdir, rela_path)?;
            if entry.stat != Default::default() {
                self.steps.push(Step::Placed {
                    path,
                    stat: entry.stat,
                });
                stats.insert(rela_path.to_owned(), Stat::Known(entry.stat));
            } else if let Ok(stat) =
                gix::index::fs::Metadata::from_path_no_follow(&path).and_then(|md| stat_of(&md))
            {
                self.steps.push(Step::Placed { path, stat });
            }
        }
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
        Ok(stats)
    }

    /// Create a copy of the file or symlink at `staged` at `dest`, unless something appeared
    /// there, and return what the index may record about it. The staged copy is removed once
    /// placed, so staging doesn't hold a second copy of every file it placed.
    fn place(&mut self, staged: &Path, dest: &Path) -> anyhow::Result<Stat> {
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
        &mut self,
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
    fn create_symlink(&mut self, target: &Path, dest: &Path) -> std::io::Result<Stat> {
        let private = private_builder()
            .make_in(parent_of(dest), |path| {
                gix::fs::symlink::create(target, path)
            })?
            .into_temp_path();
        let written = stat_of(&gix::index::fs::Metadata::from_path_no_follow(&private)?)?;
        self.install(private, &written, dest)
    }

    /// Move the `private` file or symlink, which had the `written` stat before anything else could
    /// see it, to `dest` unless something appeared there, and journal it.
    fn install(
        &mut self,
        mut private: tempfile::TempPath,
        written: &gix::index::entry::Stat,
        dest: &Path,
    ) -> std::io::Result<Stat> {
        rename_no_replace(&private, dest)?;
        private.disable_cleanup(true);
        self.placed(dest, written)
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

    /// Like Git, remove the directories leading to the deleted `path` that it left empty, as far as
    /// it can: one that can't be removed, like in a read-only directory, stays, and so does one
    /// behind a symlink, which isn't in the worktree.
    fn remove_emptied_parents(&mut self, path: &Path) {
        let workdir = self.workdir.clone();
        if !matches!(leading_dirs_of(&workdir, path), Ok(Leading::Dirs)) {
            return;
        }
        for dir in path.ancestors().skip(1).take_while(|dir| *dir != workdir) {
            match dir.symlink_metadata() {
                Ok(md) if self.remove_empty_dir(dir, md.permissions()) => {}
                _ => break,
            }
        }
    }

    /// Undo every step, latest first, and return `err`. What changed since a step stays: an
    /// object moved aside whose place was taken meanwhile stays where it was moved, which is then
    /// kept, and `err` says where.
    fn undo(mut self, staged: Staged, err: anyhow::Error) -> anyhow::Error {
        let workdir = self.workdir.clone();
        let asides = std::iter::once(staged.dir)
            .chain(std::mem::take(&mut self.asides_elsewhere).into_values())
            .collect::<Vec<_>>();
        let unrestored = self.undo_steps();
        if unrestored.is_empty() {
            remove_asides(asides);
            return err;
        }
        for dir in asides {
            let _kept = dir.keep();
        }
        let kept: Vec<String> = unrestored
            .iter()
            .map(|(from, to)| {
                let rela_path = from.strip_prefix(&workdir).unwrap_or(from);
                format!("\"{}\" at \"{}\"", rela_path.display(), to.display())
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
                Step::RemovedDir { path, permissions } => std::fs::create_dir(path)
                    .and_then(|()| std::fs::set_permissions(path, permissions.clone())),
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

/// Remove the directories holding what checkout moved aside, logging those that can't be.
fn remove_asides(dirs: impl IntoIterator<Item = tempfile::TempDir>) {
    for dir in dirs {
        let path = dir.path().to_owned();
        if let Err(err) = dir.close() {
            tracing::warn!(?err, ?path, "could not remove what checkout moved aside");
        }
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

/// The directories leading to `path` in `workdir`, `workdir` included, that are on the
/// filesystem `path` is on, from the top down.
fn dirs_on_the_filesystem_of(workdir: &Path, path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let device = |dir: &Path| gix::index::fs::Metadata::from_path_no_follow(dir).map(|md| md.dev());
    let parent = parent_of(path);
    let on = device(parent)?;
    let mut dirs = Vec::new();
    for dir in parent.ancestors() {
        if !dir.starts_with(workdir) || device(dir)? != on {
            break;
        }
        dirs.push(dir.to_owned());
    }
    dirs.reverse();
    Ok(dirs)
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
            plain: gix::index::State::new(gix::hash::Kind::Sha1),
        };
        std::fs::create_dir(staged.new_files())?;
        std::fs::create_dir(staged.aside())?;
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
                 previous versions were kept elsewhere: \"a\" at \"{}\": Uncommitted files \
                 would be overwritten by checkout: \"a\"",
                backup.display()
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

    #[test]
    fn placing_a_file_frees_its_staged_copy() -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let new_b = stage(&staged, "b", "b\n")?;
        let mut journal = super::Journal::new(workdir.path());

        journal.place(&new_b, &workdir.path().join("b"))?;

        assert_eq!(std::fs::read(workdir.path().join("b"))?, b"b\n");
        assert!(
            !new_b.exists(),
            "staging holds no second copy of a file once it's in place"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn a_deleted_directory_that_cannot_be_removed_stays() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let (tmp, repo, staged) = plain_writes(&[])?;
        let parent = tmp.path().join("p");
        std::fs::create_dir_all(parent.join("e"))?;
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555))?;

        let removed = super::commit(
            &repo,
            staged,
            &[super::Clearance {
                rela_path: "p/e".into(),
                reason: super::Reason::Deleted,
            }],
        );
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
        let (tmp, repo, staged) = plain_writes(&[])?;
        std::fs::create_dir(tmp.path().join("e"))?;
        std::fs::write(tmp.path().join("e/new"), "new\n")?;

        super::commit(
            &repo,
            staged,
            &[super::Clearance {
                rela_path: "e".into(),
                reason: super::Reason::Deleted,
            }],
        )?;

        assert_eq!(
            std::fs::read_to_string(tmp.path().join("e/new"))?,
            "new\n",
            "the check found `e` empty, so a file in it now was created since"
        );
        Ok(())
    }

    /// A repository in a new worktree, and what checkout writes there in place: each `content`
    /// at its path.
    fn plain_writes(
        writes: &[(&str, &str)],
    ) -> anyhow::Result<(tempfile::TempDir, gix::Repository, super::Staged)> {
        let tmp = tempfile::tempdir()?;
        let repo = gix::init(tmp.path())?;
        let mut plain = gix::index::State::new(repo.object_hash());
        for (rela_path, content) in writes {
            let id = repo.write_blob(content)?.detach();
            plain.dangerously_push_entry(
                Default::default(),
                id,
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                (*rela_path).into(),
            );
        }
        plain.sort_entries();
        let staged = super::Staged {
            dir: tempfile::tempdir()?,
            files: Vec::new(),
            submodules: Vec::new(),
            plain,
        };
        std::fs::create_dir(staged.new_files())?;
        std::fs::create_dir(staged.aside())?;
        Ok((tmp, repo, staged))
    }

    #[test]
    fn a_symlink_written_in_place_leads_to_a_directory_of_staged_files() -> anyhow::Result<()> {
        let (tmp, repo, mut staged) = plain_writes(&[("link", "assets")])?;
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
        let (tmp, repo, staged) = plain_writes(&[("d/a", "new\n")])?;

        let placed = super::commit(&repo, staged, &[])?;

        assert_eq!(std::fs::read_to_string(tmp.path().join("d/a"))?, "new\n");
        assert!(matches!(placed.stat("d/a".into()), Stat::Known(_)));
        Ok(())
    }

    #[test]
    fn writing_in_place_refuses_a_file_already_there() -> anyhow::Result<()> {
        let (tmp, repo, staged) = plain_writes(&[("a", "new\n")])?;
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
        let (tmp, repo, staged) = plain_writes(&[("d/a", "new\n")])?;
        let outside = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("d"))?;

        super::commit(&repo, staged, &[]).expect_err("`d` appeared as a symlink");

        assert!(
            !outside.path().join("a").exists(),
            "nothing is written outside the worktree"
        );
        Ok(())
    }

    #[test]
    fn undo_removes_what_was_written_in_place_and_its_directories() -> anyhow::Result<()> {
        let (tmp, repo, staged) = plain_writes(&[("b", "new\n"), ("d/e/a", "new\n")])?;
        std::fs::write(tmp.path().join("b"), "mine\n")?;

        super::commit(&repo, staged, &[]).expect_err("something appeared where `b` goes");

        assert!(
            !tmp.path().join("d").exists(),
            "the file written in place and the directories created for it are gone"
        );
        assert_eq!(std::fs::read_to_string(tmp.path().join("b"))?, "mine\n");
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

    #[test]
    #[cfg(unix)]
    fn undo_keeps_a_symlink_replaced_after_it_was_placed() -> anyhow::Result<()> {
        let (workdir, staged) = worktree_and_staging()?;
        let link = workdir.path().join("link");
        let new_link = staged.new_files().join("link");
        std::os::unix::fs::symlink("target", &new_link)?;
        let mut journal = super::Journal::new(workdir.path());
        journal.place(&new_link, &link)?;
        std::fs::remove_file(&link)?;
        std::os::unix::fs::symlink("target", &link)?;

        journal.undo(staged, anyhow::anyhow!("a later step failed"));

        assert!(
            link.symlink_metadata().is_ok(),
            "a symlink saved over the placed one, with the same target, is another symlink"
        );
        Ok(())
    }
}
