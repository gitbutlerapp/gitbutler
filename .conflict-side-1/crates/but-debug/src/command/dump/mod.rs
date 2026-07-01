//! Implementation of the `dump` debug command.
//!
//! ## QUALITY NOTICE
//! NOTE: mostly vibed, what's properly reviewed is the tests. Also tested by hand of course.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use gix::{bstr::ByteSlice as _, index};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    args::{Args, DumpArgs, DumpSubcommands, RepoDumpArgs},
    setup,
};

mod diagnostics;
mod progress;
use progress::{DumpProgress, ProgressReader, ProgressWriter};

/// Execute the `dump` subcommand.
pub(crate) fn run(
    args: &Args,
    dump_args: &DumpArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    match &dump_args.cmd {
        DumpSubcommands::Repo(repo_args) => run_repo(args, repo_args, out, err),
        DumpSubcommands::Diagnostics(diagnostics_args) => {
            diagnostics::run(args, diagnostics_args, out, err)
        }
    }
}

/// Execute the `dump repo` subcommand.
fn run_repo(
    args: &Args,
    repo_args: &RepoDumpArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let current_dir = effective_current_dir(args)?;
    let repo = setup::repo_from_args(args).with_context(|| {
        format!(
            "Could not discover Git repository at '{}'",
            current_dir.display()
        )
    })?;
    // nothing really works unless we have a non-relative path, keep it simple.
    let repo = gix::open_opts(repo.git_dir().canonicalize()?, repo.open_options().clone())?;

    let output_path = match &repo_args.archive.output {
        Some(path) => current_dir.join(path),
        None => default_output_path(&repo, "dump")?,
    };
    let diagnostics = if repo_args.no_diagnostics {
        None
    } else {
        Some(diagnostics::capture_from_current_dir(
            &current_dir,
            diagnostics::dot_timeout(repo_args.diagnostics.dot_timeout_seconds),
            out,
            err,
        )?)
    };
    let progress = DumpProgress::new()?;
    let layout = ArchiveLayout::new(&repo)?;
    let output_path = OutputPath::new(output_path, current_dir.clone());
    let lock = acquire_archive_lock(&output_path.path, repo.workdir().map(Path::to_owned))?;
    let output_path = output_path.with_lock_path(lock.lock_path().to_owned());
    std::thread::scope(|scope| -> Result<()> {
        let counter = scope.spawn({
            let progress = &progress;
            let repo = repo.clone().into_sync();
            let output_path = output_path.clone();
            let diagnostics_files = diagnostics
                .as_ref()
                .map_or(0, diagnostics::Diagnostics::file_count);
            move || {
                count_archive_input(
                    repo,
                    &output_path,
                    repo_args.git_only,
                    diagnostics_files,
                    progress,
                )
            }
        });

        let file = ProgressWriter::new(lock, &progress);
        let mut archive = ArchiveWriter::new(file, &progress);
        if let Some(diagnostics) = &diagnostics {
            archive.add_diagnostics(diagnostics, &layout.worktree_root)?;
        }
        archive.add_repo(&repo, &layout, &output_path, repo_args.git_only)?;
        let file = archive.finish(err)?;
        let lock = file.into_inner();

        let _ = counter
            .join()
            .map_err(|_| anyhow::anyhow!("Archive progress counter thread panicked"))?;
        persist_archive(lock)?;
        Ok(())
    })?;

    writeln!(out, "Archive at: {}", output_path.path.display())?;
    open_archive_dir_unless_requested(
        &output_path.path,
        repo_args.archive.no_open_archive_directory,
    )?;
    Ok(())
}

/// Return the effective current directory selected by `-C`.
fn effective_current_dir(args: &Args) -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(&args.current_dir))
}

/// Return the default archive path for `repo`, using `suffix` after the repository name.
fn default_output_path(repo: &gix::Repository, suffix: &str) -> Result<PathBuf> {
    let base = archive_base_name(repo)?;
    let root = repo.workdir().unwrap_or_else(|| repo.git_dir());
    let parent = root.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join(format!("{base}-{suffix}.zip")))
}

/// Acquire a lock file for writing an archive atomically.
///
/// When `boundary_directory` is provided, missing directories below that
/// boundary may be created for the archive path and removed again if the lock is
/// rolled back. When it is `None`, the archive's parent directory must already
/// exist.
fn acquire_archive_lock(
    archive_path: &Path,
    boundary_directory: Option<PathBuf>,
) -> Result<gix::lock::File> {
    gix::lock::File::acquire_to_update_resource(
        archive_path,
        gix::lock::acquire::Fail::Immediately,
        boundary_directory,
    )
    .with_context(|| format!("Could not lock archive '{}'", archive_path.display()))
}

/// Atomically move a fully-written archive lock file into its final path.
fn persist_archive(mut lock: gix::lock::File) -> Result<PathBuf> {
    let archive_path = lock.resource_path();
    io::Write::flush(&mut lock)
        .with_context(|| format!("Could not flush archive '{}'", archive_path.display()))?;
    lock.commit()
        .map(|(path, _file)| path)
        .map_err(|err| err.error)
        .with_context(|| format!("Could not persist archive '{}'", archive_path.display()))
}

/// Open the parent directory of `archive_path` unless disabled by the caller.
fn open_archive_dir_unless_requested(archive_path: &Path, disabled: bool) -> Result<()> {
    if !disabled {
        let dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
        open::that(dir)
            .with_context(|| format!("Could not open archive directory '{}'", dir.display()))?;
    }
    Ok(())
}

/// Keeps worktree files and Git files under the right top-level paths in the zip.
///
/// Computes and stores the zip-internal root names for repository content.
///
/// These are owned `String` values because they are derived at runtime from the
/// repository directory name and are reused while writing the archive. They are
/// not `PathBuf`s because they are zip entry prefixes, not filesystem paths:
/// zip entries always use `/` separators regardless of the host platform.
#[derive(Clone)]
struct ArchiveLayout {
    /// Root directory for worktree files in the archive.
    worktree_root: String,
    /// Root directory for Git repository files in the archive.
    git_root: String,
}

impl ArchiveLayout {
    /// Build archive root names for `repo`, accounting for bare and non-bare layouts.
    fn new(repo: &gix::Repository) -> Result<Self> {
        let base = archive_base_name(repo)?;
        let worktree_root = if repo.workdir().is_some() {
            format!("{base}-dump")
        } else {
            format!("{base}-dump.git")
        };
        let git_root = if repo.workdir().is_some() {
            format!("{worktree_root}/.git")
        } else {
            worktree_root.clone()
        };
        Ok(Self {
            worktree_root,
            git_root,
        })
    }
}

/// Return the sanitized archive base name for `repo`.
///
/// For example, a worktree at `/tmp/my repo` becomes `my-repo`, while a bare
/// repository at `/tmp/project.git` becomes `project`.
fn archive_base_name(repo: &gix::Repository) -> Result<String> {
    let root = repo.workdir().unwrap_or_else(|| repo.git_dir());
    let raw = root
        .file_name()
        .and_then(|name| name.to_str())
        .context("Repository path does not have a usable final component")?;
    let raw = if repo.workdir().is_none() {
        raw.strip_suffix(".git").unwrap_or(raw)
    } else {
        raw
    };
    Ok(root_name_slug(raw))
}

/// Convert `name` into a safe zip root component.
///
/// For example, `my repo/main` becomes `my-repo-main`, `你好` stays `你好`,
/// and an all-dot name like `...` falls back to `repository`.
fn root_name_slug(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('.');
    if sanitized.is_empty() || sanitized == ".." {
        "repository".to_owned()
    } else {
        sanitized.to_owned()
    }
}

struct ArchiveWriter<'progress, W: io::Write + io::Seek> {
    zip: ZipWriter<W>,
    progress: &'progress DumpProgress,
    written: BTreeSet<String>,
    skipped_unarchivable_paths: BTreeSet<PathBuf>,
}

impl<'progress, W: io::Write + io::Seek> ArchiveWriter<'progress, W> {
    /// Create an archive writer around `writer`.
    fn new(writer: W, progress: &'progress DumpProgress) -> Self {
        Self {
            zip: ZipWriter::new(writer),
            progress,
            written: BTreeSet::new(),
            skipped_unarchivable_paths: BTreeSet::new(),
        }
    }

    /// Add all selected repository entries to `self`.
    ///
    /// `repo` supplies the worktree, index, ignore rules, and Git directory
    /// locations to archive. `layout` defines the zip-internal roots used for
    /// worktree and Git state entries. `output_path` identifies the archive
    /// currently being written so it can be skipped if it appears during
    /// traversal. `git_only` omits worktree files when true and archives only
    /// Git repository state.
    fn add_repo(
        &mut self,
        repo: &gix::Repository,
        layout: &ArchiveLayout,
        output_path: &OutputPath,
        git_only: bool,
    ) -> Result<()> {
        let mut skipped_unarchivable_paths = BTreeSet::new();
        let result = visit_archive_entries(
            repo,
            layout,
            output_path,
            git_only,
            self.progress,
            |entry| self.add_entry(entry),
            |path| {
                skipped_unarchivable_paths.insert(path);
            },
        );
        self.skipped_unarchivable_paths
            .extend(skipped_unarchivable_paths);
        result
    }

    /// Add generated diagnostics files directly below `archive_root`.
    fn add_diagnostics(
        &mut self,
        diagnostics: &diagnostics::Diagnostics,
        archive_root: &str,
    ) -> Result<()> {
        for (relative_path, contents) in diagnostics.entries() {
            self.add_generated_file(format!("{archive_root}/{relative_path}"), contents)?;
        }
        Ok(())
    }

    fn add_generated_file(&mut self, entry_name: String, contents: &[u8]) -> Result<()> {
        self.progress.check_abort()?;
        if !self.written.insert(entry_name.clone()) {
            return Ok(());
        }
        self.zip.start_file(entry_name, generated_file_options())?;
        io::Write::write_all(&mut self.zip, contents)?;
        self.progress.add_file_processed();
        Ok(())
    }

    /// Add `entry` to `self`.
    ///
    /// The `written` set makes duplicate archive names a no-op. This matters
    /// for linked worktrees, where common Git state and per-worktree Git state
    /// are overlaid into one `.git` directory. Symlinks are stored as symlinks
    /// with their link target, directories are added as directory entries, and
    /// regular files are streamed into the zip with permissions derived from
    /// `meta`. Other filesystem node types are ignored.
    fn add_entry(&mut self, entry: ArchiveEntry) -> Result<()> {
        self.progress.check_abort()?;
        if !self.written.insert(entry.entry_name.clone()) {
            return Ok(());
        }

        if entry.meta.file_type().is_symlink() {
            let target = fs::read_link(&entry.path)?;
            self.zip
                .add_symlink_from_path(&entry.entry_name, target, symlink_options())?;
            self.progress.add_file_processed();
        } else if entry.meta.is_dir() {
            self.zip
                .add_directory(&entry.entry_name, directory_options())?;
        } else if entry.meta.is_file() {
            let file = fs::File::open(&entry.path)?;
            let mut file = ProgressReader::new(file, self.progress);
            self.zip
                .start_file(&entry.entry_name, file_options(&entry))?;
            io::copy(&mut file, &mut self.zip)?;
            self.progress.add_file_processed();
        }
        Ok(())
    }

    /// Finish writing `self` and return the wrapped writer.
    fn finish(self, err: &mut dyn io::Write) -> Result<W> {
        self.report_skipped_unarchivable_paths(err)?;
        Ok(self.zip.finish()?)
    }

    /// Report paths skipped because they could not be represented safely as zip names.
    fn report_skipped_unarchivable_paths(&self, err: &mut dyn io::Write) -> Result<()> {
        if self.skipped_unarchivable_paths.is_empty() {
            return Ok(());
        }

        writeln!(
            err,
            "Skipped paths that cannot be represented safely in the archive:"
        )?;
        for path in &self.skipped_unarchivable_paths {
            writeln!(err, "  {path:?}")?;
        }
        Ok(())
    }
}

/// Filesystem entry selected for inclusion in the archive.
struct ArchiveEntry {
    /// Source filesystem path to read from.
    path: PathBuf,
    /// Zip entry name to write, using `/` separators and rooted in the archive layout.
    entry_name: String,
    /// Metadata captured while traversing, reused while writing to preserve type and mode.
    meta: fs::Metadata,
}

impl ArchiveEntry {
    /// Return the maximum number of source bytes expected to be read for `self`.
    ///
    /// Regular files are streamed into the zip, so their file length contributes
    /// to the progress upper bound. Directories and symlinks do not read file
    /// contents from `path`, so they contribute zero bytes.
    fn bytes_read_upper_bound(&self) -> usize {
        if self.meta.is_file() {
            usize::try_from(self.meta.len()).unwrap_or(usize::MAX)
        } else {
            0
        }
    }

    /// Return `true` if `self` advances the processed-file progress counter.
    ///
    /// Regular files and symlinks are counted because they produce concrete
    /// archive payload entries. Directories are written too, but are structural
    /// entries and are intentionally left out of the file count.
    fn is_archivable(&self) -> bool {
        self.meta.is_file() || self.meta.file_type().is_symlink()
    }

    /// Return the zip compression method to use for `self`.
    fn compression_method(&self) -> CompressionMethod {
        if is_git_object_entry(&self.entry_name) {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Bzip2
        }
    }
}

/// Progress upper bounds collected before archive writing completes.
#[derive(Default)]
struct ArchiveTotals {
    /// Total regular-file bytes expected to be read.
    bytes_read: usize,
    /// Total regular files and symlinks expected to be processed.
    files_processed: usize,
}

impl ArchiveTotals {
    fn add(&mut self, entry: &ArchiveEntry) {
        self.bytes_read = self
            .bytes_read
            .saturating_add(entry.bytes_read_upper_bound());
        if entry.is_archivable() {
            self.files_processed = self.files_processed.saturating_add(1);
        }
    }
}

struct SourceEntry {
    path: PathBuf,
    meta: fs::Metadata,
}

#[derive(Clone, Copy)]
struct GitEntryFilter {
    skip_link_layout_files: bool,
    skip_worktree_registry: bool,
}

/// Count archive input for progress upper bounds without writing entries.
/// The latter is written once everything is countered.
///
/// `repo` is the thread-safe repository clone used by the scoped counter
/// thread. `output_path` is skipped for the same reason as during archive
/// writing. `git_only` mirrors the user option so the counter and writer count
/// the same entry set. `progress` receives the computed byte and file-count
/// upper bounds.
fn count_archive_input(
    repo: gix::ThreadSafeRepository,
    output_path: &OutputPath,
    git_only: bool,
    diagnostics_files: usize,
    progress: &DumpProgress,
) -> Result<()> {
    let repo: gix::Repository = repo.into();
    let layout = ArchiveLayout::new(&repo)?;
    let mut totals = ArchiveTotals::default();
    visit_archive_entries(
        &repo,
        &layout,
        output_path,
        git_only,
        progress,
        |entry| {
            totals.add(&entry);
            Ok(())
        },
        |_path| {},
    )?;
    totals.files_processed = totals.files_processed.saturating_add(diagnostics_files);
    progress.set_input_upper_bounds(totals.bytes_read, totals.files_processed);
    Ok(())
}

/// Visit every repository entry that should be represented in the archive.
///
/// The traversal first visits non-ignored worktree contents, when requested and
/// available, then always visits Git repository state. Linked worktree Git
/// directories are normalized by the Git-state visitor before entries reach
/// `visit`.
///
/// `repo` supplies the repository layout and ignore/index state. `layout`
/// provides the archive roots for worktree and Git entries. `output_path` is
/// skipped if encountered so the dump does not include the archive currently
/// being written. `git_only` disables the worktree pass when true. `progress`
/// is checked for interruption during traversal. For each archivable entry,
/// `visit` receives an [`ArchiveEntry`] to inform about what's traversed.
/// Paths that cannot be represented as a safe zip entry name are passed
/// to `skip_unarchivable` instead.
fn visit_archive_entries(
    repo: &gix::Repository,
    layout: &ArchiveLayout,
    output_path: &OutputPath,
    git_only: bool,
    progress: &DumpProgress,
    mut visit: impl FnMut(ArchiveEntry) -> Result<()>,
    mut skip_unarchivable: impl FnMut(PathBuf),
) -> Result<()> {
    if !git_only && let Some(workdir) = repo.workdir() {
        visit_worktree_entries(
            repo,
            workdir,
            &layout.worktree_root,
            output_path,
            progress,
            &mut visit,
            &mut skip_unarchivable,
        )?;
    }
    visit_git_entries(
        repo,
        &layout.git_root,
        output_path,
        progress,
        &mut visit,
        &mut skip_unarchivable,
    )
}

/// Visit the non-ignored worktree contents below `workdir`.
///
/// Entries are rooted under `archive_root`, use `repo` for index and ignore
/// state, and skip `output_path` to avoid archiving the file being written.
/// The worktree `.git` entry is intentionally omitted here; Git repository
/// state is written separately by [`visit_git_entries()`].
fn visit_worktree_entries(
    repo: &gix::Repository,
    workdir: &Path,
    archive_root: &str,
    output_path: &OutputPath,
    progress: &DumpProgress,
    visit: &mut impl FnMut(ArchiveEntry) -> Result<()>,
    skip_unarchivable: &mut impl FnMut(PathBuf),
) -> Result<()> {
    let index = repo.index_or_empty()?;
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;

    let mut stack = sorted_children(workdir)?;
    while let Some(path) = stack.pop() {
        progress.check_abort()?;
        if output_path.is_same(&path) {
            continue;
        }
        let relative = path.strip_prefix(workdir)?;
        if is_dot_git(relative) {
            continue;
        }

        let meta = path.symlink_metadata()?;
        let is_dir = meta.is_dir();
        let is_excluded = excludes
            .at_path(relative, is_dir.then_some(index::entry::Mode::DIR))
            .is_ok_and(|platform| platform.is_excluded());
        if is_excluded && !is_tracked(relative, is_dir, &index) {
            continue;
        }

        let Some(entry_name) = entry_name(archive_root, relative) else {
            skip_unarchivable(relative.to_owned());
            continue;
        };
        visit(ArchiveEntry {
            path: path.clone(),
            entry_name,
            meta,
        })?;
        if is_dir {
            stack.extend(sorted_children(&path)?);
        }
    }
    Ok(())
}

/// Visit Git repository state from `repo`.
///
/// Entries are rooted under `archive_root` and skip `output_path`.
///
/// A normal repository has the same `git_dir()` and `common_dir()`, so its
/// `.git` directory can be copied as-is. A linked worktree has a small
/// per-worktree Git directory whose `commondir` and `gitdir` files point
/// back to the main repository. For that case, this first collects the
/// common Git directory and then overlays the linked worktree's Git
/// directory by relative path. While doing that, it omits the common
/// `worktrees/` registry and the linked worktree's `commondir`/`gitdir`
/// link files, producing a self-contained `.git` directory that can be
/// unpacked and used as a regular repository.
fn visit_git_entries(
    repo: &gix::Repository,
    archive_root: &str,
    output_path: &OutputPath,
    progress: &DumpProgress,
    visit: &mut impl FnMut(ArchiveEntry) -> Result<()>,
    skip_unarchivable: &mut impl FnMut(PathBuf),
) -> Result<()> {
    let mut entries = BTreeMap::new();
    let is_linked_worktree = repo.git_dir() != repo.common_dir();
    collect_git_entries(
        repo.common_dir(),
        repo.common_dir(),
        &mut entries,
        output_path,
        progress,
        GitEntryFilter {
            skip_link_layout_files: false,
            skip_worktree_registry: is_linked_worktree,
        },
    )?;
    if is_linked_worktree {
        collect_git_entries(
            repo.git_dir(),
            repo.git_dir(),
            &mut entries,
            output_path,
            progress,
            GitEntryFilter {
                skip_link_layout_files: true,
                skip_worktree_registry: false,
            },
        )?;
    }

    for (relative, source) in entries {
        progress.check_abort()?;
        let Some(entry_name) = entry_name(archive_root, &relative) else {
            skip_unarchivable(relative);
            continue;
        };
        visit(ArchiveEntry {
            path: source.path,
            entry_name,
            meta: source.meta,
        })?;
    }
    Ok(())
}

/// Collect Git directory entries below `dir` into `entries`.
///
/// Paths are stored relative to `root`, skip `output_path`, and apply `filter`
/// to omit linked-worktree bookkeeping that should not be archived.
fn collect_git_entries(
    root: &Path,
    dir: &Path,
    entries: &mut BTreeMap<PathBuf, SourceEntry>,
    output_path: &OutputPath,
    progress: &DumpProgress,
    filter: GitEntryFilter,
) -> Result<()> {
    let mut children = sorted_children(dir)?;
    while let Some(path) = children.pop() {
        progress.check_abort()?;
        if output_path.is_same(&path) {
            continue;
        }
        let relative = path.strip_prefix(root)?.to_owned();
        if filter.skip_link_layout_files && is_link_layout_file(&relative) {
            continue;
        }
        if filter.skip_worktree_registry && is_worktree_registry(&relative) {
            continue;
        }
        let meta = path.symlink_metadata()?;
        if meta.is_dir() {
            children.extend(sorted_children(&path)?);
        }
        entries.insert(relative, SourceEntry { path, meta });
    }
    Ok(())
}

/// Return children of `dir` in deterministic pop order.
fn sorted_children(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut children = fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    children.sort();
    children.reverse();
    Ok(children)
}

/// Return true if `relative` starts with a worktree `.git` component.
fn is_dot_git(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .is_some_and(|component| matches!(component, Component::Normal(name) if name == ".git"))
}

/// Return true if `relative` is a linked-worktree layout file.
fn is_link_layout_file(relative: &Path) -> bool {
    matches!(
        relative.components().next(),
        Some(Component::Normal(name)) if name == "commondir" || name == "gitdir"
    )
}

/// Return true if `relative` points into the common Git worktree registry.
fn is_worktree_registry(relative: &Path) -> bool {
    matches!(
        relative.components().next(),
        Some(Component::Normal(name)) if name == "worktrees"
    )
}

/// Return true if `relative` is tracked in `index`.
///
/// Directories use `is_dir` to switch to prefix matching so ignored directories
/// containing tracked files are still archived.
fn is_tracked(relative: &Path, is_dir: bool, index: &gix::index::State) -> bool {
    let relative = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(relative));
    if !is_dir {
        return index.entry_by_path(relative.as_ref()).is_some();
    }

    let mut prefix = relative.into_owned();
    if !prefix.ends_with(b"/") {
        prefix.push(b'/');
    }
    index
        .entries()
        .iter()
        .any(|entry| entry.path(index).as_bytes().starts_with(prefix.as_slice()))
}

/// Build a safe zip entry name from archive `root` and filesystem `relative` path.
///
/// Returns `None` when `relative` cannot be represented as Unicode without
/// losing information, or when the resulting zip entry would be unsafe.
fn entry_name(root: &str, relative: &Path) -> Option<String> {
    let mut name = String::with_capacity(root.len() + relative.as_os_str().len() + 2);
    name.push_str(root);
    if !relative.as_os_str().is_empty() {
        let relative = relative.to_str()?;
        name.push('/');
        #[cfg(windows)]
        {
            name.push_str(&relative.replace('\\', "/"));
        }
        #[cfg(not(windows))]
        {
            name.push_str(relative);
        }
    }
    if name.contains('\0') || name.split('/').any(|component| component == "..") {
        return None;
    }
    Some(name)
}

/// Stores the output archive path and its normalized form.
#[derive(Clone)]
struct OutputPath {
    /// The path requested by the caller.
    path: PathBuf,
    /// The current directory used to resolve relative output paths.
    current_dir: PathBuf,
    /// Lock file path used while writing the archive.
    lock_path: Option<PathBuf>,
    /// Realpath form of `path`, even when only parent directories exist.
    realpath: Option<PathBuf>,
    /// Realpath form of the lock file path used while writing.
    lock_realpath: Option<PathBuf>,
}

impl OutputPath {
    /// Create an output path handle for `path`.
    fn new(path: PathBuf, current_dir: PathBuf) -> Self {
        let realpath = realpath(&path, &current_dir);
        Self {
            path,
            current_dir,
            lock_path: None,
            realpath,
            lock_realpath: None,
        }
    }

    /// Track the in-progress lock file so repository traversal can skip it too.
    ///
    /// For example, while writing `repo-dump.zip`, the temporary lock file may
    /// be `repo-dump.zip.lock`.
    fn with_lock_path(mut self, lock_path: PathBuf) -> Self {
        self.lock_realpath = realpath(&lock_path, &self.current_dir);
        self.lock_path = Some(lock_path);
        self
    }

    /// Return true if `path` identifies the output archive path.
    fn is_same(&self, path: &Path) -> bool {
        path == self.path
            || self.lock_path.as_deref() == Some(path)
            || realpath(path, &self.current_dir).is_some_and(|path| {
                self.realpath.as_deref() == Some(path.as_path())
                    || self.lock_realpath.as_deref() == Some(path.as_path())
            })
    }
}

fn realpath(path: &Path, current_dir: &Path) -> Option<PathBuf> {
    gix::path::realpath_opts(path, current_dir, gix::path::realpath::MAX_SYMLINKS).ok()
}

/// Return zip file options for a regular archive entry.
fn file_options(entry: &ArchiveEntry) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(entry.compression_method())
        .unix_permissions(file_permissions(&entry.meta))
}

/// Return true if `entry_name` points into a dumped Git object database.
fn is_git_object_entry(entry_name: &str) -> bool {
    let mut components = entry_name.split('/');
    if components.next().is_some_and(|root| root.ends_with(".git")) {
        return components.next() == Some("objects");
    }

    while let Some(component) = components.next() {
        if component == ".git" {
            return components.next() == Some("objects");
        }
    }
    false
}

/// Return zip directory options.
fn directory_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o755)
}

/// Return zip file options for generated diagnostics entries.
fn generated_file_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o644)
}

/// Return zip symlink options.
fn symlink_options() -> SimpleFileOptions {
    SimpleFileOptions::default().unix_permissions(0o777)
}

#[cfg(unix)]
/// Return normalized Unix file permissions for `meta`.
fn file_permissions(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;

    if meta.permissions().mode() & 0o111 != 0 {
        0o755
    } else {
        0o644
    }
}

#[cfg(not(unix))]
/// Return portable non-Unix file permissions; `_meta` is ignored.
fn file_permissions(_meta: &fs::Metadata) -> u32 {
    0o644
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn entry_name_skips_non_unicode_relative_paths() -> anyhow::Result<()> {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let relative = PathBuf::from(OsString::from_vec(b"non-unicode-\xff.txt".to_vec()));

        assert!(entry_name("root", &relative).is_none());
        Ok(())
    }

    #[test]
    fn entry_name_skips_unsafe_relative_paths() -> anyhow::Result<()> {
        assert!(entry_name("root", Path::new("../escape")).is_none());
        assert!(entry_name("root", Path::new("null-\0-byte")).is_none());
        Ok(())
    }

    #[test]
    fn git_object_entries_are_detected_for_stored_compression() {
        assert!(is_git_object_entry(
            "project-dump/.git/objects/pack/pack-abc.pack"
        ));
        assert!(is_git_object_entry("project-dump/.git/objects/12/34567890"));
        assert!(is_git_object_entry(
            "project-dump.git/objects/pack/pack-abc.idx"
        ));

        assert!(!is_git_object_entry("project-dump/.git/config"));
        assert!(!is_git_object_entry("project-dump/objects/file"));
    }

    #[test]
    fn archive_lock_rolls_back_until_persisted_and_prevents_double_writes() -> anyhow::Result<()> {
        use std::io::Write as _;

        let dir = tempfile::tempdir()?;
        let archive_path = dir.path().join("out.zip");
        fs::write(&archive_path, b"previous")?;

        let mut lock = acquire_archive_lock(&archive_path, None)?;
        lock.write_all(b"partial")?;
        assert!(
            acquire_archive_lock(&archive_path, None).is_err(),
            "archive lock should prevent concurrent writers"
        );
        drop(lock);
        assert_eq!(
            fs::read(&archive_path)?,
            b"previous",
            "dropping the lock should keep the previous archive"
        );

        let nested_archive_path = dir.path().join("new").join("nested").join("out.zip");
        assert!(
            acquire_archive_lock(&nested_archive_path, None).is_err(),
            "without a boundary, the archive parent directory should have to exist"
        );

        let mut lock = acquire_archive_lock(&nested_archive_path, Some(dir.path().to_owned()))?;
        lock.write_all(b"partial")?;
        drop(lock);
        assert!(
            !nested_archive_path
                .parent()
                .expect("nested archive has a parent")
                .exists(),
            "dropping the lock should remove empty parent directories created for it"
        );

        let mut lock = acquire_archive_lock(&archive_path, None)?;
        lock.write_all(b"complete")?;
        persist_archive(lock)?;
        assert_eq!(
            fs::read(&archive_path)?,
            b"complete",
            "persisting the lock should replace the archive"
        );
        Ok(())
    }
}
