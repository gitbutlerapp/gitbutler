use std::collections::HashMap;

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
use gitbutler_stack::StackId;
use itertools::Itertools;
use md5::Digest;

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
        // The commit id's must be in the order they get applied (parents first).
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
                            change_type: hunk.change_type,
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

pub struct StackDependencies {
    pub commit_dependencies: HashMap<git2::Oid, Vec<git2::Oid>>,
    pub reverse_commit_dependencies: HashMap<git2::Oid, Vec<git2::Oid>>,
    pub dependent_diffs: HashMap<git2::Oid, Vec<Digest>>,
}

pub struct CommitDependencies {
    pub dependencies: Vec<git2::Oid>,
    pub reverse_dependencies: Vec<git2::Oid>,
    pub dependent_diffs: Vec<String>,
}

/// Returns the dependencies of a commit from the stack dependencies.
pub fn commit_dependencies_from_stack(
    stack_dependencies: &StackDependencies,
    commit_id: git2::Oid,
) -> CommitDependencies {
    let dependencies = stack_dependencies
        .commit_dependencies
        .get(&commit_id)
        .cloned()
        .unwrap_or_default();

    let reverse_dependencies = stack_dependencies
        .reverse_commit_dependencies
        .get(&commit_id)
        .cloned()
        .unwrap_or_default();

    let dependent_diffs = stack_dependencies
        .dependent_diffs
        .get(&commit_id)
        .map(|v| {
            v.iter()
                .map(|hunk_hash| format!("{:x}", hunk_hash).to_string())
                .collect()
        })
        .unwrap_or_default();

    CommitDependencies {
        dependencies,
        reverse_dependencies,
        dependent_diffs,
    }
}

/// Returns the dependencies of a stack from the workspace dependencies.
pub fn stack_dependencies_from_workspace(
    workspace_dependencies: &HunkDependencyResult,
    stack_id: StackId,
) -> StackDependencies {
    let commit_dependencies = workspace_dependencies
        .commit_dependencies
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .iter()
        .map(|(commit_id, dependencies)| (*commit_id, dependencies.iter().cloned().collect()))
        .collect();

    let reverse_commit_dependencies = workspace_dependencies
        .inverse_commit_dependencies
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .iter()
        .map(|(commit_id, dependencies)| (*commit_id, dependencies.iter().cloned().collect()))
        .collect();

    let dependent_diffs = workspace_dependencies
        .commit_dependent_diffs
        .get(&stack_id)
        .unwrap_or(&Default::default())
        .iter()
        .map(|(commit_id, hunk_hashes)| (*commit_id, hunk_hashes.iter().cloned().collect()))
        .collect();

    StackDependencies {
        commit_dependencies,
        reverse_commit_dependencies,
        dependent_diffs,
    }
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
                .map(|hunk_hash| format!("{:x}", hunk_hash).to_string())
                .collect()
        })
        .unwrap_or_default();

    CommitDependencies {
        dependencies,
        reverse_dependencies,
        dependent_diffs,
    }
}
