use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{error::Code, fs::read_toml_file_or_default};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::{target::Target, Branch};
use crate::virtual_branches::BranchId;

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    pub default_target: Option<Target>,
    /// The targets for each virtual branch
    pub branch_targets: HashMap<BranchId, Target>,
    /// The current state of the virtual branches
    pub branches: HashMap<BranchId, Branch>,
}
/// A handle to the state of virtual branches.
///
/// For all operations, if the state file does not exist, it will be created.
pub struct VirtualBranchesHandle {
    /// The path to the file containing the virtual branches state.
    file_path: PathBuf,
}

impl VirtualBranchesHandle {
    /// Creates a new concurrency-safe handle to the state of virtual branches.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        let file_path = base_path.as_ref().join("virtual_branches.toml");
        Self { file_path }
    }

    /// Persists the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_default_target(&self, target: Target) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.default_target = Some(target);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_default_target(&self) -> Result<Target> {
        let virtual_branches = self.read_file();
        virtual_branches?
            .default_target
            .ok_or(anyhow!("there is no default target").context(Code::DefaultTargetNotFound))
    }

    /// Sets the target for the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_branch_target(&self, id: BranchId, target: Target) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branch_targets.insert(id, target);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_branch(&self, branch: Branch) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.insert(branch.id, branch);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Removes the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn remove_branch(&self, id: BranchId) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.remove(&id);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch(&self, id: BranchId) -> Result<Branch> {
        self.try_branch(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_branch(&self, id: BranchId) -> Result<Option<Branch>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.branches.get(&id).cloned())
    }

    /// Lists all virtual branches.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches(&self) -> Result<Vec<Branch>> {
        let virtual_branches = self.read_file()?;
        let branches: Vec<Branch> = virtual_branches.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Checks if the state file exists.
    ///
    /// This would only be false if the application just updated from a very old verion.
    pub fn file_exists(&self) -> bool {
        self.file_path.exists()
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<VirtualBranches> {
        read_toml_file_or_default(&self.file_path)
    }

    fn write_file(&self, virtual_branches: &VirtualBranches) -> Result<()> {
        write(self.file_path.as_path(), virtual_branches)
    }
}

fn write<P: AsRef<Path>>(file_path: P, virtual_branches: &VirtualBranches) -> Result<()> {
    crate::fs::write(file_path, toml::to_string(&virtual_branches)?)
}
