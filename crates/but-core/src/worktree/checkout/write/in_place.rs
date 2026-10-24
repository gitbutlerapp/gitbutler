//! Writing a checkout's changes to the worktree in place, like Git.

use std::{collections::BTreeMap, path::PathBuf};
#[cfg(unix)]
use std::{collections::BTreeSet, path::Path};

use anyhow::Context as _;
use bstr::{BStr, BString};
#[cfg(unix)]
use bstr::{ByteSlice as _, ByteVec as _};

use super::plan::Plan;
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
pub(super) fn write(
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
    let mut opts =
        repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    opts.overwrite_existing = true;
    let objects = repo.objects.clone().into_arc()?;
    let replaced = Replaced::move_aside(&mut path_check, plan)?;
    // Without `keep_going`, files that fail to be written, or that a filter delays and never
    // delivers, fail the checkout here, before the index records them.
    let checkout = gix::worktree::state::checkout(
        &mut index,
        workdir,
        objects,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    );
    match checkout {
        // An interrupted checkout stops writing files, but gix still reports success.
        Ok(_) if gix::interrupt::is_triggered() => {
            Err(replaced.settle_after_failure(anyhow::anyhow!("Checkout was interrupted")))
        }
        Ok(_) => {
            replaced.discard()?;
            Ok(placed(&index, plan))
        }
        Err(err) => Err(replaced.settle_after_failure(err.into())),
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
        Err(err) if is_absent(&err) || is_symlinked_parent_error(&err) => {
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
        Err(err) if is_absent(&err) => {}
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

/// Whether `err` means nothing is at the path, also because a file replaced a directory leading
/// to it.
fn is_absent(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

fn is_symlinked_parent_error(err: &std::io::Error) -> bool {
    err.get_ref()
        .is_some_and(|source| gix::error::classify(source).is_validation())
}

/// What checkout replaces with files, moved out of the way until its outcome is known.
///
/// gix writes over an existing file in place. Like Git, which writes a new file instead, moving
/// the old one aside leaves other hard links with their content, and replaces read-only files the
/// directory allows to be replaced. Unlike removing it up front, it keeps a file checkout never
/// reaches when a filter fails partway, as Git does by removing each file right before writing it.
///
/// An interim until gix can remove an existing file right before writing it, like Git.
struct Replaced {
    /// Each destination and where its old file is meanwhile.
    asides: Vec<(PathBuf, PathBuf)>,
}

impl Replaced {
    /// Move whatever isn't a directory where `plan` writes, like a file, a symlink or a FIFO, to a
    /// free name next to it. gix would write into a FIFO and wait for a reader forever.
    #[cfg(unix)]
    fn move_aside(
        path_check: &mut gix::status::plumbing::SymlinkCheck,
        plan: &Plan,
    ) -> anyhow::Result<Self> {
        let planned = planned_paths(plan);
        // Where each directory's search for a free name continues, so it never starts over.
        let mut next_names: BTreeMap<PathBuf, usize> = BTreeMap::new();
        let mut replaced = Replaced { asides: Vec::new() };
        for (rela_path, _) in plan.writes() {
            let Ok(path) = path_check.verified_path(rela_path) else {
                continue;
            };
            if !std::fs::symlink_metadata(path).is_ok_and(|md| !md.is_dir()) {
                continue;
            }
            let path = path.to_owned();
            let dir = path.parent().expect("a file is in a directory").to_owned();
            let next_name = next_names.entry(dir).or_default();
            let aside = free_name_next_to(&path, rela_path, &planned, next_name);
            if let Err(err) = std::fs::rename(&path, &aside) {
                let err = anyhow::Error::from(err).context(format!("moving {path:?} aside"));
                return Err(replaced.settle_after_failure(err));
            }
            replaced.asides.push((path, aside));
        }
        Ok(replaced)
    }

    /// Like before, write over existing files in place where hard links and read-only files
    /// don't need a new file.
    #[cfg(not(unix))]
    fn move_aside(
        _path_check: &mut gix::status::plumbing::SymlinkCheck,
        _plan: &Plan,
    ) -> anyhow::Result<Self> {
        Ok(Replaced { asides: Vec::new() })
    }

    /// The checkout succeeded: remove the old files.
    fn discard(self) -> anyhow::Result<()> {
        for (_, aside) in self.asides {
            std::fs::remove_file(&aside).with_context(|| format!("removing {aside:?}"))?;
        }
        Ok(())
    }

    /// The checkout failed with `err`: put back each old file whose destination checkout never
    /// wrote, and remove the others, like Git. Name the old files that stay aside in `err`.
    fn settle_after_failure(self, err: anyhow::Error) -> anyhow::Error {
        let mut left_aside = Vec::new();
        for (path, aside) in self.asides {
            let settled = match std::fs::symlink_metadata(&path) {
                Err(not_written) if not_written.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::rename(&aside, &path)
                }
                _ => std::fs::remove_file(&aside),
            };
            if settled.is_err() {
                left_aside.push(aside);
            }
        }
        match left_aside.is_empty() {
            true => err,
            false => err.context(format!("the old content is still in {left_aside:?}")),
        }
    }
}

/// The paths `plan` writes, and the directories leading to them, in lower case, so they are
/// reserved also where the filesystem ignores case.
#[cfg(unix)]
fn planned_paths(plan: &Plan) -> BTreeSet<BString> {
    let mut paths = BTreeSet::new();
    for (path, _) in plan.writes() {
        let leading = path.iter().enumerate().filter(|(_, byte)| **byte == b'/');
        paths.extend(leading.map(|(at, _)| BString::from(path[..at].to_lowercase())));
        paths.insert(BString::from(path.to_lowercase()));
    }
    paths
}

/// A path next to `path`, which is `rela_path` in the worktree, that nothing occupies and that
/// isn't among the `planned` paths in any case. Its name is short, whatever the length of
/// `path`'s name. The search starts at `next_name`, which then moves past the name it found.
#[cfg(unix)]
fn free_name_next_to(
    path: &Path,
    rela_path: &BStr,
    planned: &BTreeSet<BString>,
    next_name: &mut usize,
) -> PathBuf {
    let rela_dir = match rela_path.rfind_byte(b'/') {
        Some(at) => rela_path[..=at].to_lowercase(),
        None => Vec::new(),
    };
    let name = |n: usize| format!(".gitbutler-replaced-{n}");
    let n = (*next_name..)
        .find(|&n| {
            let mut rela = BString::from(rela_dir.clone());
            rela.push_str(name(n));
            !planned.contains(&rela)
                && std::fs::symlink_metadata(path.with_file_name(name(n))).is_err()
        })
        .expect("one of infinitely many names is free");
    *next_name = n + 1;
    path.with_file_name(name(n))
}
