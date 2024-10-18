use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use gitbutler_stack::StackId;

use crate::{diff::Diff, hunk::DependencyHunk, stack::DependencyStack};

/// Calculates dependencies between workspace changes and workspace commits.
///
/// What we ultimately want to understand is, given an uncommitted change
/// in some file, do the old line numbers intersect with any commmit(s) in
/// the workspace?
///
/// The problem we have to overcome is that we the workspace changes are
/// produced by diffing the working directory against the workspace commit.
/// It means changes from one stack can offset line numbers in changes from
/// a different stack. The most intuitive way of checking if they touch
/// the same lines is to use regular git blame, but it suffers from two
/// problems, 1) speed and 2) lack of --reverse flag in git2. The latter
/// means we can't detect intersections with deleted lines.
///
/// If we don't calculate these dependencies correctly it means a user
/// might be able to move a hunk into a stack where it cannot be committed.
///
/// So the solution here is that we build up the same information we would
/// get from blame by adding diffs together.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct HunkDependencyBuilder {
    stacks: HashMap<StackId, DependencyStack>,
}

impl HunkDependencyBuilder {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        path: &PathBuf,
        diffs: Vec<Diff>,
    ) -> anyhow::Result<()> {
        if let Some(lane) = self.stacks.get_mut(&stack_id) {
            lane.add(stack_id, commit_id, path, diffs)?;
        } else {
            let mut lane_deps = DependencyStack::default();
            lane_deps.add(stack_id, commit_id, path, diffs)?;
            self.stacks.insert(stack_id, lane_deps);
        }
        Ok(())
    }

    /// Gets an object that can be used to lookup dependencies for a given path.
    ///
    /// The reasoning for combining the stacks/lanes here, rather than including
    /// it where diffs are combined within the branch, is/was to keep the logic
    /// simple. In iterating on the code, however, it feels like it might make
    /// more sense to go directly to "global" line numbers.
    ///
    /// The constraint we would need to introduce is that diffs from different
    /// stacks cannot intersect with each other. Doing so would mean the workspace
    /// is corrupt.
    ///
    /// TODO: Consider moving most of the code below to path.rs
    pub fn get_path(&mut self, path: &Path) -> anyhow::Result<PathDependencyLookup> {
        let paths = self
            .stacks
            .values()
            .filter(|s| s.contains_path(path))
            .filter_map(|value| value.get_path(path))
            .collect::<Vec<_>>();
        // Tracks the cumulative lines added/removed.
        let mut line_shift = 0;
        // Next hunk to consider for each branch containing path.
        let mut hunk_indexes: Vec<usize> = vec![0; paths.len()];
        let mut result = vec![];

        loop {
            let start_lines = paths
                .iter()
                .enumerate()
                .map(|(i, path_dep)| path_dep.hunks.get(hunk_indexes[i]))
                .map(|hunk_dep| hunk_dep.map(|hunk_dep| hunk_dep.start as u32))
                .collect::<Vec<_>>();

            // Find the index of the dependency path with the lowest start line.
            let path_index = start_lines
                .iter()
                .enumerate() // We want to filter out None values, but keep their index.
                .filter(|(_, start_line)| start_line.is_some())
                .min_by_key(|&(index, &value)| value.unwrap() + start_lines[index].unwrap_or(0))
                .map(|(index, _)| index);

            if path_index.is_none() {
                break; // No more items to process.
            }
            let path_index = path_index.unwrap();
            let hunk_index = hunk_indexes[path_index];
            hunk_indexes[path_index] += 1;

            let path_dep = &paths[path_index];
            let hunk_dep = &path_dep.hunks[hunk_index];

            result.push(DependencyHunk {
                start: hunk_dep.start + line_shift,
                ..hunk_dep.clone()
            });
            line_shift += hunk_dep.line_shift;
        }
        Ok(PathDependencyLookup { hunk_deps: result })
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PathDependencyLookup {
    hunk_deps: Vec<DependencyHunk>,
}

impl PathDependencyLookup {
    pub fn find(self, start: i32, lines: i32) -> Vec<DependencyHunk> {
        self.hunk_deps
            .into_iter()
            .filter(|hunk| hunk.intersects(start, lines))
            .collect::<Vec<_>>()
    }
}
