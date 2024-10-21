use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gitbutler_stack::StackId;
use itertools::Itertools;

use crate::{diff::InputDiff, hunk::HunkRange, path::PathHunkRanges};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct StackHunkRanges {
    pub paths: HashMap<PathBuf, PathHunkRanges>,
}

impl StackHunkRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        path: &PathBuf,
        diffs: Vec<InputDiff>,
    ) {
        if let Some(deps_path) = self.paths.get_mut(path) {
            deps_path.add(stack_id, commit_id, diffs);
        } else {
            let mut path_deps = PathHunkRanges::default();
            path_deps.add(stack_id, commit_id, diffs);
            self.paths.insert(path.clone(), path_deps);
        };
    }

    pub fn unique_paths(&self) -> HashSet<PathBuf> {
        self.paths
            .keys()
            .unique()
            .map(|path| path.to_owned())
            .collect::<HashSet<PathBuf>>()
    }

    pub fn intersection(&mut self, path: &PathBuf, start: i32, lines: i32) -> Vec<&mut HunkRange> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            return deps_path.find(start, lines);
        }
        vec![]
    }
}
