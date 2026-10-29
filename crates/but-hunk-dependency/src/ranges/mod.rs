use std::collections::{HashMap, HashSet};

use but_core::TreeStatusKind;
use gix::bstr::{BString, ByteSlice};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{InputCommit, InputDiffHunk, InputStack, ui::HunkLockTarget};
use but_core::unified_diff::DiffHunk;

mod hunk;
pub use hunk::HunkRange;

mod paths;
use paths::PathRanges;

/// All hunk-dependencies for the entire workspace.
#[derive(Debug)]
pub struct WorkspaceRanges {
    paths: HashMap<BString, Vec<HunkRange>>,
    /// Errors that occurred while computing the fields in this instance.
    pub errors: Vec<CalculationError>,
}

/// An error that can say what went wrong when computing the hunk ranges for a commit in a stack at a given path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[expect(missing_docs)]
pub struct CalculationError {
    pub error_message: String,
    pub target: HunkLockTarget,
    #[serde(serialize_with = "but_serde::object_id::serialize")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub commit_id: gix::ObjectId,
    #[serde(serialize_with = "but_serde::bstring_lossy::serialize")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub path: BString,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CalculationError);

#[derive(Debug, Default)]
struct StackRanges {
    paths: HashMap<BString, PathRanges>,
}

/// A struct for collecting hunk ranges by path, before they get merged into a single dimension
/// representing the workspace view.
impl StackRanges {
    fn add(
        &mut self,
        target: HunkLockTarget,
        commit_id: gix::ObjectId,
        path: BString,
        change_type: TreeStatusKind,
        diffs: Vec<InputDiffHunk>,
    ) -> anyhow::Result<()> {
        self.paths
            .entry(path)
            .or_default()
            .add(target, commit_id, change_type, diffs)?;

        Ok(())
    }

    pub fn unique_paths(&self) -> HashSet<BString> {
        self.paths
            .keys()
            .unique()
            .map(|path| path.to_owned())
            .collect::<HashSet<BString>>()
    }
}

/// Provides blame-like functionality for looking up what commit(s) have touched a specific line
/// number range for a given path.
///
/// First it combines changes per branch sequentially by commit, allowing for dependent changes
/// where one commit introduces changes that overwrites previous changes.
///
/// It then combines the changes per branch into a single vector with line numbers that should
/// match the workspace commit. These per branch changes are assumed and required to be
/// independent without overlap.
impl WorkspaceRanges {
    /// Calculates all ranges for the workspace, which is identified by `input_stacks`,
    /// i.e. all stacks that make up that workspace.
    pub fn try_from_stacks(input_stacks: Vec<InputStack>) -> anyhow::Result<WorkspaceRanges> {
        let mut stacks = vec![];
        let mut errors = vec![];
        for input_stack in input_stacks {
            let mut stack_ranges = StackRanges {
                ..Default::default()
            };
            let InputStack {
                target: stack_id,
                commits_from_base_to_tip: commits,
            } = input_stack;
            for commit in commits {
                let InputCommit { commit_id, files } = commit;
                for file in files {
                    if let Some(error) = stack_ranges
                        .add(
                            stack_id,
                            commit_id,
                            file.path.clone(),
                            file.change_type,
                            file.hunks,
                        )
                        .err()
                    {
                        errors.push(CalculationError {
                            error_message: error.to_string(),
                            target: stack_id,
                            commit_id,
                            path: file.path,
                        });
                    }
                }
            }
            stacks.push(stack_ranges);
        }
        let paths = stacks
            .iter()
            .flat_map(StackRanges::unique_paths)
            .unique()
            .collect_vec();
        Ok(WorkspaceRanges {
            paths: paths
                .iter()
                .map(|path| (path.clone(), combine_path_ranges(path, &stacks)))
                .collect(),
            errors,
        })
    }

    /// Finds commits that intersect with a given path and hunk.
    ///
    /// For `Modification`-type commit ranges that only *touch* (are adjacent to, but don't
    /// overlap) the worktree hunk, the adjacency is only counted as an intersection when the
    /// hunk's diff content suggests the change is a reorder or block-move — the class of
    /// edits that can produce an LCS insertion point right at the boundary and thereby cause
    /// a cherry-pick conflict even with no overlapping lines.  Plain substitutions (where
    /// none of the old lines appear in the new lines) are not flagged.
    pub fn intersection(&self, path: &BString, hunk: &DiffHunk) -> Option<Vec<&HunkRange>> {
        let start = hunk.old_start;
        let lines = hunk.old_lines;
        if let Some(hunk_range) = self.paths.get(path) {
            let intersection = hunk_range
                .iter()
                .filter(|hr| {
                    if hr.change_type == TreeStatusKind::Modification {
                        if hr.intersects(start, lines).unwrap_or(false) {
                            return true;
                        }
                        // Extend to adjacent ranges when:
                        // - the worktree hunk is a pure insertion (old_lines == 0)
                        //   AND the commit range is a deletion point (hr.lines == 0):
                        //   a deletion removes the anchor that the merge algorithm
                        //   uses to place the insertion, so this can conflict.
                        //   Insertions adjacent to normal modification spans are safe
                        //   because `intersects()` already catches insertions *inside*
                        //   the span, and the merge algorithm can anchor outside it.
                        // - the hunk content signals a reorder/move (not a plain
                        //   substitution) via `hunk_suggests_boundary_insertion`.
                        hr.intersects_or_adjacent(start, lines).unwrap_or(false)
                            && ((lines == 0 && hr.lines == 0)
                                || (lines > 0 && hunk_suggests_boundary_insertion(&hunk.diff)))
                    } else {
                        // For additions and deletions, we consider the hunk to always intersect.
                        true
                    }
                })
                .collect_vec();
            if !intersection.is_empty() {
                return Some(intersection);
            }
        }
        None
    }

    /// Like [`intersection`], but takes raw coordinates instead of a [`DiffHunk`].
    /// The adjacency check is not applied — only strict range overlap is tested.
    /// Prefer [`intersection`] in production code; this exists for unit tests that
    /// construct ranges without real diff content.
    #[cfg(test)]
    pub(crate) fn intersection_at(
        &self,
        path: &BString,
        start: u32,
        lines: u32,
    ) -> Option<Vec<&HunkRange>> {
        if let Some(hunk_range) = self.paths.get(path) {
            let intersection = hunk_range
                .iter()
                .filter(|hunk| {
                    if hunk.change_type == TreeStatusKind::Modification {
                        hunk.intersects(start, lines).unwrap_or(false)
                    } else {
                        true
                    }
                })
                .collect_vec();
            if !intersection.is_empty() {
                return Some(intersection);
            }
        }
        None
    }

    /// Return a reference to the internal mapping that is used for [`Self::intersection()`]
    pub fn ranges_by_path_map(&self) -> &HashMap<BString, Vec<HunkRange>> {
        &self.paths
    }
}

/// Returns `true` when the hunk diff content suggests that applying this change may create
/// an LCS insertion point at the trailing boundary — the condition that causes a cherry-pick
/// conflict with an adjacent commit even when the line ranges don't strictly overlap.
///
/// The signal is a *reorder or block-move*: at least one line appears in both the old (`-`)
/// and new (`+`) sides, **and** the last new line differs from the last old line (meaning the
/// LCS is unlikely to terminate with a matched anchor at the end, leaving an unmatched
/// insertion at the boundary).
///
/// Plain substitutions (e.g. `- "2"\n+ "two"`) return `false` because no line is shared
/// between old and new, so the LCS cannot produce a boundary collision.
fn hunk_suggests_boundary_insertion(diff: &gix::bstr::BString) -> bool {
    let content = diff.to_str_lossy();
    // DiffHunk.diff starts with an @@ header line, never ---/+++ file headers,
    // so we skip lines starting with @@ and collect all -/+ prefixed lines.
    let old_lines: Vec<&str> = content
        .lines()
        .filter(|l: &&str| l.starts_with('-'))
        .map(|l| &l[1..])
        .collect();
    let new_lines: Vec<&str> = content
        .lines()
        .filter(|l: &&str| l.starts_with('+'))
        .map(|l| &l[1..])
        .collect();

    if old_lines.is_empty() || new_lines.is_empty() {
        return false;
    }
    // At least one line must appear in both sides (movement/reorder signal).
    let has_moved_line = old_lines.iter().any(|o| new_lines.iter().any(|n| o == n));
    if !has_moved_line {
        return false;
    }
    // The last new line must differ from the last old line (trailing insertion signal).
    old_lines.last() != new_lines.last()
}

/// Combines ranges from multiple branches/stacks into a single vector
/// with adjusted line numbers. For this to work it is required that changes
/// between stacks are not overlapping, which is already a hard requirement.
fn combine_path_ranges(path: &BString, stacks: &[StackRanges]) -> Vec<HunkRange> {
    let mut result: Vec<HunkRange> = vec![];

    // Only process stacks that contain the path.
    let filtered_paths = stacks
        .iter()
        .filter_map(|stack| stack.paths.get(path))
        .collect_vec();

    // Tracks the cumulative lines added/removed.
    let mut line_shifts = vec![0i32; filtered_paths.len()];

    // Next hunk to consider for each branch containing path.
    let mut hunk_indexes: Vec<usize> = vec![0; filtered_paths.len()];

    loop {
        let start_lines = filtered_paths
            .iter()
            .enumerate()
            .map(|(i, path_dep)| path_dep.hunk_ranges.get(hunk_indexes[i]))
            .map(|hunk| hunk.map(|hunk_dep| hunk_dep.start))
            .collect_vec();

        // Find the index of the dependency path with the lowest start line.
        let next_index = start_lines
            .iter()
            .enumerate() // We want to filter out None values, but keep their index.
            .filter(|(_, start_line)| start_line.is_some())
            .min_by_key(|&(index, &start_line)| {
                start_line.unwrap() + start_lines[index].unwrap_or(0)
            })
            .map(|(index, _)| index);

        if next_index.is_none() {
            break; // No more items to process.
        }

        let next_index = next_index.unwrap();
        let hunk_index = hunk_indexes[next_index];

        // Get the path with the lowest next start line.
        let path_dep = &filtered_paths[next_index];
        let hunk_dep = &path_dep.hunk_ranges[hunk_index];

        result.push(HunkRange {
            start: hunk_dep
                .start
                .saturating_add_signed(line_shifts[next_index]),
            ..*hunk_dep
        });

        // Advance the path specific hunk pointer.
        hunk_indexes[next_index] += 1;

        // Increment shift for all stacks except the one this hunk belongs to.
        for (i, shift) in line_shifts.iter_mut().enumerate() {
            if i != next_index {
                *shift += hunk_dep.line_shift;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
