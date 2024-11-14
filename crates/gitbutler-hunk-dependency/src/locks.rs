use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gitbutler_diff::{GitHunk, Hunk, HunkHash};
use gitbutler_stack::StackId;
use itertools::Itertools;
use serde::Serialize;

use crate::{InputStack, WorkspaceRanges};

// A hunk is locked when it depends on changes in commits that are in your workspace. A hunk can
// be locked to more than one branch if it overlaps with more than one committed hunk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    // TODO: Rename this stack_id.
    pub branch_id: StackId,
    #[serde(with = "gitbutler_serde::oid")]
    pub commit_id: git2::Oid,
}

pub struct HunkDependencyOptions<'a> {
    // Uncommitted changes in workspace.
    pub workdir: &'a HashMap<PathBuf, Vec<GitHunk>>,

    /// A nested map of committed diffs per path, commit id, and stack id.
    pub stacks: Vec<InputStack>,
}

#[derive(Debug, Clone)]
pub struct HunkDependencyResult {
    /// A map from diffs to branch and commit dependencies.
    pub diffs: HashMap<HunkHash, Vec<HunkLock>>,
    /// A map from stack id to commit dependencies.
    /// Commit dependencies map commit id to commits it depends on.
    pub commit_dependencies: HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>>,
    /// A map from stack id to inverse commit dependencies.
    /// Inverse commit dependencies map commit id to commits that depend on it.
    pub inverse_commit_dependencies: HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>>,
    /// A map from stack id to dependent commit dependent diffs.
    /// Commit dependent diffs map commit id to diffs that depend on it.
    pub commit_dependent_diffs: HashMap<StackId, HashMap<git2::Oid, HashSet<HunkHash>>>,
}

/// Returns a map from hunk hash to hunk locks.
///
/// To understand if any uncommitted changes depend on (intersect) any existing changes we first
/// transform the branch specific line numbers to the workspace, then look for places they
/// intersect.
///
/// TODO: Change terminology to talk about dependencies instead of locks.
pub fn calculate_hunk_dependencies(
    options: HunkDependencyOptions,
) -> anyhow::Result<HunkDependencyResult> {
    let HunkDependencyOptions { workdir, stacks } = options;

    // Transforms stack specific line numbers to workspace line numbers.
    let ranges = WorkspaceRanges::create(stacks)?;

    let diffs: HashMap<_, _> = workdir
        .iter()
        .flat_map(|(path, workspace_hunks)| {
            workspace_hunks.iter().filter_map(|hunk| {
                ranges
                    .intersection(path, hunk.old_start, hunk.old_lines)
                    .map(|intersection| {
                        intersection
                            .iter()
                            .map(|dependency| HunkLock {
                                commit_id: dependency.commit_id,
                                branch_id: dependency.stack_id,
                            })
                            .collect_vec()
                    })
                    .map(|locks| (Hunk::hash_diff(&hunk.diff_lines), locks))
            })
        })
        .collect();

    let commit_dependent_diffs = diffs.iter().fold(
        HashMap::new(),
        |mut acc: HashMap<StackId, HashMap<git2::Oid, HashSet<HunkHash>>>, (hash, locks)| {
            for lock in locks {
                acc.entry(lock.branch_id)
                    .or_default()
                    .entry(lock.commit_id)
                    .or_default()
                    .insert(*hash);
            }
            acc
        },
    );

    let commit_dependencies = ranges.commit_dependencies;
    let inverse_commit_dependencies = ranges.inverse_commit_dependencies;

    Ok(HunkDependencyResult {
        diffs,
        commit_dependencies,
        inverse_commit_dependencies,
        commit_dependent_diffs,
    })
}
