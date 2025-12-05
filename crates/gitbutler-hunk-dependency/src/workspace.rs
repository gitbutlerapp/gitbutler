use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use gitbutler_stack::StackId;
use itertools::Itertools;
use serde::Serialize;

use crate::{HunkRange, InputCommit, InputStack, StackRanges};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeCalculationError {
    pub error_message: String,
    pub stack_id: StackId,
    #[serde(with = "but_serde::oid")]
    pub commit_id: git2::Oid,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct WorkspaceRanges {
    paths: HashMap<PathBuf, Vec<HunkRange>>,
    pub commit_dependencies: HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>>,
    pub inverse_commit_dependencies: HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>>,
    pub errors: Vec<RangeCalculationError>,
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
        let mut errors = vec![];
        for input_stack in input_stacks {
            let mut stack_ranges = StackRanges {
                stack_id: input_stack.stack_id.into(),
                ..Default::default()
            };
            let InputStack { stack_id, commits } = input_stack;
            for commit in commits {
                let InputCommit { commit_id, files } = commit;
                for file in files {
                    if let Some(error) = stack_ranges
                        .add(stack_id, commit_id, &file.path, file.diffs)
                        .err()
                    {
                        errors.push(RangeCalculationError {
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

        let commit_dependencies = stacks
            .iter()
            .filter_map(|stack| {
                stack
                    .stack_id
                    .map(|id| (id, stack.get_commit_dependencies()))
            })
            .collect();
        let inverse_commit_dependencies = get_inverted_dependency_maps(&commit_dependencies);

        Ok(WorkspaceRanges {
            paths: paths
                .iter()
                .map(|path| (path.clone(), combine_path_ranges(path, &stacks)))
                .collect(),
            commit_dependencies,
            inverse_commit_dependencies,
            errors,
        })
    }

    /// Finds commits that intersect with a given path and range combination.
    pub fn intersection(&self, path: &Path, start: u32, lines: u32) -> Option<Vec<&HunkRange>> {
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
}

/// Combines ranges from multiple branches/stacks into a single vector
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

fn get_inverted_dependency_maps(
    commit_dependencies: &HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>>,
) -> HashMap<StackId, HashMap<git2::Oid, HashSet<git2::Oid>>> {
    commit_dependencies
        .iter()
        .map(|(stack_id, dependencies)| {
            (
                *stack_id,
                dependencies
                    .iter()
                    .flat_map(|(key, values)| values.iter().map(move |value| (value, key)))
                    .fold(
                        HashMap::new(),
                        |mut acc: HashMap<git2::Oid, HashSet<git2::Oid>>, (value, key)| {
                            acc.entry(*value).or_default().insert(*key);
                            acc
                        },
                    ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use gitbutler_stack::StackId;

    use super::*;
    use crate::{InputDiff, input::InputFile, parse_diff_from_string};

    #[test]
    fn get_inverted_dependency_maps_test_single_stack() {
        let stack_id = StackId::generate();
        let commit_a = git2::Oid::from_str("a").unwrap();
        let commit_b = git2::Oid::from_str("b").unwrap();
        let commit_c = git2::Oid::from_str("c").unwrap();
        let commit_d = git2::Oid::from_str("d").unwrap();

        let original_map = {
            let mut map = HashMap::new();
            map.insert(stack_id, {
                let mut inner_map = HashMap::new();
                inner_map.insert(commit_a, {
                    let mut set = HashSet::new();
                    set.insert(commit_b);
                    set.insert(commit_c);
                    set.insert(commit_d);
                    set
                });
                inner_map
            });
            map
        };

        let inverted_map = get_inverted_dependency_maps(&original_map);
        assert_eq!(inverted_map.len(), 1);
        let stack_inverted_map = inverted_map.get(&stack_id).unwrap();
        assert_eq!(stack_inverted_map.len(), 3);
        // b
        assert!(stack_inverted_map.contains_key(&commit_b));
        let commit_b_deps = stack_inverted_map.get(&commit_b).unwrap();
        assert_eq!(commit_b_deps.len(), 1);
        assert!(commit_b_deps.contains(&commit_a));
        // c
        assert!(stack_inverted_map.contains_key(&commit_c));
        let commit_c_deps = stack_inverted_map.get(&commit_c).unwrap();
        assert_eq!(commit_c_deps.len(), 1);
        assert!(commit_c_deps.contains(&commit_a));
        // d
        assert!(stack_inverted_map.contains_key(&commit_d));
        let commit_d_deps = stack_inverted_map.get(&commit_d).unwrap();
        assert_eq!(commit_d_deps.len(), 1);
        assert!(commit_d_deps.contains(&commit_a));
    }

    #[test]
    fn get_inverted_dependency_maps_test_multiple_stacks() {
        let stack_id_a = StackId::generate();
        let stack_id_b = StackId::generate();

        let commit_a = git2::Oid::from_str("a").unwrap();
        let commit_b = git2::Oid::from_str("b").unwrap();
        let commit_c = git2::Oid::from_str("c").unwrap();
        let commit_d = git2::Oid::from_str("d").unwrap();
        let commit_e = git2::Oid::from_str("e").unwrap();
        let commit_f = git2::Oid::from_str("f").unwrap();
        let commit_g = git2::Oid::from_str("0").unwrap();
        let commit_h = git2::Oid::from_str("1").unwrap();

        let original_map = {
            let mut map = HashMap::new();
            map.insert(stack_id_a, {
                let mut inner_map = HashMap::new();
                inner_map.insert(commit_a, {
                    let mut set = HashSet::new();
                    set.insert(commit_b);
                    set.insert(commit_c);
                    set.insert(commit_d);
                    set
                });
                inner_map
            });
            map.insert(stack_id_b, {
                let mut inner_map = HashMap::new();
                inner_map.insert(commit_e, {
                    let mut set = HashSet::new();
                    set.insert(commit_f);
                    set.insert(commit_g);
                    set.insert(commit_h);
                    set
                });
                inner_map
            });
            map
        };

        let inverted_map = get_inverted_dependency_maps(&original_map);
        assert_eq!(inverted_map.len(), 2);
        // stack a
        assert!(inverted_map.contains_key(&stack_id_a));
        let stack_a_inverted_map = inverted_map.get(&stack_id_a).unwrap();
        assert_eq!(stack_a_inverted_map.len(), 3);
        // === b
        assert!(stack_a_inverted_map.contains_key(&commit_b));
        let commit_b_deps = stack_a_inverted_map.get(&commit_b).unwrap();
        assert_eq!(commit_b_deps.len(), 1);
        assert!(commit_b_deps.contains(&commit_a));
        // === c
        assert!(stack_a_inverted_map.contains_key(&commit_c));
        let commit_c_deps = stack_a_inverted_map.get(&commit_c).unwrap();
        assert_eq!(commit_c_deps.len(), 1);
        assert!(commit_c_deps.contains(&commit_a));
        // === d
        assert!(stack_a_inverted_map.contains_key(&commit_d));
        let commit_d_deps = stack_a_inverted_map.get(&commit_d).unwrap();
        assert_eq!(commit_d_deps.len(), 1);
        assert!(commit_d_deps.contains(&commit_a));

        // stack b
        assert!(inverted_map.contains_key(&stack_id_b));
        let stack_b_inverted_map = inverted_map.get(&stack_id_b).unwrap();
        assert_eq!(stack_b_inverted_map.len(), 3);
        // === f
        assert!(stack_b_inverted_map.contains_key(&commit_f));
        let commit_f_deps = stack_b_inverted_map.get(&commit_f).unwrap();
        assert_eq!(commit_f_deps.len(), 1);
        assert!(commit_f_deps.contains(&commit_e));
        // === g
        assert!(stack_b_inverted_map.contains_key(&commit_g));
        let commit_g_deps = stack_b_inverted_map.get(&commit_g).unwrap();
        assert_eq!(commit_g_deps.len(), 1);
        assert!(commit_g_deps.contains(&commit_e));
        // === h
        assert!(stack_b_inverted_map.contains_key(&commit_h));
        let commit_h_deps = stack_b_inverted_map.get(&commit_h).unwrap();
        assert_eq!(commit_h_deps.len(), 1);
        assert!(commit_h_deps.contains(&commit_e));
    }

    #[test]
    fn get_inverted_dependency_maps_test_multple_dependencies() {
        let stack_id_a = StackId::generate();
        let stack_id_b = StackId::generate();

        let commit_a = git2::Oid::from_str("a").unwrap();
        let commit_b = git2::Oid::from_str("b").unwrap();
        let commit_c = git2::Oid::from_str("c").unwrap();
        let commit_d = git2::Oid::from_str("d").unwrap();
        let commit_e = git2::Oid::from_str("e").unwrap();
        let commit_f = git2::Oid::from_str("f").unwrap();
        let commit_g = git2::Oid::from_str("0").unwrap();
        let commit_h = git2::Oid::from_str("1").unwrap();

        let original_map = {
            let mut map = HashMap::new();
            map.insert(stack_id_a, {
                let mut inner_map = HashMap::new();
                inner_map.insert(commit_a, {
                    let mut set = HashSet::new();
                    set.insert(commit_b);
                    set.insert(commit_c);
                    set.insert(commit_d);
                    set
                });

                inner_map.insert(commit_b, {
                    let mut set = HashSet::new();
                    set.insert(commit_c);
                    set.insert(commit_d);
                    set
                });

                inner_map.insert(commit_c, {
                    let mut set = HashSet::new();
                    set.insert(commit_d);
                    set
                });

                inner_map
            });
            map.insert(stack_id_b, {
                let mut inner_map = HashMap::new();
                inner_map.insert(commit_e, {
                    let mut set = HashSet::new();
                    set.insert(commit_f);
                    set.insert(commit_g);
                    set.insert(commit_h);
                    set
                });

                inner_map.insert(commit_f, {
                    let mut set = HashSet::new();
                    set.insert(commit_g);
                    set.insert(commit_h);
                    set
                });

                inner_map.insert(commit_g, {
                    let mut set = HashSet::new();
                    set.insert(commit_h);
                    set
                });

                inner_map
            });
            map
        };

        let inverted_map = get_inverted_dependency_maps(&original_map);
        assert_eq!(inverted_map.len(), 2);
        // stack a
        assert!(inverted_map.contains_key(&stack_id_a));
        let stack_a_inverted_map = inverted_map.get(&stack_id_a).unwrap();
        assert_eq!(stack_a_inverted_map.len(), 3);
        // === b
        assert!(stack_a_inverted_map.contains_key(&commit_b));
        let commit_b_deps = stack_a_inverted_map.get(&commit_b).unwrap();
        assert_eq!(commit_b_deps.len(), 1);
        assert!(commit_b_deps.contains(&commit_a));
        // === c
        assert!(stack_a_inverted_map.contains_key(&commit_c));
        let commit_c_deps = stack_a_inverted_map.get(&commit_c).unwrap();
        assert_eq!(commit_c_deps.len(), 2);
        assert!(commit_c_deps.contains(&commit_a));
        assert!(commit_c_deps.contains(&commit_b));
        // === d
        assert!(stack_a_inverted_map.contains_key(&commit_d));
        let commit_d_deps = stack_a_inverted_map.get(&commit_d).unwrap();
        assert_eq!(commit_d_deps.len(), 3);
        assert!(commit_d_deps.contains(&commit_a));
        assert!(commit_d_deps.contains(&commit_b));
        assert!(commit_d_deps.contains(&commit_c));

        // stack b
        assert!(inverted_map.contains_key(&stack_id_b));
        let stack_b_inverted_map = inverted_map.get(&stack_id_b).unwrap();
        assert_eq!(stack_b_inverted_map.len(), 3);
        // === f
        assert!(stack_b_inverted_map.contains_key(&commit_f));
        let commit_f_deps = stack_b_inverted_map.get(&commit_f).unwrap();
        assert_eq!(commit_f_deps.len(), 1);
        assert!(commit_f_deps.contains(&commit_e));

        // === g
        assert!(stack_b_inverted_map.contains_key(&commit_g));
        let commit_g_deps = stack_b_inverted_map.get(&commit_g).unwrap();
        assert_eq!(commit_g_deps.len(), 2);
        assert!(commit_g_deps.contains(&commit_e));
        assert!(commit_g_deps.contains(&commit_f));

        // === h
        assert!(stack_b_inverted_map.contains_key(&commit_h));
        let commit_h_deps = stack_b_inverted_map.get(&commit_h).unwrap();
        assert_eq!(commit_h_deps.len(), 3);
        assert!(commit_h_deps.contains(&commit_e));
        assert!(commit_h_deps.contains(&commit_f));
        assert!(commit_h_deps.contains(&commit_g));
    }

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
                        diffs: vec![InputDiff {
                            change_type: gitbutler_diff::ChangeType::Modified,
                            old_start: 2,
                            old_lines: 1,
                            new_start: 2,
                            new_lines: 1,
                        }],
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
                            parse_diff_from_string(
                                "@@ -6,8 +6,6 @@

6
7
8
-9
-10
11
12
13
",
                                gitbutler_diff::ChangeType::Modified,
                            )?,
                            parse_diff_from_string(
                                "@@ -14,6 +12,7 @@
14
15
16
+17
18
19
20
",
                                gitbutler_diff::ChangeType::Modified,
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

        let dependencies_2 = workspace_ranges.intersection(&path, 10, 1).unwrap();
        assert_eq!(dependencies_2.len(), 1);
        assert_eq!(dependencies_2[0].commit_id, commit2_id);
        assert_eq!(dependencies_2[0].stack_id, stack2_id);

        let dependencies_3 = workspace_ranges.intersection(&path, 15, 1).unwrap();
        assert_eq!(dependencies_3.len(), 1);
        assert_eq!(dependencies_3[0].commit_id, commit2_id);
        assert_eq!(dependencies_3[0].stack_id, stack2_id);

        Ok(())
    }

    #[test]
    fn gracefully_handle_invalid_input_commits() -> anyhow::Result<()> {
        let path = PathBuf::from_str("/test.txt")?;

        let stack_id = StackId::generate();
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        // Invalid input, two subsequent commits with the same changes.
        let workspace_ranges = WorkspaceRanges::create(vec![InputStack {
            stack_id,
            commits: vec![
                InputCommit {
                    commit_id: commit_a_id, // Delete file
                    files: vec![InputFile {
                        path: path.to_owned(),
                        diffs: vec![InputDiff {
                            change_type: gitbutler_diff::ChangeType::Deleted,
                            old_start: 1,
                            old_lines: 2,
                            new_start: 0,
                            new_lines: 0,
                        }],
                    }],
                },
                InputCommit {
                    commit_id: commit_b_id, // Delete file, again
                    files: vec![InputFile {
                        path: path.to_owned(),
                        diffs: vec![InputDiff {
                            change_type: gitbutler_diff::ChangeType::Deleted,
                            old_start: 1,
                            old_lines: 2,
                            new_start: 0,
                            new_lines: 0,
                        }],
                    }],
                },
                InputCommit {
                    commit_id: commit_c_id, // Re-add file
                    files: vec![InputFile {
                        path: path.to_owned(),
                        diffs: vec![InputDiff {
                            change_type: gitbutler_diff::ChangeType::Added,
                            old_start: 0,
                            old_lines: 0,
                            new_start: 1,
                            new_lines: 5,
                        }],
                    }],
                },
            ],
        }])?;

        let dependencies_1 = workspace_ranges.intersection(&path, 2, 1).unwrap();
        assert_eq!(dependencies_1.len(), 1);
        assert_eq!(dependencies_1[0].commit_id, commit_c_id);
        assert_eq!(dependencies_1[0].stack_id, stack_id);

        let errors = &workspace_ranges.errors;
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].commit_id, commit_b_id);
        assert_eq!(errors[0].stack_id, stack_id);
        assert_eq!(errors[0].path, path);
        assert_eq!(
            errors[0].error_message,
            "File recreation must be an addition"
        );

        Ok(())
    }
}
