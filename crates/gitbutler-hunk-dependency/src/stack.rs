use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use gitbutler_stack::StackId;

use crate::{diff::Diff, hunk::DependencyHunk, path::DependencyPath};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct DependencyStack {
    paths: HashMap<PathBuf, DependencyPath>,
}

impl DependencyStack {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        path: &PathBuf,
        diffs: Vec<Diff>,
    ) -> anyhow::Result<()> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            deps_path.add(stack_id, commit_id, diffs)?;
        } else {
            let mut path_deps = DependencyPath::default();
            path_deps.add(stack_id, commit_id, diffs)?;
            self.paths.insert(path.clone(), path_deps);
        };
        Ok(())
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        self.paths.contains_key(path)
    }

    pub fn get_path(&self, path: &Path) -> Option<DependencyPath> {
        self.paths.get(path).cloned()
    }

    pub fn intersection(
        &mut self,
        path: &PathBuf,
        start: i32,
        lines: i32,
    ) -> Vec<&mut DependencyHunk> {
        if let Some(deps_path) = self.paths.get_mut(path) {
            return deps_path.find(start, lines);
        }
        vec![]
    }
}
