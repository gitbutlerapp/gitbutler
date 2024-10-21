use std::{collections::HashMap, path::PathBuf};

use gitbutler_diff::{Hunk, HunkHash};
use gitbutler_stack::StackId;
use itertools::Itertools;
use serde::Serialize;

use crate::{InputStack, WorkspaceRanges};

// Type defined in gitbutler-branch-actions and can't be imported here.
type BranchStatus = HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>;

// A hunk is locked when it depends on changes in commits that are in your
// workspace. A hunk can be locked to more than one branch if it overlaps
// with more than one committed hunk.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    // TODO: Rename this stack_id.
    pub branch_id: StackId,
    #[serde(with = "gitbutler_serde::oid")]
    pub commit_id: git2::Oid,
}

pub struct HunkDependencyOptions<'a> {
    // Uncommitted changes in workspace.
    pub workdir: &'a BranchStatus,

    /// A nested map of committed diffs per stack, commit, and path.
    pub stacks: Vec<InputStack>,
}

/// Returns a map from hunk hash to hunk locks.
///
/// To understand if any uncommitted changes depend on (intersect) any existing
/// changes we first transform the branch specific line numbers to global ones,
/// then look for places they intersect.
///
/// TODO: Change terminology to talk about dependencies instead of locks.
pub fn compute_hunk_locks(
    options: HunkDependencyOptions,
) -> anyhow::Result<HashMap<HunkHash, Vec<HunkLock>>> {
    let HunkDependencyOptions { workdir, stacks } = options;

    // Transforms local line numbers to global line numbers.
    let workspace_ranges = WorkspaceRanges::create(stacks)?;

    let mut result = HashMap::new();

    for (path, workspace_hunks) in workdir {
        for hunk in workspace_hunks {
            let hunk_dependencies =
                workspace_ranges.intersection(path, hunk.old_start, hunk.old_lines);
            let hash = Hunk::hash_diff(&hunk.diff_lines);
            let locks = hunk_dependencies
                .iter()
                .map(|dependency| HunkLock {
                    commit_id: dependency.commit_id,
                    branch_id: dependency.stack_id,
                })
                .collect_vec();

            if !locks.is_empty() {
                result.insert(hash, locks);
            }
        }
    }
    Ok(result)
}
