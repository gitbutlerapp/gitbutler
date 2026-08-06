use std::path::{Path, PathBuf};

use anyhow::Result;
use but_meta::virtual_branches_legacy_types;

use crate::stack::Stack;

/// A handle to the state of virtual branches.
///
/// For all operations, if the state file does not exist, it will be created.
#[deprecated(note = "use ctx.workspace_* helpers instead of VirtualBranchesHandle")]
pub struct VirtualBranchesHandle {
    /// The path to the file containing the virtual branches state.
    file_path: PathBuf,
}

#[expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]
impl VirtualBranchesHandle {
    /// Creates a new concurrency-safe handle to the state of virtual branches.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        let file_path = base_path.as_ref().join("virtual_branches.toml");
        Self { file_path }
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_stack(&mut self, stack: Stack) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.insert(stack.id, stack.into());
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    fn read_file(&self) -> Result<virtual_branches_legacy_types::VirtualBranches> {
        but_meta::legacy_storage::read_synced_virtual_branches(&self.file_path)
    }

    /// Write the given `virtual_branches` back to disk in one go.
    fn write_file(
        &mut self,
        virtual_branches: &virtual_branches_legacy_types::VirtualBranches,
    ) -> Result<()> {
        but_meta::legacy_storage::write_virtual_branches_and_sync(&self.file_path, virtual_branches)
    }
}
