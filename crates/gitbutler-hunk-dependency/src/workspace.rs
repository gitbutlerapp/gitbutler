use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use itertools::Itertools;

use crate::{HunkRange, InputCommit, InputStack, StackRanges};

#[derive(Debug)]
pub struct WorkspaceRanges {
    paths: HashMap<PathBuf, Vec<HunkRange>>,
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
    pub fn create(input_stacks: Vec<InputStack>) -> anyhow::Result<WorkspaceRanges> {
        let mut stacks = vec![];
        for input_stack in input_stacks {
            let mut stack = StackRanges::default();
            let InputStack { stack_id, commits } = input_stack;
            for commit in commits {
                let InputCommit { commit_id, files } = commit;
                for file in files {
                    stack.add(stack_id, commit_id, &file.path, file.diffs)?;
                }
            }
            stacks.push(stack);
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
        })
    }

    /// Finds commits that intersect with a given path and range combination.
    pub fn intersection(&self, path: &Path, start: u32, lines: u32) -> Option<Vec<&HunkRange>> {
        if let Some(hunk_range) = self.paths.get(path) {
            let intersection = hunk_range
                .iter()
                .filter(|hunk| hunk.intersects(start, lines))
                .collect_vec();
            if !intersection.is_empty() {
                return Some(intersection);
            }
        }
        None
    }
}

/// Combines ranges from muiltiple branches/stacks into a single vector
/// with adjusted line numbers. For this to work it is required that changes
/// between stacks are not overlapping, which is already a hard requirement.
fn combine_path_ranges(path: &Path, stacks: &[StackRanges]) -> Vec<HunkRange> {
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
            .map(|(i, path_dep)| path_dep.hunks.get(hunk_indexes[i]))
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
        let hunk_dep = &path_dep.hunks[hunk_index];

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
mod tests {
    use std::str::FromStr;

    use gitbutler_stack::StackId;

    use crate::input::{InputDiff, InputFile};

    use super::*;

    #[test]
    fn workspace_simple() -> anyhow::Result<()> {
        let path = PathBuf::from_str("/test.txt")?;

        let commit1_id = git2::Oid::from_str("a")?;
        let stack1_id = StackId::generate();

        let commit2_id = git2::Oid::from_str("b")?;
        let stack2_id = StackId::generate();

        let workspace_ranges = WorkspaceRanges::create(vec![
            InputStack {
                stack_id: stack1_id,
                commits: vec![InputCommit {
                    commit_id: commit1_id,
                    files: vec![InputFile {
                        path: path.to_owned(),
                        diffs: vec![InputDiff::try_from(
                            "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
                        )?],
                    }],
                }],
            },
            InputStack {
                stack_id: stack2_id,
                commits: vec![InputCommit {
                    commit_id: commit2_id,
                    files: vec![InputFile {
                        path: path.to_owned(),
                        diffs: vec![
                            InputDiff::try_from(
                                "@@ -1,5 +1,3 @@
-1
-2
3
5
6
",
                            )?,
                            InputDiff::try_from(
                                "@@ -10,6 +8,7 @@
10
11
12
+13
14
15
16
",
                            )?,
                        ],
                    }],
                }],
            },
        ])?;

        let dependencies_1 = workspace_ranges.intersection(&path, 2, 1).unwrap();
        assert_eq!(dependencies_1.len(), 1);
        assert_eq!(dependencies_1[0].commit_id, commit1_id);
        assert_eq!(dependencies_1[0].stack_id, stack1_id);

        let dependencies_2 = workspace_ranges.intersection(&path, 12, 1).unwrap();
        assert_eq!(dependencies_2.len(), 1);
        assert_eq!(dependencies_2[0].commit_id, commit2_id);
        assert_eq!(dependencies_2[0].stack_id, stack2_id);

        Ok(())
    }
}
