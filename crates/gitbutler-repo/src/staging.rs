use std::path::PathBuf;

use anyhow::Result;
use git2::{ApplyLocation, ApplyOptions, Repository, ResetType};
use gitbutler_command_context::CommandContext;
use gitbutler_diff::{ChangeType, GitHunk};

pub fn stage_file(ctx: &CommandContext, path: &PathBuf, hunks: &Vec<GitHunk>) -> Result<()> {
    let repo = ctx.repo();
    let mut index = repo.index()?;
    if hunks.iter().any(|h| h.change_type == ChangeType::Untracked) {
        index.add_path(path)?;
        index.write()?;
        return Ok(());
    }

    let mut apply_opts = ApplyOptions::new();
    apply_opts.hunk_callback(|cb_hunk| {
        cb_hunk.map_or(false, |cb_hunk| {
            for hunk in hunks {
                if hunk.new_start == cb_hunk.new_start()
                    && hunk.new_start + hunk.new_lines == cb_hunk.new_start() + cb_hunk.new_lines()
                {
                    return true;
                }
            }
            false
        })
    });

    let diff = diff_workdir_to_index(repo, path)?;
    repo.apply(&diff, ApplyLocation::Index, Some(&mut apply_opts))?;
    Ok(())
}

pub fn stage_files(
    ctx: &CommandContext,
    files_to_stage: &Vec<(PathBuf, Vec<GitHunk>)>,
) -> Result<()> {
    for (path_to_stage, hunks_to_stage) in files_to_stage {
        stage_file(ctx, path_to_stage, hunks_to_stage)?;
    }
    Ok(())
}

pub fn unstage_all(ctx: &CommandContext) -> Result<()> {
    let repo = ctx.repo();
    // Get the HEAD commit (current commit)
    let head_commit = repo.head()?.peel_to_commit()?;
    // Reset the index to match the HEAD commit
    repo.reset(head_commit.as_object(), ResetType::Mixed, None)?;
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
