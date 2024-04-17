use std::{collections::HashMap, path, str};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
    pub diff: String,
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
    pub old_path: Option<path::PathBuf>,
    pub new_path: Option<path::PathBuf>,
    pub hunks: Option<Vec<GitHunk>>,
    pub skipped: bool,
    pub binary: bool,
    pub old_size_bytes: u64,
    pub new_size_bytes: u64,
}

pub fn workdir(
    repository: &Repository,
    commit_oid: &git::Oid,
) -> Result<HashMap<path::PathBuf, FileDiff>> {
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
) -> Result<HashMap<path::PathBuf, FileDiff>> {
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
) -> (git2::DiffOptions, HashMap<path::PathBuf, FileDiff>) {
    let mut skipped_files: HashMap<path::PathBuf, FileDiff> = HashMap::new();
    for delta in diff.deltas() {
        if delta.new_file().size() > size_limit_bytes {
            if let Some(path) = delta.new_file().path() {
                skipped_files.insert(
                    path.to_path_buf(),
                    FileDiff {
                        old_path: delta.old_file().path().map(std::path::Path::to_path_buf),
                        new_path: delta.new_file().path().map(std::path::Path::to_path_buf),
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
                diff_opts.pathspec(path);
            }
        }
    }
    (diff_opts, skipped_files)
}

fn hunks_by_filepath(
    repository: &Repository,
    diff: &git2::Diff,
) -> Result<HashMap<path::PathBuf, FileDiff>> {
    // find all the hunks
    let mut hunks_by_filepath: HashMap<path::PathBuf, Vec<GitHunk>> = HashMap::new();
    let mut diff_files: HashMap<path::PathBuf, FileDiff> = HashMap::new();

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

            if let Some((line, is_binary)) = match line.origin() {
                '+' | '-' | ' ' => {
                    if let Ok(content) = str::from_utf8(line.content()) {
                        Some((format!("{}{}", line.origin(), content), false))
                    } else {
                        let full_path = repository.workdir().unwrap().join(file_path);
                        // save the file_path to the odb
                        if !delta.new_file().id().is_zero() && full_path.exists() {
                            // the binary file wasnt deleted
                            repository.blob_path(full_path.as_path()).unwrap();
                        }
                        Some((delta.new_file().id().to_string(), true))
                    }
                }
                'B' => {
                    let full_path = repository.workdir().unwrap().join(file_path);
                    // save the file_path to the odb
                    if !delta.new_file().id().is_zero() && full_path.exists() {
                        // the binary file wasnt deleted
                        repository.blob_path(full_path.as_path()).unwrap();
                    }
                    Some((delta.new_file().id().to_string(), true))
                }
                'F' => None,
                _ => {
                    if let Ok(content) = str::from_utf8(line.content()) {
                        Some((content.to_string(), false))
                    } else {
                        let full_path = repository.workdir().unwrap().join(file_path);
                        // save the file_path to the odb
                        if !delta.new_file().id().is_zero() && full_path.exists() {
                            // the binary file wasnt deleted
                            repository.blob_path(full_path.as_path()).unwrap();
                        }
                        Some((delta.new_file().id().to_string(), true))
                    }
                }
            } {
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
                    old_path: delta.old_file().path().map(std::path::Path::to_path_buf),
                    new_path: delta.new_file().path().map(std::path::Path::to_path_buf),
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

    let hunks_by_filepath: HashMap<path::PathBuf, Vec<GitHunk>> = hunks_by_filepath
        .into_iter()
        .map(|(k, v)| {
            if let Some(binary_hunk) = v.iter().find(|hunk| hunk.binary) {
                if v.len() > 1 {
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
                        diff: String::new(),
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
fn reverse_patch_header(header: &str) -> Option<String> {
    use itertools::Itertools;

    let mut parts = header.split_whitespace();

    match parts.next() {
        Some("@@") => {}
        _ => return None,
    };

    let old_range = parts.next()?;
    let new_range = parts.next()?;

    match parts.next() {
        Some("@@") => {}
        _ => return None,
    };

    Some(format!(
        "@@ {} {} @@ {}",
        new_range.replace('+', "-"),
        old_range.replace('-', "+"),
        parts.join(" ")
    ))
}

fn reverse_patch(patch: &str) -> Option<String> {
    let mut reversed = String::new();
    for line in patch.lines() {
        if line.starts_with("@@") {
            if let Some(header) = reverse_patch_header(line) {
                reversed.push_str(&header);
                reversed.push('\n');
            } else {
                return None;
            }
        } else if line.starts_with('+') {
            reversed.push_str(&line.replacen('+', "-", 1));
            reversed.push('\n');
        } else if line.starts_with('-') {
            reversed.push_str(&line.replacen('-', "+", 1));
            reversed.push('\n');
        } else {
            reversed.push_str(line);
            reversed.push('\n');
        }
    }
    Some(reversed)
}

// returns None if cannot reverse the hunk
pub fn reverse_hunk(hunk: &GitHunk) -> Option<GitHunk> {
    if hunk.binary {
        None
    } else {
        reverse_patch(&hunk.diff).map(|diff| GitHunk {
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
    files: &HashMap<path::PathBuf, FileDiff>,
) -> HashMap<path::PathBuf, Vec<git::diff::GitHunk>> {
    let mut file_hunks: HashMap<path::PathBuf, Vec<git::diff::GitHunk>> = HashMap::new();
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
