use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gitbutler_stack::StackId;
use itertools::Itertools;

use crate::{HunkRange, InputDiff, PathRanges};

#[derive(Debug, Default)]
pub struct StackRanges {
    pub stack_id: Option<StackId>,
    pub paths: HashMap<PathBuf, PathRanges>,
}

/// A struct for collecting hunk ranges by path, before they get merged into a single dimension
/// representing the workspace view.
impl StackRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        path: &PathBuf,
        diffs: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        self.paths
            .entry(path.to_owned())
            .or_default()
            .add(stack_id, commit_id, diffs)?;

        Ok(())
    }

    pub fn unique_paths(&self) -> HashSet<PathBuf> {
        self.paths
            .keys()
            .unique()
            .map(|path| path.to_owned())
            .collect::<HashSet<PathBuf>>()
    }

    pub fn intersection(&mut self, path: &PathBuf, start: u32, lines: u32) -> Vec<&HunkRange> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            return deps_path.intersection(start, lines);
        }
        vec![]
    }

    /// Merge all the commit dependencies for each path into a single, global commit dependency map
    pub fn get_commit_dependencies(&self) -> HashMap<git2::Oid, HashSet<git2::Oid>> {
        self.paths
            .values()
            .flat_map(|path_ranges| path_ranges.commit_dependencies.iter())
            .fold(HashMap::new(), |mut acc, (commit_id, dependencies)| {
                acc.entry(*commit_id)
                    .and_modify(|existing_dependencies| existing_dependencies.extend(dependencies))
                    .or_insert(dependencies.clone());
                acc
            })
    }
}
