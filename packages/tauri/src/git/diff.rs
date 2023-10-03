use std::{collections::HashMap, path, str};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::git;

use super::Repository;

#[derive(Debug, PartialEq, Clone, Serialize)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
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

    let diff = repository.diff_tree_to_tree(Some(old_tree), Some(new_tree), Some(&mut diff_opts))?;

    hunks_by_filepath(repository, &diff)
}

fn hunks_by_filepath(
    repository: &Repository,
    diff: &git2::Diff,
) -> Result<HashMap<path::PathBuf, Vec<Hunk>>> {
    // find all the hunks
    let mut hunks_by_filepath: HashMap<path::PathBuf, Vec<Hunk>> = HashMap::new();
    let mut current_diff = String::new();

    let mut current_file_path: Option<path::PathBuf> = None;
    let mut current_hunk_id: Option<String> = None;
    let mut current_new_start: Option<usize> = None;
    let mut current_new_lines: Option<usize> = None;
    let mut current_old_start: Option<usize> = None;
    let mut current_old_lines: Option<usize> = None;
    let mut current_binary = false;

    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        let file_path = delta.new_file().path().unwrap_or_else(|| {
            delta
                .old_file()
                .path()
                .expect("failed to get file name from diff")
        });

        let (hunk_id, new_start, new_lines, old_start, old_lines) = if let Some(hunk) = hunk {
            (
                format!(
                    "{}-{} {}-{}",
                    hunk.new_start(),
                    hunk.new_lines(),
                    hunk.old_start(),
                    hunk.old_lines(),
                ),
                hunk.new_start(),
                hunk.new_lines(),
                hunk.old_start(),
                hunk.old_lines(),
            )
        } else if line.origin() == 'B' {
            let hunk_id = format!("{:?}:{}", file_path.as_os_str(), delta.new_file().id());
            (hunk_id.clone(), 0, 0, 0, 0)
        } else {
            return true;
        };

        let is_path_changed = current_file_path
            .as_ref()
            .map(|p| !file_path.eq(p))
            .unwrap_or(false);
        let is_hunk_changed = current_hunk_id
            .as_ref()
            .map(|h| !hunk_id.eq(h))
            .unwrap_or(false);

        if is_hunk_changed || is_path_changed {
            let file_path = current_file_path.as_ref().unwrap().to_path_buf();
            hunks_by_filepath.entry(file_path).or_default().push(Hunk {
                old_start: current_old_start.unwrap(),
                old_lines: current_old_lines.unwrap(),
                new_start: current_new_start.unwrap(),
                new_lines: current_new_lines.unwrap(),
                diff: current_diff.clone(),
                binary: current_binary,
            });
            current_diff = String::new();
        }

        match line.origin() {
            '+' | '-' | ' ' => {
                current_diff.push_str(&format!("{}", line.origin()));
                current_diff.push_str(
                    str::from_utf8(line.content())
                        .map_err(|error| tracing::error!(?error, ?file_path))
                        .unwrap_or_default(),
                );
                current_binary = false;
            }
            'B' => {
                let full_path = repository.workdir().unwrap().join(file_path);
                // save the file_path to the odb
                if !delta.new_file().id().is_zero() && full_path.exists() {
                    // the binary file wasnt deleted
                    repository.blob_path(full_path.as_path()).unwrap();
                }
                current_diff.push_str(&format!("{}", delta.new_file().id()));
                current_binary = true;
            }
            _ => {
                current_diff.push_str(
                    str::from_utf8(line.content())
                        .map_err(|error| tracing::error!(?error, ?file_path))
                        .unwrap_or_default(),
                );
            }
        }

        current_file_path = Some(file_path.to_path_buf());
        current_hunk_id = Some(hunk_id);
        current_new_start = Some(new_start as usize);
        current_new_lines = Some(new_lines as usize);
        current_old_start = Some(old_start as usize);
        current_old_lines = Some(old_lines as usize);

        true
    })
    .context("failed to print diff")?;

    // push the last hunk
    if let Some(file_path) = current_file_path {
        hunks_by_filepath.entry(file_path).or_default().push(Hunk {
            old_start: current_old_start.unwrap(),
            old_lines: current_old_lines.unwrap(),
            new_start: current_new_start.unwrap(),
            new_lines: current_new_lines.unwrap(),
            diff: current_diff,
            binary: current_binary,
        });
    }

    Ok(hunks_by_filepath)
}
