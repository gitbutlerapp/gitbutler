use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
    pub fn new(base_path: &Path) -> Self {
        let file_path = base_path.join("virtual_branches.toml");
        Self { file_path }
    }

    /// Persists the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_default_target(&self, target: Target) -> anyhow::Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.default_target = Some(target);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_default_target(&self) -> Result<Target, crate::reader::Error> {
        let virtual_branches = self.read_file();
        virtual_branches?
            .default_target
            .ok_or(crate::reader::Error::NotFound)
    }

    /// Sets the target for the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_branch_target(&self, id: BranchId, target: Target) -> anyhow::Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branch_targets.insert(id, target);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the target for the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch_target(&self, id: &BranchId) -> Result<Target, crate::reader::Error> {
        let virtual_branches = self.read_file()?;
        let tartget = virtual_branches.branch_targets.get(id).cloned();
        match tartget {
            Some(target) => Ok(target),
            None => self.get_default_target(),
        }
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_branch(&self, branch: Branch) -> anyhow::Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.insert(branch.id, branch);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Removes the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn remove_branch(&self, id: BranchId) -> anyhow::Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.remove(&id);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_branch(&self, id: &BranchId) -> Result<Branch, crate::reader::Error> {
        let virtual_branches = self.read_file()?;
        virtual_branches
            .branches
            .get(id)
            .cloned()
            .ok_or(crate::reader::Error::NotFound)
    }

    /// Lists all virtual branches.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_branches(&self) -> anyhow::Result<Vec<Branch>> {
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
    fn read_file(&self) -> Result<VirtualBranches, crate::reader::Error> {
        // let file_path = &self.file_path.lock().await;
        if !self.file_path.exists() {
            return Ok(VirtualBranches::default());
        }
        let mut file: File = File::open(self.file_path.as_path())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let virtual_branches: VirtualBranches =
            toml::from_str(&contents).map_err(|e| crate::reader::Error::ParseError {
                path: self.file_path.clone(),
                source: e,
            })?;
        Ok(virtual_branches)
    }

    fn write_file(&self, virtual_branches: &VirtualBranches) -> anyhow::Result<()> {
        write(self.file_path.as_path(), virtual_branches)
    }
}

fn write<P: AsRef<Path>>(file_path: P, virtual_branches: &VirtualBranches) -> anyhow::Result<()> {
    let contents = toml::to_string(&virtual_branches)?;
    let temp_file = tempfile::NamedTempFile::new_in(file_path.as_ref().parent().unwrap())?;
    let (mut file, temp_path) = temp_file.keep()?;
    file.write_all(contents.as_bytes())?;
    drop(file);
    std::fs::rename(temp_path, file_path.as_ref())?;
    Ok(())
}
