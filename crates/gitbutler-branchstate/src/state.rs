use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

use gitbutler_core::{error::Code, fs::read_toml_file_or_default, git};
use gitbutler_project::Project;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use gitbutler_core::virtual_branches::{target::Target, Branch, BranchId};

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    default_target: Option<Target>,
    /// The targets for each virtual branch
    branch_targets: HashMap<BranchId, Target>,
    /// The current state of the virtual branches
    branches: HashMap<BranchId, Branch>,
}

impl VirtualBranches {
    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_all_branches(&self) -> Result<Vec<Branch>> {
        let branches: Vec<Branch> = self.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches_in_workspace(&self) -> Result<Vec<Branch>> {
        self.list_all_branches().map(|branches| {
            branches
                .into_iter()
                .filter(|branch| branch.in_workspace)
                .collect()
        })
    }
}

/// A handle to the state of virtual branches.
///
/// For all operations, if the state file does not exist, it will be created.
pub struct VirtualBranchesHandle {
    /// The path to the file containing the virtual branches state.
    file_path: PathBuf,
}

pub trait VirtualBranchesAccess {
    fn virtual_branches(&self) -> VirtualBranchesHandle;
}

impl VirtualBranchesAccess for Project {
    fn virtual_branches(&self) -> VirtualBranchesHandle {
        VirtualBranchesHandle::new(self.gb_dir())
    }
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

    /// Marks a particular branch as not in the workspace
    ///
    /// Errors if the file cannot be read or written.
    pub fn mark_as_not_in_workspace(&self, id: BranchId) -> Result<()> {
        let mut branch = self.get_branch(id)?;
        branch.in_workspace = false;
        branch.old_applied = false;
        self.set_branch(branch)?;
        Ok(())
    }

    pub fn find_by_source_refname_where_not_in_workspace(
        &self,
        refname: &git::Refname,
    ) -> Result<Option<Branch>> {
        self.list_all_branches().map(|branches| {
            branches.into_iter().find(|branch| {
                if branch.in_workspace {
                    return false;
                }

                if let Some(source_refname) = branch.source_refname.clone() {
                    return source_refname.to_string() == refname.to_string();
                }

                false
            })
        })
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch_in_workspace(&self, id: BranchId) -> Result<Branch> {
        self.try_branch_in_workspace(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
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
    pub fn try_branch_in_workspace(&self, id: BranchId) -> Result<Option<Branch>> {
        if let Some(branch) = self.try_branch(id)? {
            if branch.in_workspace {
                Ok(Some(branch))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_branch(&self, id: BranchId) -> Result<Option<Branch>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.branches.get(&id).cloned())
    }

    /// Lists all branches in vbranches.toml
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_all_branches(&self) -> Result<Vec<Branch>> {
        let virtual_branches = self.read_file()?;
        let branches: Vec<Branch> = virtual_branches.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches_in_workspace(&self) -> Result<Vec<Branch>> {
        self.list_all_branches().map(|branches| {
            branches
                .into_iter()
                .filter(|branch| branch.in_workspace)
                .collect()
        })
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

    pub fn update_ordering(&self) -> Result<()> {
        let succeeded = self
            .list_branches_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .enumerate()
            .all(|(index, branch)| {
                let mut branch = branch.clone();
                branch.order = index;
                self.set_branch(branch).is_ok()
            });

        if succeeded {
            Ok(())
        } else {
            Err(anyhow!("Failed to update virtual branches ordering"))
        }
    }

    pub fn next_order_index(&self) -> Result<usize> {
        self.update_ordering()?;
        let order = self
            .list_branches_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .collect::<Vec<&Branch>>()
            .last()
            .map_or(0, |b| b.order + 1);

        Ok(order)
    }
}

fn write<P: AsRef<Path>>(file_path: P, virtual_branches: &VirtualBranches) -> Result<()> {
    gitbutler_core::fs::write(file_path, toml::to_string(&virtual_branches)?)
}
