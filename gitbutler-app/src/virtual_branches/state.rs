use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::virtual_branches::BranchId;

use super::{target::Target, Branch};

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
    file_path: Arc<Mutex<PathBuf>>,
}

impl VirtualBranchesHandle {
    /// Creates a new concurrency-safe handle to the state of virtual branches.
    pub fn new(base_path: &Path) -> Self {
        let file_path = base_path.join("virtual_branches.toml");
        Self {
            file_path: Arc::new(Mutex::new(file_path)),
        }
    }

    /// Persists the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub async fn set_default_target(&self, target: Target) -> Result<()> {
        let mut virtual_branches = self.read_file().await?;
        virtual_branches.default_target = Some(target);
        self.write_file(virtual_branches).await?;
        Ok(())
    }

    /// Gets the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    #[allow(dead_code)]
    pub async fn get_default_target(&self) -> Result<Option<Target>> {
        let virtual_branches = self.read_file().await?;
        Ok(virtual_branches.default_target)
    }

    /// Sets the target for the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub async fn set_branch_target(&self, id: BranchId, target: Target) -> Result<()> {
        let mut virtual_branches = self.read_file().await?;
        virtual_branches.branch_targets.insert(id, target);
        self.write_file(virtual_branches).await?;
        Ok(())
    }

    /// Gets the target for the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    #[allow(dead_code)]
    pub async fn get_branch_target(&self, id: BranchId) -> Result<Option<Target>> {
        let virtual_branches = self.read_file().await?;
        Ok(virtual_branches.branch_targets.get(&id).cloned())
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub async fn set_branch(&self, branch: Branch) -> Result<()> {
        let mut virtual_branches = self.read_file().await?;
        virtual_branches.branches.insert(branch.id, branch);
        self.write_file(virtual_branches).await?;
        Ok(())
    }

    /// Removes the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    #[allow(dead_code)]
    pub async fn remove_branch(&self, id: BranchId) -> Result<()> {
        let mut virtual_branches = self.read_file().await?;
        virtual_branches.branches.remove(&id);
        self.write_file(virtual_branches).await?;
        Ok(())
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    #[allow(dead_code)]
    pub async fn get_branch(&self, id: BranchId) -> Result<Option<Branch>> {
        let virtual_branches = self.read_file().await?;
        Ok(virtual_branches.branches.get(&id).cloned())
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    async fn read_file(&self) -> Result<VirtualBranches> {
        let file_path = &self.file_path.lock().await;
        if !file_path.exists() {
            write(file_path.as_path(), &VirtualBranches::default())?;
        }
        let mut file: File = File::open(file_path.as_path())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let virtual_branches: VirtualBranches = toml::from_str(&contents)?;
        Ok(virtual_branches)
    }

    async fn write_file(&self, virtual_branches: VirtualBranches) -> Result<()> {
        let file_path = &self.file_path.lock().await;
        write(file_path.as_path(), &virtual_branches)
    }
}

fn write(file_path: &Path, virtual_branches: &VirtualBranches) -> Result<()> {
    let file_path = file_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("bad file path"))?;
    let contents = toml::to_string(&virtual_branches)?;
    let mut file = File::create(file_path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
