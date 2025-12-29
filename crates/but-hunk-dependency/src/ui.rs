use but_core::{UnifiedPatch, unified_diff::DiffHunk};
use but_ctx::Context;
use but_oxidize::OidExt;
use gitbutler_stack::StackId;
use serde::{Deserialize, Serialize};

/// Compute the hunk dependencies of a set of tree changes.
pub fn hunk_dependencies_for_changes(
    ctx: &Context,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<HunkDependencies> {
    // accelerate tree-tree-diffs
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let project_data_dir = &ctx.project_data_dir();
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
    )?;
    let stacks = but_workspace::legacy::stacks_v3(&repo, &meta, Default::default(), None)?;
    let common_merge_base = gitbutler_stack::VirtualBranchesHandle::new(project_data_dir)
        .get_default_target()?
        .sha;
    let input_stacks =
        crate::workspace_stacks_to_input_stacks(&repo, &stacks, common_merge_base.to_gix())?;
    let ranges = crate::WorkspaceRanges::try_from_stacks(input_stacks)?;
    HunkDependencies::try_from_workspace_ranges(&repo, ranges, changes)
}

/// Compute hunk-dependencies for the UI knowing the `worktree_dir` for changes
/// and `gitbutler_dir` for obtaining stack information.
pub fn hunk_dependencies_for_workspace_changes_by_worktree_dir(
    ctx: &Context,
    worktree_changes: Option<Vec<but_core::TreeChange>>,
) -> anyhow::Result<HunkDependencies> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let worktree_changes = worktree_changes
        .map(Ok)
        .unwrap_or_else(|| but_core::diff::worktree_changes(&repo).map(|wtc| wtc.changes))?;
    hunk_dependencies_for_changes(ctx, worktree_changes)
}

/// A way to represent all hunk dependencies that would make it possible to know what can be applied, and were.
///
/// Note that the [`errors`](Self::errors) field may contain information about specific failures, while other paths
/// may have succeeded computing.
#[derive(Debug, Clone, Serialize, Default)]
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
                if let Some(intersections) =
                    ranges.intersection(&change.path, hunk.old_start, hunk.old_lines)
                {
                    let locks: Vec<_> = intersections
                        .into_iter()
                        .map(|dependency| HunkLock {
                            commit_id: dependency.commit_id,
                            stack_id: dependency.stack_id,
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
    /// The ID of the stack that contains [`commit_id`](Self::commit_id).
    pub stack_id: StackId,
    /// The commit the hunk applies to.
    #[serde(with = "but_serde::object_id")]
    pub commit_id: gix::ObjectId,
}
