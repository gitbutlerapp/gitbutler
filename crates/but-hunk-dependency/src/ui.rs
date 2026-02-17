use std::fmt::Display;

use but_core::{RepositoryExt, UnifiedPatch, ref_metadata::StackId, unified_diff::DiffHunk};
use serde::{Deserialize, Serialize};

/// Compute the hunk dependencies of a set of tree changes.
fn hunk_dependencies_for_changes(
    repo: &gix::Repository,
    workspace: &but_graph::projection::Workspace,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<HunkDependencies> {
    // accelerate tree-tree-diffs
    let repo = repo.clone().for_tree_diffing()?.with_object_memory();
    let input_stacks = crate::new_stacks_to_input_stacks(&repo, workspace)?;
    let ranges = crate::WorkspaceRanges::try_from_stacks(input_stacks)?;
    HunkDependencies::try_from_workspace_ranges(&repo, ranges, changes)
}

/// Compute hunk-dependencies for the UI knowing the `worktree_dir` for changes
/// and `gitbutler_dir` for obtaining stack information.
pub fn hunk_dependencies_for_workspace_changes_by_worktree_dir(
    repo: &gix::Repository,
    workspace: &but_graph::projection::Workspace,
    worktree_changes: Option<Vec<but_core::TreeChange>>,
) -> anyhow::Result<HunkDependencies> {
    let worktree_changes = worktree_changes
        .map(Ok)
        .unwrap_or_else(|| but_core::diff::worktree_changes(repo).map(|wtc| wtc.changes))?;
    hunk_dependencies_for_changes(repo, workspace, worktree_changes)
}

/// A way to represent all hunk dependencies that would make it possible to know what can be applied, and were.
///
/// Note that the [`errors`](Self::errors) field may contain information about specific failures, while other paths
/// may have succeeded computing.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HunkDependencies {
    /// A map from hunk diffs to stack and commit dependencies.
    pub diffs: Vec<(String, DiffHunk, Vec<HunkLock>)>,
    /// Errors that occurred during the calculation that should be presented in some way.
    // TODO: Does the UI really use whatever partial result that there may be? Should this be a real error?
    pub errors: Vec<crate::CalculationError>,
}

impl HunkDependencies {
    /// Calculate all hunk dependencies using a preparepd [`crate::WorkspaceRanges`].
    pub fn try_from_workspace_ranges(
        repo: &gix::Repository,
        ranges: crate::WorkspaceRanges,
        worktree_changes: Vec<but_core::TreeChange>,
    ) -> anyhow::Result<HunkDependencies> {
        let mut diffs = Vec::<(String, DiffHunk, Vec<HunkLock>)>::new();
        for change in worktree_changes {
            let unidiff = change.unified_patch(repo, 0 /* zero context lines */)?;
            let Some(UnifiedPatch::Patch { hunks, .. }) = unidiff else {
                continue;
            };
            for hunk in hunks {
                if let Some(intersections) = ranges.intersection(&change.path, hunk.old_start, hunk.old_lines) {
                    let locks: Vec<_> = intersections
                        .into_iter()
                        .map(|dependency| HunkLock {
                            commit_id: dependency.commit_id,
                            target: dependency.target,
                        })
                        .collect();
                    diffs.push((change.path.to_string(), hunk, locks));
                }
            }
        }

        Ok(HunkDependencies {
            diffs,
            errors: ranges.errors,
        })
    }
}

/// A commit that owns this lock, along with the stack that owns it.
/// A hunk is locked when it depends on changes in commits that are in your workspace. A hunk can
/// be locked to more than one branch if it overlaps with more than one committed hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    /// The ID if available of the stack that contains
    /// [`commit_id`](Self::commit_id).
    pub target: HunkLockTarget,
    /// The commit the hunk applies to.
    #[serde(with = "but_serde::object_id")]
    pub commit_id: gix::ObjectId,
}

/// The target of a hunk lock. If a stack is identifiable, then it's StackId
/// will be provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum HunkLockTarget {
    /// References a stack that has a StackId.
    Stack(StackId),
    /// The hunk is locked to a stack that we can't reference because it didn't
    /// have a StackId. This is likely because the stack that the change is
    /// locked to doesn't have any associated metadata or doesn't have anything
    /// we can use to associate it with metadata.
    Unidentified,
}

impl From<HunkLockTarget> for Option<StackId> {
    fn from(val: HunkLockTarget) -> Self {
        match val {
            HunkLockTarget::Stack(s) => Some(s),
            HunkLockTarget::Unidentified => None,
        }
    }
}

impl Display for HunkLockTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stack(s) => write!(f, "{s}"),
            Self::Unidentified => write!(f, "unidentified"),
        }
    }
}
