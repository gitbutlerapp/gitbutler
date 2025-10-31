use std::collections::{HashMap, HashSet};

use but_core::TreeStatusKind;
use but_workspace::StackId;
use gix::bstr::BString;
use itertools::Itertools;
use serde::Serialize;

use crate::{InputCommit, InputDiffHunk, InputStack};

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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(missing_docs)]
pub struct CalculationError {
    pub error_message: String,
    pub stack_id: StackId,
    #[serde(serialize_with = "gitbutler_serde::object_id::serialize")]
    pub commit_id: gix::ObjectId,
    pub path: BString,
}

#[derive(Debug, Default)]
struct StackRanges {
    paths: HashMap<BString, PathRanges>,
}

/// A struct for collecting hunk ranges by path, before they get merged into a single dimension
/// representing the workspace view.
impl StackRanges {
    fn add(
        &mut self,
        stack_id: StackId,
        commit_id: gix::ObjectId,
        path: BString,
        change_type: TreeStatusKind,
        diffs: Vec<InputDiffHunk>,
    ) -> anyhow::Result<()> {
        self.paths
            .entry(path)
            .or_default()
            .add(stack_id, commit_id, change_type, diffs)?;

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
                stack_id,
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
                            stack_id,
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

    /// Finds commits that intersect with a given path and range combination.
    pub fn intersection(&self, path: &BString, start: u32, lines: u32) -> Option<Vec<&HunkRange>> {
        if let Some(hunk_range) = self.paths.get(path) {
            let intersection = hunk_range
                .iter()
                .filter(|hunk| hunk.intersects(start, lines).unwrap_or(false))
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

/// Combines ranges from muiltiple branches/stacks into a single vector
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
