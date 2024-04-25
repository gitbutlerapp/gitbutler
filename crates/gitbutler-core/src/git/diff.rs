use std::borrow::Cow;
use std::path::PathBuf;
use std::{collections::HashMap, str};

use anyhow::{Context, Result};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::Repository;
use crate::git;
use crate::virtual_branches::BranchStatus;

pub type DiffByPathMap = HashMap<PathBuf, FileDiff>;

/// The type of change
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Entry does not exist in old version
    Added,
    /// Entry does not exist in new version
    Deleted,
    /// Entry content changed between old and new
    Modified,
}
impl From<git2::Delta> for ChangeType {
    fn from(v: git2::Delta) -> Self {
        use git2::Delta as D;
        use ChangeType as C;
        match v {
            D::Untracked | D::Added => C::Added,
            D::Modified
            | D::Unmodified
            | D::Renamed
            | D::Copied
            | D::Typechange
            | D::Conflicted => C::Modified,
            D::Ignored | D::Unreadable | D::Deleted => C::Deleted,
        }
    }
}

/// A description of a hunk, as identified by its line number and the amount of lines it spans
/// before and after the change.
#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// The `+`, `-` or ` ` prefixed lines of the diff produced by `git2`, along with their line separator.
    #[serde(rename = "diff", serialize_with = "crate::serde::as_string_lossy")]
    pub diff_lines: BString,
    pub binary: bool,
    pub locked_to: Box<[HunkLock]>,
    pub change_type: ChangeType,
}

/// Lifecycle
impl GitHunk {
    /// A special hunk that signals a binary file whose complete content is a blob under `hex_id` in Git.
    /// `changetype` is tells us what happened with the file.
    fn binary_marker(hex_id: String, change_type: ChangeType) -> Self {
        GitHunk {
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            diff_lines: hex_id.into(),
            binary: true,
            change_type,
            locked_to: Box::new([]),
        }
    }

    /// Return a hunk that represents a new file by convention.
    fn generic_new_file() -> Self {
        Self {
            old_start: 0,
            old_lines: 0,
            new_start: 0,
            new_lines: 0,
            diff_lines: Default::default(),
            binary: false,
            change_type: ChangeType::Modified,
            locked_to: Box::new([]),
        }
    }
}

/// Access
impl GitHunk {
    pub fn contains(&self, line: u32) -> bool {
        self.new_start <= line && self.new_start + self.new_lines >= line
    }

    pub fn with_locks(mut self, locks: &[HunkLock]) -> Self {
        self.locked_to = locks.to_owned().into();
        self
    }
}

// A hunk is locked when it depends on changes in commits that are in your
// workspace. A hunk can be locked to more than one branch if it overlaps
// with more than one committed hunk.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    pub branch_id: uuid::Uuid,
    pub commit_id: git::Oid,
}

#[derive(Debug, PartialEq, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    /// Hunks might be empty if nothing about the files content is known, which happens
    /// if the content is skipped due to it being a large file.
    pub hunks: Vec<GitHunk>,
    pub skipped: bool,
    /// This is `true` if this is a file with undiffable content. Then, `hunks` might be a single
    /// hunk that is the hash of the binary blob in Git.
    pub binary: bool,
    pub old_size_bytes: u64,
    pub new_size_bytes: u64,
}

#[instrument(skip(repository))]
pub fn workdir(repository: &Repository, commit_oid: &git::Oid) -> Result<DiffByPathMap> {
    let commit = repository
        .find_commit(*commit_oid)
        .context("failed to find commit")?;
    let tree = commit.tree().context("failed to find tree")?;

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_binary(true)
        .show_untracked_content(true)
        .ignore_submodules(true)
        .context_lines(3);

    let mut diff = repository.diff_tree_to_workdir(Some(&tree), Some(&mut diff_opts))?;
    let (mut diff_opts, skipped_files) = without_large_files(50_000_000, &diff, diff_opts);
    if !skipped_files.is_empty() {
        diff = repository.diff_tree_to_workdir(Some(&tree), Some(&mut diff_opts))?;
    }
    let diff_files = hunks_by_filepath(Some(repository), &diff);
    diff_files.map(|mut df| {
        for (key, value) in skipped_files {
            df.insert(key, value);
        }
        df
    })
}

pub fn trees(
    repository: &Repository,
    old_tree: &git::Tree,
    new_tree: &git::Tree,
) -> Result<DiffByPathMap> {
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_binary(true)
        .ignore_submodules(true)
        .context_lines(3)
        .show_untracked_content(true);

    let diff =
        repository.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))?;

    hunks_by_filepath(None, &diff)
}

pub fn without_large_files(
    size_limit_bytes: u64,
    diff: &git2::Diff,
    mut diff_opts: git2::DiffOptions,
) -> (git2::DiffOptions, DiffByPathMap) {
    let mut skipped_files = HashMap::new();
    for delta in diff.deltas() {
        if delta.new_file().size() > size_limit_bytes {
            if let Some(path) = delta.new_file().path() {
                skipped_files.insert(
                    path.to_path_buf(),
                    FileDiff {
                        old_path: delta.old_file().path().map(ToOwned::to_owned),
                        new_path: delta.new_file().path().map(ToOwned::to_owned),
                        hunks: Vec::new(),
                        skipped: true,
                        binary: true,
                        old_size_bytes: delta.old_file().size(),
                        new_size_bytes: delta.new_file().size(),
                    },
                );
            }
        } else if let Some(path) = delta.new_file().path() {
            if let Some(path) = path.to_str() {
                // TODO(ST): use negative pathspecs instead, but with `gitoxide` this might not even be necessary.
                //           Currently, performance could be bad if there are thousands of pathspecs.
                diff_opts.pathspec(path);
            }
        }
    }
    (diff_opts, skipped_files)
}

/// Transform `diff` into a mapping of `worktree-relative path -> FileDiff`, where `FileDiff` is
/// all the diff-related information one could ask for. This is mainly to workaround `git2`
/// which doesn't provide a format that is easy to use or hunk-based, but it's line-by-line only.
///
/// `repository` should be `None` if there is no reason to access the workdir, which it will do to
/// keep the binary data in the object database, which otherwise would be lost to the system
/// (it's not reconstructable from the delta, or it's not attempted).
fn hunks_by_filepath(repo: Option<&Repository>, diff: &git2::Diff) -> Result<DiffByPathMap> {
    enum LineOrHexHash<'a> {
        Line(Cow<'a, BStr>),
        HexHashOfBinaryBlob(String),
    }
    // find all the hunks
    let mut diff_files = HashMap::new();

    diff.print(
        git2::DiffFormat::Patch,
        |delta, hunk, line: git2::DiffLine<'_>| {
            let change_type: ChangeType = delta.status().into();
            let file_path = delta.new_file().path().unwrap_or_else(|| {
                delta
                    .old_file()
                    .path()
                    .expect("failed to get file name from diff")
            });

            let new_start = hunk.as_ref().map_or(0, git2::DiffHunk::new_start);
            let new_lines = hunk.as_ref().map_or(0, git2::DiffHunk::new_lines);
            let old_start = hunk.as_ref().map_or(0, git2::DiffHunk::old_start);
            let old_lines = hunk.as_ref().map_or(0, git2::DiffHunk::old_lines);

            use git2::DiffLineType as D;
            let line = match line.origin_value() {
                D::Addition | D::Deletion | D::Context => {
                    let mut buf = BString::new(Vec::with_capacity(line.content().len() + 1));
                    buf.push_char(line.origin());
                    buf.push_str(line.content());
                    Some(LineOrHexHash::Line(buf.into()))
                }
                D::Binary => {
                    if let Some((full_path, repo)) = repo
                        .and_then(|repo| repo.workdir())
                        .map(|workdir| workdir.join(file_path))
                        .zip(repo)
                    {
                        if !delta.new_file().id().is_zero() && full_path.exists() {
                            let oid = repo.blob_path(full_path.as_path()).unwrap();
                            assert_eq!(
                                delta.new_file().id(),
                                oid.into(),
                                "BUG: we only store the file which is already known by the diff system, but it was different"
                            )
                        }
                    }
                    Some(LineOrHexHash::HexHashOfBinaryBlob(delta.new_file().id().to_string()))
                }
                D::FileHeader => None,
                D::HunkHeader | D::ContextEOFNL | D::AddEOFNL | D::DeleteEOFNL => {
                    Some(LineOrHexHash::Line(line.content().as_bstr().into()))
                }
            };

            match line {
                None => {
                    let existing = diff_files
                        .insert(file_path.to_path_buf(),
                            FileDiff {
                                old_path: delta.old_file().path().map(ToOwned::to_owned),
                                new_path: delta.new_file().path().map(ToOwned::to_owned),
                                hunks: Vec::new(),
                                skipped: false,
                                binary: delta.new_file().is_binary(),
                                old_size_bytes: delta.old_file().size(),
                                new_size_bytes: delta.new_file().size(),
                        });
                    assert_eq!(existing, None, "BUG: this only happens for file-headers, they are provided once");
                }
                Some(line) => {
                    let hunks = &mut diff_files.get_mut(file_path).expect("File header inserts the hunk-list").hunks;
                    let same_hunk = hunks.last_mut().filter(|previous_hunk| {
                        previous_hunk.old_start == old_start
                            && previous_hunk.old_lines == old_lines
                            && previous_hunk.new_start == new_start
                            && previous_hunk.new_lines == new_lines
                    });
                    match same_hunk {
                        Some(hunk) => match line {
                            LineOrHexHash::Line(line) => {
                                hunk.diff_lines.push_str(line.as_ref());
                            }
                            LineOrHexHash::HexHashOfBinaryBlob(id) => {
                                let marker =  GitHunk::binary_marker(id, hunk.change_type) ;
                                *hunk = marker;
                            }
                        },
                        None => {
                            let new_hunk = match line {
                                LineOrHexHash::Line(line) => {
                                    GitHunk {
                                        old_start,
                                        old_lines,
                                        new_start,
                                        new_lines,
                                        diff_lines: line.into_owned(),
                                        binary: false,
                                        change_type,
                                        locked_to: Box::new([]),
                                    }
                                }
                                LineOrHexHash::HexHashOfBinaryBlob(id) => {
                                    GitHunk::binary_marker(id, change_type)
                                }
                            };
                            hunks.push(new_hunk);
                        }
                    }
                }
            }
            true
        },
    )
    .context("failed to print diff")?;

    for file in diff_files.values_mut() {
        if let Some(binary_hunk) = file
            .hunks
            .iter()
            .find_map(|hunk| hunk.binary.then(|| hunk.clone()))
        {
            if file.hunks.len() > 1 {
                // TODO(ST): needs tests, this code isn't executed yet.
                // if there are multiple hunks with binary among them, we replace it with a single marker.
                file.hunks = vec![binary_hunk];
            }
        } else if file.hunks.is_empty() {
            file.hunks = vec![GitHunk::generic_new_file()];
        }
    }

    Ok(diff_files)
}

// returns None if it cannot reverse the patch header
fn reverse_patch_header(header: &BStr) -> Option<BString> {
    let mut parts = header.split(|b| b.is_ascii_whitespace());

    match parts.next() {
        Some(b"@@") => {}
        _ => return None,
    };

    let old_range = parts.next()?;
    let new_range = parts.next()?;

    if parts.next() != Some(b"@@") {
        return None;
    };

    let mut buf: BString = "@@ ".into();
    buf.extend_from_slice(&new_range.replace(b"+", b"-"));
    buf.push(b' ');
    buf.extend_from_slice(&old_range.replace(b"-", b"+"));
    buf.push_str(b" @@ ");

    let mut at_least_one_part = false;
    for part in parts {
        buf.extend_from_slice(part);
        buf.push(b' ');
        at_least_one_part = true;
    }
    if at_least_one_part {
        buf.pop();
    }
    Some(buf)
}

fn reverse_patch(patch: &BStr) -> Option<BString> {
    let mut reversed = BString::default();
    for line in patch.lines() {
        if line.starts_with(b"@@") {
            if let Some(header) = reverse_patch_header(line.as_ref()) {
                reversed.push_str(&header);
                reversed.push(b'\n');
            } else {
                return None;
            }
        } else if line.starts_with(b"+") {
            reversed.push_str(&line.replacen(b"+", b"-", 1));
            reversed.push(b'\n');
        } else if line.starts_with(b"-") {
            reversed.push_str(&line.replacen(b"-", b"+", 1));
            reversed.push(b'\n');
        } else {
            reversed.push_str(line);
            reversed.push(b'\n');
        }
    }
    Some(reversed)
}

// returns `None` if the reversal failed
pub fn reverse_hunk(hunk: &GitHunk) -> Option<GitHunk> {
    if hunk.binary {
        None
    } else {
        reverse_patch(hunk.diff_lines.as_ref()).map(|diff| GitHunk {
            old_start: hunk.new_start,
            old_lines: hunk.new_lines,
            new_start: hunk.old_start,
            new_lines: hunk.old_lines,
            diff_lines: diff,
            binary: hunk.binary,
            change_type: hunk.change_type,
            locked_to: Box::new([]),
        })
    }
}

// TODO(ST): turning this into an iterator will trigger a cascade of changes that
//           mean less unnecessary copies. It also leads to `virtual.rs` - 4k SLOC!
pub fn diff_files_into_hunks(files: DiffByPathMap) -> BranchStatus {
    HashMap::from_iter(files.into_iter().map(|(path, file)| (path, file.hunks)))
}
