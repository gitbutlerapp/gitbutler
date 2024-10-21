use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gitbutler_stack::StackId;
use itertools::Itertools;

use crate::{HunkRange, InputDiff, PathRanges};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct StackRanges {
    pub paths: HashMap<PathBuf, PathRanges>,
}

impl StackRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        path: &PathBuf,
        diffs: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            deps_path.add(stack_id, commit_id, diffs)?;
        } else {
            let mut path_deps = PathRanges::default();
            path_deps.add(stack_id, commit_id, diffs)?;
            self.paths.insert(path.clone(), path_deps);
        };
        Ok(())
    }

    pub fn unique_paths(&self) -> HashSet<PathBuf> {
        self.paths
            .keys()
            .unique()
            .map(|path| path.to_owned())
            .collect::<HashSet<PathBuf>>()
    }

    pub fn intersection(&mut self, path: &PathBuf, start: u32, lines: u32) -> Vec<&mut HunkRange> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            return deps_path.intersection(start, lines);
        }
        vec![]
    }
}
