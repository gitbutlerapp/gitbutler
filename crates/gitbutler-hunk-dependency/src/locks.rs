use std::{collections::HashMap, path::PathBuf};

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

/// Returns a map from hunk hash to hunk locks.
///
/// To understand if any uncommitted changes depend on (intersect) any existing changes we first
/// transform the branch specific line numbers to the workspace, then look for places they
/// intersect.
///
/// TODO: Change terminology to talk about dependencies instead of locks.
pub fn compute_hunk_locks(
    options: HunkDependencyOptions,
) -> anyhow::Result<HashMap<HunkHash, Vec<HunkLock>>> {
    let HunkDependencyOptions { workdir, stacks } = options;
    // Transforms stack specific line numbers to workspace line numbers.
    let ranges = WorkspaceRanges::create(stacks)?;

    Ok(workdir
        .iter()
        .flat_map(|(path, workspace_hunks)| {
            workspace_hunks.iter().filter_map(|hunk| {
                let locks = ranges
                    .intersection(path, hunk.old_start, hunk.old_lines)
                    .iter()
                    .map(|dependency| HunkLock {
                        commit_id: dependency.commit_id,
                        branch_id: dependency.stack_id,
                    })
                    .collect_vec();
                if locks.is_empty() {
                    return None;
                }
                Some((Hunk::hash_diff(&hunk.diff_lines), locks))
            })
        })
        .collect())
}
