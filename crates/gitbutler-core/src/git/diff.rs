use std::path::PathBuf;
use std::{collections::HashMap, str};

use anyhow::{Context, Result};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::Repository;
use crate::git;

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

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct GitHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    #[serde(serialize_with = "crate::serde::as_string_lossy")]
    pub diff: BString,
    pub binary: bool,
    pub change_type: ChangeType,
}

impl GitHunk {
    pub fn contains(&self, line: u32) -> bool {
        self.new_start <= line && self.new_start + self.new_lines >= line
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    pub hunks: Option<Vec<GitHunk>>,
    pub skipped: bool,
    pub binary: bool,
    pub old_size_bytes: u64,
    pub new_size_bytes: u64,
}

#[instrument(skip(repository))]
pub fn workdir(
    repository: &Repository,
    commit_oid: &git::Oid,
) -> Result<HashMap<PathBuf, FileDiff>> {
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
    let diff_files = hunks_by_filepath(repository, &diff);
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
) -> Result<HashMap<PathBuf, FileDiff>> {
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

    hunks_by_filepath(repository, &diff)
}

pub fn without_large_files(
    size_limit_bytes: u64,
    diff: &git2::Diff,
    mut diff_opts: git2::DiffOptions,
) -> (git2::DiffOptions, HashMap<PathBuf, FileDiff>) {
    let mut skipped_files: HashMap<PathBuf, FileDiff> = HashMap::new();
    for delta in diff.deltas() {
        if delta.new_file().size() > size_limit_bytes {
            if let Some(path) = delta.new_file().path() {
                skipped_files.insert(
                    path.to_path_buf(),
                    FileDiff {
                        old_path: delta.old_file().path().map(ToOwned::to_owned),
                        new_path: delta.new_file().path().map(ToOwned::to_owned),
                        hunks: None,
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

fn hunks_by_filepath(
    repository: &Repository,
    diff: &git2::Diff,
) -> Result<HashMap<PathBuf, FileDiff>> {
    // find all the hunks
    let mut hunks_by_filepath: HashMap<PathBuf, Vec<GitHunk>> = HashMap::new();
    let mut diff_files: HashMap<PathBuf, FileDiff> = HashMap::new();

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

            hunks_by_filepath
                .entry(file_path.to_path_buf())
                .or_default();

            let new_start = hunk.as_ref().map_or(0, git2::DiffHunk::new_start);
            let new_lines = hunk.as_ref().map_or(0, git2::DiffHunk::new_lines);
            let old_start = hunk.as_ref().map_or(0, git2::DiffHunk::old_start);
            let old_lines = hunk.as_ref().map_or(0, git2::DiffHunk::old_lines);

            let line = match line.origin() {
                '+' | '-' | ' ' => {
                    let mut buf = BString::new(Vec::with_capacity(line.content().len() + 1));
                    buf.push_char(line.origin());
                    buf.push_str(line.content());
                    Some((buf, false))
                }
                'B' => {
                    let full_path = repository.workdir().unwrap().join(file_path);
                    // save the file_path to the odb
                    if !delta.new_file().id().is_zero() && full_path.exists() {
                        // the binary file wasn't deleted
                        repository.blob_path(full_path.as_path()).unwrap();
                    }
                    Some((delta.new_file().id().to_string().into(), true))
                }
                'F' => None,
                _ => {
                    let line: BString = line.content().into();
                    Some((line, false))
                }
            };
            if let Some((line, is_binary)) = line {
                let hunks = hunks_by_filepath
                    .entry(file_path.to_path_buf())
                    .or_default();

                if let Some(previous_hunk) = hunks.last_mut() {
                    let hunk_did_not_change = previous_hunk.old_start == old_start
                        && previous_hunk.old_lines == old_lines
                        && previous_hunk.new_start == new_start
                        && previous_hunk.new_lines == new_lines;

                    if hunk_did_not_change {
                        if is_binary {
                            // binary overrides the diff
                            previous_hunk.binary = true;
                            previous_hunk.old_start = 0;
                            previous_hunk.old_lines = 0;
                            previous_hunk.new_start = 0;
                            previous_hunk.new_lines = 0;
                            previous_hunk.diff = line;
                        } else if !previous_hunk.binary {
                            // append non binary hunks
                            previous_hunk.diff.push_str(&line);
                        }
                    } else {
                        hunks.push(GitHunk {
                            old_start,
                            old_lines,
                            new_start,
                            new_lines,
                            diff: line,
                            binary: is_binary,
                            change_type,
                        });
                    }
                } else {
                    hunks.push(GitHunk {
                        old_start,
                        old_lines,
                        new_start,
                        new_lines,
                        diff: line,
                        binary: is_binary,
                        change_type,
                    });
                }
            }
            diff_files.insert(
                file_path.to_path_buf(),
                FileDiff {
                    old_path: delta.old_file().path().map(ToOwned::to_owned),
                    new_path: delta.new_file().path().map(ToOwned::to_owned),
                    hunks: None,
                    skipped: false,
                    binary: delta.new_file().is_binary(),
                    old_size_bytes: delta.old_file().size(),
                    new_size_bytes: delta.new_file().size(),
                },
            );

            true
        },
    )
    .context("failed to print diff")?;

    let hunks_by_filepath: HashMap<PathBuf, Vec<GitHunk>> = hunks_by_filepath
        .into_iter()
        .map(|(k, v)| {
            if let Some(binary_hunk) = v.iter().find(|hunk| hunk.binary) {
                if v.len() > 1 {
                    // TODO(ST): Would it be possible here to permanently discard lines because
                    //           they are considered binary? After all, here we create a new change,
                    //           turning multiple binary hunks into single line hunk (somehow).
                    //           Probably answer: it's likely that this data is only created on the fly,
                    //           and only the original source data is relevant - validate it.
                    //           But: virtual branches definitely apply hunks.
                    // if there are multiple hunks with binary among them, then the binary hunk
                    // takes precedence
                    (
                        k,
                        vec![GitHunk {
                            old_start: 0,
                            old_lines: 0,
                            new_start: 0,
                            new_lines: 0,
                            diff: binary_hunk.diff.clone(),
                            binary: true,
                            change_type: binary_hunk.change_type,
                        }],
                    )
                } else {
                    (k, v)
                }
            } else if v.is_empty() {
                // this is a new file
                (
                    k,
                    vec![GitHunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 0,
                        new_lines: 0,
                        diff: Default::default(),
                        binary: false,
                        change_type: ChangeType::Modified,
                    }],
                )
            } else {
                (k, v)
            }
        })
        .collect();

    for (file_path, diff_file) in &mut diff_files {
        diff_file.hunks = hunks_by_filepath.get(file_path).cloned();
    }
    Ok(diff_files)
}

// returns None if cannot reverse the patch header
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

// returns None if cannot reverse the hunk
pub fn reverse_hunk(hunk: &GitHunk) -> Option<GitHunk> {
    if hunk.binary {
        None
    } else {
        reverse_patch(hunk.diff.as_ref()).map(|diff| GitHunk {
            old_start: hunk.new_start,
            old_lines: hunk.new_lines,
            new_start: hunk.old_start,
            new_lines: hunk.old_lines,
            diff,
            binary: hunk.binary,
            change_type: hunk.change_type,
        })
    }
}

pub fn diff_files_to_hunks(
    files: &HashMap<PathBuf, FileDiff>,
) -> HashMap<PathBuf, Vec<git::diff::GitHunk>> {
    let mut file_hunks: HashMap<PathBuf, Vec<git::diff::GitHunk>> = HashMap::new();
    for (file_path, diff_file) in files {
        if !diff_file.skipped {
            file_hunks.insert(
                file_path.clone(),
                diff_file.hunks.clone().unwrap_or_default(),
            );
        }
    }
    file_hunks
}
