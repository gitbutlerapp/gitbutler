use std::collections::HashMap;

use gitbutler_diff::{Hunk, HunkHash};
use gitbutler_stack::StackId;
use itertools::Itertools;
use serde::Serialize;

use crate::workspace::{HunkDependencyOptions, WorkspaceHunkRanges};

// A hunk is locked when it depends on changes in commits that are in your
// workspace. A hunk can be locked to more than one branch if it overlaps
// with more than one committed hunk.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    pub branch_id: StackId,
    #[serde(with = "gitbutler_serde::oid")]
    pub commit_id: git2::Oid,
}

pub fn compute_hunk_locks(
    options: HunkDependencyOptions,
) -> anyhow::Result<HashMap<HunkHash, Vec<HunkLock>>> {
    let HunkDependencyOptions { workdir, stacks } = options;
    let workspace_deps = WorkspaceHunkRanges::new(stacks);
    let mut result = HashMap::new();

    for (path, workspace_hunks) in workdir {
        for workspace_hunk in workspace_hunks {
            let dependencies = workspace_deps.intersection(
                path,
                workspace_hunk.old_start as i32,
                workspace_hunk.old_lines as i32,
            );
            let hash = Hunk::hash_diff(&workspace_hunk.diff_lines);
            let locks = dependencies
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
