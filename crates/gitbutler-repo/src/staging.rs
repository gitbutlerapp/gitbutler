use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use git2::{ApplyLocation, ApplyOptions, Repository};
use gitbutler_command_context::CommandContext;
use gitbutler_diff::{ChangeType, FileDiff, GitHunk, Hunk};

fn stage_tracked_changes(
    ctx: &CommandContext,
    changes: &Vec<&(PathBuf, Vec<GitHunk>)>,
) -> Result<()> {
    let repo = ctx.repo();
    for (path, hunks) in changes {
        let mut apply_opts = ApplyOptions::new();
        apply_opts.hunk_callback(|cb_hunk| {
            cb_hunk.map_or(false, |cb_hunk| {
                for hunk in hunks {
                    if hunk == cb_hunk {
                        return true;
                    }
                }
                false
            })
        });

        let diff = diff_workdir_to_index(repo, path)?;
        repo.apply(&diff, ApplyLocation::Index, Some(&mut apply_opts))?;
    }

    Ok(())
}

fn stage_untracked_files(ctx: &CommandContext, paths: &Vec<&PathBuf>) -> Result<()> {
    let repo = ctx.repo();
    let mut index = repo.index()?;
    for path in paths {
        index.add_path(path)?;
    }
    index.write()?;
    Ok(())
}

pub fn stage(ctx: &CommandContext, changes: &[(PathBuf, Vec<GitHunk>)]) -> Result<()> {
    let (untracked_changes, tracked_changes): (Vec<_>, Vec<_>) = changes
        .iter()
        .partition(|(_path, hunks)| hunks.iter().any(|h| h.change_type == ChangeType::Untracked));
    let untracked_files = untracked_changes.iter().map(|c| &c.0).collect();
    stage_tracked_changes(ctx, &tracked_changes)?;
    stage_untracked_files(ctx, &untracked_files)?;
    Ok(())
}

fn diff_workdir_to_index<'a>(repo: &'a Repository, path: &PathBuf) -> Result<git2::Diff<'a>> {
    let index = repo.index()?;
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .recurse_untracked_dirs(true)
        .include_untracked(true)
        .show_binary(true)
        .show_untracked_content(true)
        .ignore_submodules(true)
        .context_lines(3)
        .pathspec(path);
    Ok(repo.diff_index_to_workdir(Some(&index), Some(&mut diff_opts))?)
}

pub fn filter_diff_by_hunk_ids(
    diffs: HashMap<PathBuf, FileDiff>,
    hunk_ids: Vec<(&PathBuf, &Vec<Hunk>)>,
) -> Result<Vec<(PathBuf, Vec<GitHunk>)>> {
    hunk_ids
        .into_iter()
        .map(|(path, hunk_ids)| {
            if let Some(diff) = diffs.get(path) {
                Ok((
                    path.clone(),
                    hunk_ids
                        .iter()
                        .filter_map(|claimed_hunk| {
                            diff.hunks
                                .iter()
                                .find(|diff_hunk| claimed_hunk == *diff_hunk)
                                .cloned()
                        })
                        .collect(),
                ))
            } else {
                Err(anyhow!("Hunk not found in workspace diff"))
            }
        })
        .collect::<Result<Vec<(_, Vec<_>)>>>()
}

pub fn reset_index(repo: &Repository, tree_id: git2::Oid) -> Result<()> {
    let mut index = repo.index()?;
    let tree = repo.find_tree(tree_id)?;
    index.read_tree(&tree)?;
    Ok(index.write()?)
}
