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
            if let Some(binary_hunk) = v.iter().find(|hunk| hunk.binary) {
                if v.len() > 1 {
                    // if there are multiple hunks with binary among them, then the binary hunk
                    // takes precedence
                    (
                        k,
                        vec![Hunk {
                            old_start: 0,
                            old_lines: 0,
                            new_start: 0,
                            new_lines: 0,
                            diff: binary_hunk.diff.clone(),
                            binary: true,
                        }],
                    )
                } else {
                    (k, v)
                }
            } else if v.is_empty() {
                // this is a new file
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

    #[test]
    fn diff_some_lines_are_binary() {
        let repository = test_utils::test_repository();
        std::fs::write(
            repository.workdir().unwrap().join("file"),
            [
                // butler/test/fixtures/git/1/8e/18ec9df5-65c5-4828-97ba-d91ec4903a74/objects/1f/9d7d5dd0d3d3ced66cee36bf1dd42bd33d0aa8
                120, 1, 101, 144, 79, 75, 195, 64, 16, 197, 61, 239, 167, 120, 160, 224, 165, 77, 3,
                5, 17, 111, 42, 42, 245, 162, 135, 22, 60, 118, 155, 76, 179, 75, 55, 59, 97, 103,
                182, 177, 223, 222, 77, 244, 38, 204, 97, 254, 188, 247, 155, 97, 14, 129, 15, 88,
                223, 213, 87, 215, 120, 243, 250, 148, 53, 80, 194, 110, 131, 103, 142, 13, 13, 42,
                198, 60, 10, 54, 183, 61, 34, 163, 99, 110, 97, 21, 175, 190, 235, 237, 98, 238,
                102, 241, 177, 195, 214, 250, 48, 250, 216, 66, 25, 71, 223, 229, 68, 224, 172, 24,
                93, 17, 111, 48, 218, 168, 80, 71, 5, 187, 218, 125, 77, 154, 192, 124, 66, 240,
                39, 170, 176, 117, 94, 80, 98, 154, 147, 21, 79, 82, 124, 246, 50, 169, 90, 134,
                215, 9, 36, 190, 45, 192, 35, 62, 131, 189, 116, 137, 115, 108, 23, 56, 20, 190,
                78, 94, 103, 5, 103, 74, 226, 57, 162, 225, 168, 137, 67, 101, 204, 123, 46, 156,
                148, 227, 172, 121, 48, 102, 191, 223, 155, 27, 196, 225, 27, 250, 119, 107, 35,
                130, 165, 71, 181, 242, 113, 200, 90, 205, 37, 151, 82, 199, 223, 124, 57, 90, 109,
                92, 49, 13, 23, 117, 28, 215, 88, 246, 112, 170, 67, 37, 148, 202, 62, 220, 215,
                117, 61, 99, 205, 71, 90, 64, 184, 167, 114, 78, 249, 5, 5, 161, 202, 188, 156, 41,
                162, 79, 76, 255, 38, 63, 226, 30, 123, 106,
            ],
        )
        .unwrap();

        let head_commit_id = repository.head().unwrap().peel_to_commit().unwrap().id();

        let diff = workdir(&repository, &head_commit_id, &Options::default()).unwrap();
        assert_eq!(
            diff[&path::PathBuf::from("file")],
            vec![Hunk {
                old_start: 0,
                old_lines: 0,
                new_start: 0,
                new_lines: 0,
                diff: "3fc41b9ae6836a94f41c78b4ce69d78b6e7080f1".to_string(),
                binary: true,
            }]
        );
    }
}
