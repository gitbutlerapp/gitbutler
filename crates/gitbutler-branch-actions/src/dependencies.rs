use anyhow::{Context, Result};
use but_oxidize::{ObjectIdExt, RepoExt};
use gitbutler_command_context::CommandContext;
use gitbutler_hunk_dependency::{
    HunkDependencyOptions, InputCommit, InputDiff, InputFile, InputStack,
    calculate_hunk_dependencies, locks::HunkDependencyResult,
};
use gitbutler_repo::logging::{LogUntil, RepositoryExt as _};
use gitbutler_stack::{Stack, StackId};

use crate::{BranchStatus, file::list_virtual_commit_files};

pub fn compute_workspace_dependencies(
    ctx: &CommandContext,
    target_sha: &git2::Oid,
    base_diffs: &BranchStatus,
    stacks: &Vec<Stack>,
) -> Result<HunkDependencyResult> {
    let repo = ctx.repo();
    let gix_repo = repo.to_gix()?;

    let mut stacks_input: Vec<InputStack> = vec![];
    for stack in stacks {
        let mut commits_input: Vec<InputCommit> = vec![];
        let commit_ids = get_commits_to_process(repo, &gix_repo, stack, target_sha)?;
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
                            change_type: hunk.change_type,
                            old_start: hunk.old_start,
                            old_lines: hunk.old_lines,
                            new_start: hunk.new_start,
                            new_lines: hunk.new_lines,
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

/// Get the commits that need to be processed for the stack dependencies caculation.
///
/// Commit IDs are in the order that they are applied (parent first).
/// Merge commits to the target branch are not included.
fn get_commits_to_process<'a>(
    repo: &'a git2::Repository,
    gix_repo: &'a gix::Repository,
    stack: &'a Stack,
    target_sha: &'a git2::Oid,
) -> Result<impl Iterator<Item = git2::Oid> + 'a, anyhow::Error> {
    let commit_ids = repo
        .l(
            stack.head_oid(gix_repo)?.to_git2(),
            LogUntil::Commit(*target_sha),
            false,
        )
        .context("failed to list commits")?
        .into_iter()
        .rev()
        .filter_map(move |commit_id| {
            let commit = repo.find_commit(commit_id).ok()?;
            if commit.parent_count() == 1 {
                return Some(commit_id);
            }

            let has_integrated_parent = commit.parent_ids().any(|id| {
                repo.graph_ahead_behind(id, *target_sha)
                    .is_ok_and(|(number_commits_ahead, _)| number_commits_ahead == 0)
            });

            (!has_integrated_parent).then_some(commit_id)
        });
    Ok(commit_ids)
}

pub struct CommitDependencies {
    pub dependencies: Vec<git2::Oid>,
    pub reverse_dependencies: Vec<git2::Oid>,
    pub dependent_diffs: Vec<String>,
}

/// Returns the dependencies of a commit from the workspace dependencies.
pub fn commit_dependencies_from_workspace(
    workspace_dependencies: &HunkDependencyResult,
    stack_id: StackId,
    commit_id: git2::Oid,
) -> CommitDependencies {
    let dependencies = workspace_dependencies
        .commit_dependencies
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .get(&commit_id)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let reverse_dependencies = workspace_dependencies
        .inverse_commit_dependencies
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .get(&commit_id)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let dependent_diffs = workspace_dependencies
        .commit_dependent_diffs
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .get(&commit_id)
        .map(|v| {
            v.iter()
                .map(|hunk_hash| format!("{hunk_hash:x}").to_string())
                .collect()
        })
        .unwrap_or_default();

    CommitDependencies {
        dependencies,
        reverse_dependencies,
        dependent_diffs,
    }
}
