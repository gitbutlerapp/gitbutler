use anyhow::Context;
use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_hunk_dependency::locks::HunkDependencyResult;
use gitbutler_hunk_dependency::{
    calculate_hunk_dependencies, HunkDependencyOptions, InputCommit, InputDiff, InputFile,
    InputStack,
};
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use gitbutler_stack::Stack;
use itertools::Itertools;

use crate::file::list_virtual_commit_files;
use crate::BranchStatus;

pub fn compute_workspace_dependencies(
    ctx: &CommandContext,
    target_sha: &git2::Oid,
    base_diffs: &BranchStatus,
    stacks: &Vec<Stack>,
) -> Result<HunkDependencyResult> {
    let repo = ctx.repository();

    let mut stacks_input: Vec<InputStack> = vec![];
    for stack in stacks {
        let mut commits_input: Vec<InputCommit> = vec![];
        // Commit IDs in application order
        let commit_ids = repo
            .l(stack.head(), LogUntil::Commit(*target_sha), false)
            .context("failed to list commits")?
            .into_iter()
            .rev()
            .collect_vec();
        for commit_id in commit_ids {
            let mut files_input: Vec<InputFile> = vec![];
            let commit = repo.find_commit(commit_id)?;
            let files = list_virtual_commit_files(ctx, &commit, false)?;
            for file in files {
                let value = InputFile {
                    path: file.path,
                    diffs: file
                        .hunks
                        .iter()
                        .map(|hunk| InputDiff {
                            old_start: hunk.old_start,
                            old_lines: hunk.old_lines,
                            new_start: hunk.start,
                            new_lines: hunk.end - hunk.start,
                        })
                        .collect::<Vec<_>>(),
                };
                files_input.push(value);
            }
            commits_input.push(InputCommit {
                commit_id,
                files: files_input,
            });
        }
        stacks_input.push(InputStack {
            stack_id: stack.id,
            commits: commits_input,
        });
    }

    calculate_hunk_dependencies(HunkDependencyOptions {
        workdir: base_diffs,
        stacks: stacks_input,
    })
}
