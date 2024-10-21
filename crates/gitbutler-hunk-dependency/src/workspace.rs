use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use gitbutler_stack::StackId;

use crate::{diff::InputDiff, hunk::HunkRange, stack::StackHunkRanges};

type BranchStatus = HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>;

pub struct HunkDependencyOptions<'a> {
    pub workdir: &'a BranchStatus,
    pub stacks: Vec<InputStack>,
}

pub struct InputFile {
    pub path: PathBuf,
    pub diffs: Vec<InputDiff>,
}

pub struct InputStack {
    pub stack_id: StackId,
    pub commits: Vec<InputCommit>,
}

pub struct InputCommit {
    pub commit_id: git2::Oid,
    pub files: Vec<InputFile>,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct WorkspaceHunkRanges {
    paths: HashMap<PathBuf, Vec<HunkRange>>,
}

/// Provides blame-like functionality for looking up what commit(s) have
/// touched a specific line number range for a given path.
///
/// First it combines changes per branch sequentially by commit, allowing
/// for dependent changes where one commit introduces changes that overwrites
/// previous changes.
///
/// It then combines the changes per branch into a single vector with line
/// numbers that should match the workspace commit. These per branch changes
/// are both assumed, and required, to be independent without overlap.
impl WorkspaceHunkRanges {
    pub fn new(input_stacks: Vec<InputStack>) -> WorkspaceHunkRanges {
        let mut stacks = vec![];
        for input_stack in input_stacks {
            let mut stack = StackHunkRanges::default();
            let InputStack { stack_id, commits } = input_stack;
            for commit in commits {
                let InputCommit { commit_id, files } = commit;
                for file in files {
                    stack.add(stack_id, commit_id, &file.path, file.diffs);
                }
            }
            stacks.push(stack);
        }
        let paths = stacks
            .iter()
            .flat_map(|stack| stack.unique_paths())
            .collect::<HashSet<_>>();

        WorkspaceHunkRanges {
            paths: HashMap::from_iter(
                paths
                    .iter()
                    .map(|path| (path.clone(), combine_path_ranges(path, &stacks))),
            ),
        }
    }

    /// Finds commits that intersect with a given path and range combination.
    pub fn intersection(&self, path: &Path, start: i32, lines: i32) -> Vec<HunkRange> {
        if let Some(stack_hunks) = self.paths.get(path) {
            return stack_hunks
                .iter()
                .filter(|hunk| hunk.intersects(start, lines))
                .map(|x| x.to_owned())
                .collect::<Vec<_>>();
        }
        vec![]
    }
}

fn combine_path_ranges(path: &Path, stacks: &[StackHunkRanges]) -> Vec<HunkRange> {
    // Only process stacks that contain the path.
    let filtered_stacks = stacks
        .iter()
        .filter_map(|stack| stack.paths.get(path))
        .collect::<Vec<_>>();

    // Tracks the cumulative lines added/removed.
    let mut line_shift = 0;

    // Next hunk to consider for each branch containing path.
    let mut hunk_indexes: Vec<usize> = vec![0; filtered_stacks.len()];

    let mut result: Vec<HunkRange> = vec![];

    loop {
        let start_lines = filtered_stacks
            .iter()
            .enumerate()
            .map(|(i, path_dep)| path_dep.hunks.get(hunk_indexes[i]))
            .map(|hunk| hunk.map(|hunk_dep| hunk_dep.start as u32))
            .collect::<Vec<_>>();

        // Find the index of the dependency path with the lowest start line.
        let next_index = start_lines
            .iter()
            .enumerate() // We want to filter out None values, but keep their index.
            .filter(|(_, start_line)| start_line.is_some())
            .min_by_key(|&(index, &value)| value.unwrap() + start_lines[index].unwrap_or(0))
            .map(|(index, _)| index);

        if next_index.is_none() {
            break; // No more items to process.
        }
        let path_index = next_index.unwrap();

        let hunk_index = hunk_indexes[path_index];
        hunk_indexes[path_index] += 1;

        let path_dep = &filtered_stacks[path_index];
        let hunk_dep = &path_dep.hunks[hunk_index];

        result.push(HunkRange {
            start: hunk_dep.start + line_shift,
            ..hunk_dep.clone()
        });
        line_shift += hunk_dep.line_shift;
    }
    result
}
