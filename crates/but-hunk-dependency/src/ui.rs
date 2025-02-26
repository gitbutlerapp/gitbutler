use but_core::UnifiedDiff;
use gitbutler_oxidize::OidExt;
use gitbutler_stack::StackId;
use gix::bstr::ByteSlice;
use serde::Serialize;
use std::hash::Hasher;
use std::path::Path;

/// Compute hunk-dependencies for the UI knowing the `worktree_dir` for changes
/// and `gitbutler_dir` for obtaining stack information.
pub fn hunk_dependencies_for_workspace_changes_by_worktree_dir(
    worktree_dir: &Path,
    gitbutler_dir: &Path,
) -> anyhow::Result<HunkDependencies> {
    let repo = gix::open(worktree_dir).map_err(anyhow::Error::from)?;
    let worktree_changes = but_core::diff::worktree_changes(&repo)?;
    let stacks = but_workspace::stacks(gitbutler_dir)?;
    let common_merge_base = gitbutler_stack::VirtualBranchesHandle::new(gitbutler_dir)
        .get_default_target()?
        .sha;
    let input_stacks =
        crate::workspace_stacks_to_input_stacks(&repo, &stacks, common_merge_base.to_gix())?;
    let ranges = crate::WorkspaceRanges::try_from_stacks(input_stacks)?;
    HunkDependencies::try_from_workspace_ranges(&repo, ranges, worktree_changes.changes)
}

/// Calculate as hash for a `universal_diff`.
// TODO: see if this should be avoided entirely here as the current impl would allow for hash collisions.
pub fn hash_lines(universal_diff: impl AsRef<[u8]>) -> HunkHash {
    let diff = universal_diff.as_ref();
    assert!(
        diff.starts_with(b"@@"),
        "BUG: input mut be a universal diff"
    );
    let mut ctx = rustc_hash::FxHasher::default();
    diff.lines_with_terminator()
        .skip(1) // skip the first line which is the diff header.
        .for_each(|line| ctx.write(line));
    ctx.finish()
}

/// A way to represent all hunk dependencies that would make it possible to know what can be applied, and were.
///
/// Note that the [`errors`](Self::errors) field may contain information about specific failures, while other paths
/// may have succeeded computing.
#[derive(Debug, Clone, Serialize)]
pub struct HunkDependencies {
    /// A map from diffs to branch and commit dependencies.
    // TODO: could this be a specific type? Is the mapping truly required?
    //       Is this because `commit_dependent_diffs` use `HunkHash`?
    // TODO: the frontend actually has no way of associating the hunks it gets with this hash as it's made
    //       on the patch lines without any context lines, while it has context lines.
    //       Hash must then skip the context lines if there are any.
    pub diffs: Vec<(HunkHash, Vec<HunkLock>)>,
    /// Errors that occurred during the calculation that should be presented in some way.
    // TODO: Does the UI really use whatever partial result that there may be? Should this be a real error?
    pub errors: Vec<crate::CalculationError>,
}

impl HunkDependencies {
    /// Calculate all hunk dependencies using a preparepd [`crate::WorkspaceRanges`].
    fn try_from_workspace_ranges(
        repo: &gix::Repository,
        ranges: crate::WorkspaceRanges,
        worktree_changes: Vec<but_core::TreeChange>,
    ) -> anyhow::Result<HunkDependencies> {
        let mut diffs = Vec::<(HunkHash, Vec<HunkLock>)>::new();
        for change in worktree_changes {
            let unidiff = change.unified_diff(repo, 0 /* zero context lines */)?;
            let UnifiedDiff::Patch { hunks } = unidiff else {
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
                    diffs.push((hash_lines(&hunk.diff), locks));
                }
            }
        }

        Ok(HunkDependencies {
            diffs,
            errors: ranges.errors,
        })
    }
}

/// A hash over the universal diff of a hunk.
// TODO: using the hash directly like we do can collide, would have to use actual Hunk to prevent this issue.
pub type HunkHash = u64;

/// A commit that owns this lock, along with the stack that owns it.
/// A hunk is locked when it depends on changes in commits that are in your workspace. A hunk can
/// be locked to more than one branch if it overlaps with more than one committed hunk.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    /// The ID of the stack that contains [`commit_id`](Self::commit_id).
    pub stack_id: StackId,
    /// The commit the hunk applies to.
    #[serde(serialize_with = "gitbutler_serde::object_id::serialize")]
    pub commit_id: gix::ObjectId,
}
