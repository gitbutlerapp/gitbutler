use std::{collections::HashMap, path, str};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::git;

use super::Repository;

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub diff: String,
    pub binary: bool,
}

pub struct Options {
    pub context_lines: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self { context_lines: 3 }
    }
}

pub fn workdir(
    repository: &Repository,
    commit_oid: &git::Oid,
    opts: &Options,
) -> Result<HashMap<path::PathBuf, Vec<Hunk>>> {
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
        .context_lines(opts.context_lines);

    let diff = repository.diff_tree_to_workdir(Some(&tree), Some(&mut diff_opts))?;

    hunks_by_filepath(repository, &diff)
}

pub fn trees(
    repository: &Repository,
    old_tree: &git::Tree,
    new_tree: &git::Tree,
) -> Result<HashMap<path::PathBuf, Vec<Hunk>>> {
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_binary(true)
        .show_untracked_content(true);

    let diff =
        repository.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))?;

    hunks_by_filepath(repository, &diff)
}

fn hunks_by_filepath(
    repository: &Repository,
    diff: &git2::Diff,
) -> Result<HashMap<path::PathBuf, Vec<Hunk>>> {
    // find all the hunks
    let mut hunks_by_filepath: HashMap<path::PathBuf, Vec<Hunk>> = HashMap::new();

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
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
            '+' | '-' | ' ' => Some((
                format!(
                    "{}{}",
                    line.origin(),
                    str::from_utf8(line.content())
                        .map_err(|error| tracing::error!(?error, ?file_path))
                        .unwrap_or_default()
                ),
                false,
            )),
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
            _ => Some((
                str::from_utf8(line.content())
                    .map_err(|error| tracing::error!(?error, ?file_path))
                    .unwrap_or_default()
                    .to_string(),
                false,
            )),
        } {
            let hunks = hunks_by_filepath
                .entry(file_path.to_path_buf())
                .or_default();

            if let Some(hunk) = hunks.last_mut() {
                if hunk.old_start == old_start
                    && hunk.old_lines == old_lines
                    && hunk.new_start == new_start
                    && hunk.new_lines == new_lines
                {
                    hunk.diff.push_str(&line);
                    hunk.binary |= is_binary;
                } else {
                    hunks.push(Hunk {
                        old_start,
                        old_lines,
                        new_start,
                        new_lines,
                        diff: line,
                        binary: is_binary,
                    });
                }
            } else {
                hunks.push(Hunk {
                    old_start,
                    old_lines,
                    new_start,
                    new_lines,
                    diff: line,
                    binary: is_binary,
                });
            }
        }

        true
    })
    .context("failed to print diff")?;

    Ok(hunks_by_filepath
        .into_iter()
        .map(|(k, v)| {
            if v.is_empty() {
                (
                    k,
                    vec![Hunk {
                        old_start: 0,
                        old_lines: 0,
                        new_start: 0,
                        new_lines: 0,
                        diff: String::new(),
                        binary: false,
                    }],
                )
            } else {
                (k, v)
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::test_utils;

    use super::*;

    #[test]
    fn diff_simple_text() {
        let repository = test_utils::test_repository();
        std::fs::write(repository.workdir().unwrap().join("file"), "hello").unwrap();

        let head_commit_id = repository.head().unwrap().peel_to_commit().unwrap().id();

        let diff = workdir(&repository, &head_commit_id, &Options::default()).unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(
            diff[&path::PathBuf::from("file")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
                diff: "@@ -0,0 +1 @@\n+hello\n\\ No newline at end of file\n".to_string(),
                binary: false,
            }]
        );
    }

    #[test]
    fn diff_empty_file() {
        let repository = test_utils::test_repository();
        std::fs::write(repository.workdir().unwrap().join("first"), "").unwrap();

        let head_commit_id = repository.head().unwrap().peel_to_commit().unwrap().id();

        let diff = workdir(&repository, &head_commit_id, &Options::default()).unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(
            diff[&path::PathBuf::from("first")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 0,
                diff: String::new(),
                binary: false,
            }]
        );
    }

    #[test]
    fn diff_multiple_empty_files() {
        let repository = test_utils::test_repository();
        std::fs::write(repository.workdir().unwrap().join("first"), "").unwrap();
        std::fs::write(repository.workdir().unwrap().join("second"), "").unwrap();

        let head_commit_id = repository.head().unwrap().peel_to_commit().unwrap().id();

        let diff = workdir(&repository, &head_commit_id, &Options::default()).unwrap();
        assert_eq!(diff.len(), 2);
        assert_eq!(
            diff[&path::PathBuf::from("first")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 0,
                diff: String::new(),
                binary: false,
            }]
        );
        assert_eq!(
            diff[&path::PathBuf::from("second")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 0,
                diff: String::new(),
                binary: false,
            }]
        );
    }

    #[test]
    fn diff_binary() {
        let repository = test_utils::test_repository();
        std::fs::write(
            repository.workdir().unwrap().join("image"),
            [
                255, 0, 0, // Red pixel
                0, 0, 255, // Blue pixel
                255, 255, 0, // Yellow pixel
                0, 255, 0, // Green pixel
            ],
        )
        .unwrap();

        let head_commit_id = repository.head().unwrap().peel_to_commit().unwrap().id();

        let diff = workdir(&repository, &head_commit_id, &Options::default()).unwrap();
        assert_eq!(
            diff[&path::PathBuf::from("image")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 0,
                diff: "71ae6e216f38164b6633e25d35abb043c3785af6".to_string(),
                binary: true,
            }]
        );
    }
}
