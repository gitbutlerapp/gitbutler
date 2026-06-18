use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use but_error::Code;
use but_meta::virtual_branches_legacy_types;
use gitbutler_reference::Refname;
use itertools::Itertools;

use crate::{
    stack::{Stack, StackId},
    target::Target,
};

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    pub default_target: Option<Target>,
    /// The targets for each virtual branch
    branch_targets: HashMap<StackId, Target>,
    /// The current state of the virtual branches
    pub branches: HashMap<StackId, Stack>,

    last_pushed_base: Option<gix::ObjectId>,
}

impl From<virtual_branches_legacy_types::VirtualBranches> for VirtualBranches {
    fn from(
        virtual_branches_legacy_types::VirtualBranches {
            default_target,
            branch_targets,
            branches,
            last_pushed_base,
        }: virtual_branches_legacy_types::VirtualBranches,
    ) -> Self {
        VirtualBranches {
            default_target: default_target.map(Into::into),
            branch_targets: branch_targets
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            branches: branches.into_iter().map(|(k, v)| (k, v.into())).collect(),
            last_pushed_base,
        }
    }
}

impl From<VirtualBranches> for virtual_branches_legacy_types::VirtualBranches {
    fn from(
        VirtualBranches {
            default_target,
            branch_targets,
            branches,
            last_pushed_base,
        }: VirtualBranches,
    ) -> Self {
        virtual_branches_legacy_types::VirtualBranches {
            default_target: default_target.map(Into::into),
            branch_targets: branch_targets
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            branches: branches.into_iter().map(|(k, v)| (k, v.into())).collect(),
            last_pushed_base,
        }
    }
}

impl VirtualBranches {
    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_all_stacks(&self) -> Result<Vec<Stack>> {
        let branches: Vec<Stack> = self.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_stacks_in_workspace(&self) -> Result<Vec<Stack>> {
        self.list_all_stacks().map(|stacks| {
            stacks
                .into_iter()
                .filter(|stack| stack.in_workspace)
                .collect()
        })
    }
}

/// A handle to the state of virtual branches.
///
/// For all operations, if the state file does not exist, it will be created.
#[deprecated(note = "use ctx.workspace_* helpers instead of VirtualBranchesHandle")]
pub struct VirtualBranchesHandle {
    /// The path to the file containing the virtual branches state.
    file_path: PathBuf,
}

// pub trait VirtualBranchesExt {
//     fn virtual_branches(&self) -> VirtualBranchesHandle;
// }

// impl VirtualBranchesExt for Project {
//     fn virtual_branches(&self) -> VirtualBranchesHandle {
//         VirtualBranchesHandle::new(self.gb_dir())
//     }
// }

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
        virtual_branches.branches.insert(stack.id, stack);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Marks a particular branch as not in the workspace
    ///
    /// Errors if the file cannot be read or written.
    pub fn mark_as_not_in_workspace(&mut self, id: StackId) -> Result<()> {
        let mut stack = self.get_stack(id)?;
        stack.in_workspace = false;
        self.set_stack(stack)?;
        Ok(())
    }

    pub fn find_by_source_refname_where_not_in_workspace(
        &self,
        refname: &Refname,
    ) -> Result<Option<Stack>> {
        let stacks = self.list_all_stacks()?;
        Ok(stacks.into_iter().find(|branch| {
            if branch.in_workspace {
                return false;
            }

            if let Some(source_refname) = branch.source_refname.as_ref() {
                return source_refname.to_string() == refname.to_string();
            }

            false
        }))
    }

    pub fn find_by_top_reference_name_where_not_in_workspace(
        &self,
        refname: &str,
    ) -> Result<Option<Stack>> {
        let stacks = self.list_all_stacks()?;
        Ok(stacks.into_iter().find(|stack| {
            if stack.in_workspace {
                return false;
            }

            if let Some(head_branch) = stack.heads.last() {
                if let Ok(full_name) = head_branch.full_name() {
                    return full_name.to_string() == refname;
                } else {
                    return false;
                }
            }

            false
        }))
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_stack_in_workspace(&self, id: StackId) -> Result<Stack> {
        self.try_stack_in_workspace(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found").context(Code::BranchNotFound))
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_stack(&self, id: StackId) -> Result<Stack> {
        self.try_stack(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found").context(Code::BranchNotFound))
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_stack_in_workspace(&self, id: StackId) -> Result<Option<Stack>> {
        Ok(self.try_stack(id)?.filter(|branch| branch.in_workspace))
    }

    /// Gets the state of the given virtual branch returning `Some(branch)` or `None`
    /// if that branch doesn't exist.
    pub fn try_stack(&self, id: StackId) -> Result<Option<Stack>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.branches.get(&id).cloned())
    }

    /// Lists all branches in `virtual_branches.toml`.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_all_stacks(&self) -> Result<Vec<Stack>> {
        let virtual_branches = self.read_file()?;
        let branches: Vec<Stack> = virtual_branches.branches.values().cloned().collect();
        Ok(branches)
    }

    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub fn list_stacks_in_workspace(&self) -> Result<Vec<Stack>> {
        self.list_all_stacks().map(|branches| {
            branches
                .into_iter()
                .filter(|branch| branch.in_workspace)
                .collect()
        })
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    pub fn read_file(&self) -> Result<VirtualBranches> {
        let data = self.ensure_vb_storage_in_sync()?;
        Ok(data.into())
    }

    /// Write the given `virtual_branches` back to disk in one go.
    pub fn write_file(&mut self, virtual_branches: &VirtualBranches) -> Result<()> {
        let _ = self.ensure_vb_storage_in_sync()?;
        let legacy = virtual_branches_legacy_types::VirtualBranches::from(virtual_branches.clone());
        but_meta::legacy_storage::write_virtual_branches_and_sync(&self.file_path, &legacy)
    }

    /// Ensure TOML and DB are synchronized before proceeding with metadata operations.
    fn ensure_vb_storage_in_sync(&self) -> Result<virtual_branches_legacy_types::VirtualBranches> {
        but_meta::legacy_storage::read_synced_virtual_branches(&self.file_path)
    }

    /// Import TOML into DB and refresh sync metadata.
    ///
    /// This is primarily used for oplog restore, where TOML was restored externally.
    pub fn import_toml_into_db_for_restore(&mut self) -> Result<()> {
        but_meta::legacy_storage::import_toml_into_db(&self.file_path)
    }

    pub fn update_ordering(&mut self) -> Result<()> {
        let succeeded = self
            .list_stacks_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .enumerate()
            .all(|(index, branch)| {
                let mut branch = branch.clone();
                branch.order = index;
                self.set_stack(branch).is_ok()
            });

        if succeeded {
            Ok(())
        } else {
            Err(anyhow!("Failed to update virtual branches ordering"))
        }
    }

    pub fn next_order_index(&mut self) -> Result<usize> {
        self.update_ordering()?;
        let order = self
            .list_stacks_in_workspace()?
            .iter()
            .sorted_by_key(|branch| branch.order)
            .collect::<Vec<&Stack>>()
            .last()
            .map_or(0, |b| b.order + 1);

        Ok(order)
    }

    pub fn delete_branch_entry(&mut self, branch_id: &StackId) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.remove(branch_id);
        self.write_file(&virtual_branches)?;
        Ok(())
    }
}

/// Additional functionality for the [`VirtualBranches`] structure.
mod state_extensions {}
