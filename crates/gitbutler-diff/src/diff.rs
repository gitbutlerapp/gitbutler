use std::{borrow::Cow, collections::HashMap, path::PathBuf, str};

use anyhow::{Context as _, Result};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use but_serde::BStringForFrontend;
use git2::DiffHunk;
use serde::{Deserialize, Serialize};

pub type DiffByPathMap = HashMap<PathBuf, FileDiff>;

/// The type of change
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Entry does not exist in old version
    Added,
    /// Entry is untracked item in workdir
    Untracked,
    /// Entry does not exist in new version
    Deleted,
    /// Entry content changed between old and new
    Modified,
}
impl From<git2::Delta> for ChangeType {
    fn from(v: git2::Delta) -> Self {
        use ChangeType as C;
        use git2::Delta as D;
        match v {
            D::Added => C::Added,
            D::Untracked => C::Untracked,
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
#[serde(rename_all = "camelCase")]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// The `+`, `-` or ` ` prefixed lines of the diff produced by `git2`, along with their line separator.
    #[serde(rename = "diff")]
    pub diff_lines: BStringForFrontend,
    pub binary: bool,
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
            change_type: ChangeType::Untracked,
        }
    }
}

/// Access
impl GitHunk {
    pub(crate) fn contains(&self, line: u32) -> bool {
        self.new_start <= line && self.new_start + self.new_lines >= line
    }
}

/// Comparison
impl GitHunk {
    /// workspace_intersects_unapplied is used to determine if a hunk from a diff between workspace
    /// and the trunk intersects with an unapplied hunk. We want to use the new start/end for the
    /// integration hunk and the old start/end for the unapplied hunk.
    pub fn workspace_intersects_unapplied(
        workspace_hunk: &GitHunk,
        unapplied_hunk: &GitHunk,
    ) -> bool {
        let unapplied_old_end = unapplied_hunk.old_start + unapplied_hunk.old_lines;
        let workspace_new_end = workspace_hunk.new_start + workspace_hunk.new_lines;

        unapplied_hunk.old_start <= workspace_new_end
            && workspace_hunk.new_start <= unapplied_old_end
    }
}

impl PartialEq<DiffHunk<'_>> for &GitHunk {
    fn eq(&self, other: &DiffHunk) -> bool {
        self.new_start == other.new_start()
            && self.new_lines == other.new_lines()
            && self.old_start == other.old_start()
            && self.old_lines == other.old_lines()
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: PathBuf,
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

pub fn trees(
    repo: &git2::Repository,
    old_tree: &git2::Tree,
    new_tree: &git2::Tree,
    include_context: bool,
) -> Result<DiffByPathMap> {
    let mut diff_opts = git2::DiffOptions::new();
    let context_lines = match include_context {
        true => 3,
        false => 0,
    };
    diff_opts
        .show_binary(true)
        .ignore_submodules(true)
        .context_lines(context_lines);

    let diff = repo.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))?;
    hunks_by_filepath(None, &diff)
}

/// Transform `diff` into a mapping of `worktree-relative path -> FileDiff`, where `FileDiff` is
/// all the diff-related information one could ask for. This is mainly to workaround `git2`
/// which doesn't provide a format that is easy to use or hunk-based, but it's line-by-line only.
///
/// `repository` should be `None` if there is no reason to access the workdir, which it will do to
/// keep the binary data in the object database, which otherwise would be lost to the system
/// (it's not reconstructable from the delta, or it's not attempted).
pub fn hunks_by_filepath(
    repo: Option<&git2::Repository>,
    diff: &git2::Diff,
) -> Result<DiffByPathMap> {
    enum LineOrHexHash<'a> {
        Line(Cow<'a, BStr>),
        HexHashOfBinaryBlob(String),
    }
    // find all the hunks
    let mut diff_files = HashMap::new();
    let mut err = None;

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
                        && !delta.new_file().id().is_zero() && full_path.exists() {
                            let oid = repo.blob_path(full_path.as_path()).unwrap();
                            if delta.new_file().id() != oid {
                                err = Some(format!("we only store the file which is already known by the diff system, but it was different: {} != {}", delta.new_file().id(), oid));
                                return false
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
                                path: file_path.to_path_buf(),
                                hunks: Vec::new(),
                                skipped: false,
                                binary: delta.new_file().is_binary(),
                                old_size_bytes: delta.old_file().size(),
                                new_size_bytes: delta.new_file().size(),
                        });
                    if existing.is_some() {
                        err = Some(format!("Encountered an invalid internal state related to the diff: {existing:?}"));
                        return false;
                    }
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
                                        diff_lines: line.into_owned().into(),
                                        binary: false,
                                        change_type,
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
    .with_context(|| format!("failed to print diff: {err:?}"))?;

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

pub fn diff_files_into_hunks(
    files: &DiffByPathMap,
) -> impl Iterator<Item = (PathBuf, Vec<GitHunk>)> + '_ {
    files
        .iter()
        .map(|(path, file)| (path.clone(), file.hunks.clone()))
}
