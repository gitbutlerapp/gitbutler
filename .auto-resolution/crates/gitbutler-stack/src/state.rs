use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use git2::Repository;
use gitbutler_error::error::Code;
use gitbutler_fs::read_toml_file_or_default;
use gitbutler_oxidize::OidExt as _;
use gitbutler_reference::Refname;
use gitbutler_repo::commit_message::CommitMessage;
use gitbutler_serde::object_id_opt;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    stack::{Stack, StackId},
    target::Target,
};

const LAST_PUSHED_BASE_VERSION_HEADER: &str = "base-commit-version";
const LAST_PUSHED_BASE_VERSION: &str = "1";

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    pub default_target: Option<Target>,
    /// The targets for each virtual branch
    branch_targets: HashMap<StackId, Target>,
    /// The current state of the virtual branches
    pub branches: HashMap<StackId, Stack>,

    #[serde(with = "object_id_opt", default)]
    last_pushed_base: Option<gix::ObjectId>,
}

impl VirtualBranches {
    /// Lists all virtual branches that are in the user's workspace.
    ///
    /// Errors if the file cannot be read or written.
    pub(crate) fn list_all_stacks(&self) -> Result<Vec<Stack>> {
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
        let virtual_branches = self.read_file()?;
        virtual_branches
            .default_target
            .ok_or(anyhow!("there is no default target").context(Code::DefaultTargetNotFound))
    }

    /// Gets the default target for the given repository.
    ///
    /// Errors if the file cannot be read or written.
    pub fn maybe_get_default_target(&self) -> Result<Option<Target>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.default_target)
    }

    /// Sets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn set_stack(&self, stack: Stack) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.insert(stack.id, stack);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Marks a particular branch as not in the workspace
    ///
    /// Errors if the file cannot be read or written.
    pub fn mark_as_not_in_workspace(&self, id: StackId) -> Result<()> {
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

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_stack_in_workspace(&self, id: StackId) -> Result<Stack> {
        self.try_stack_in_workspace(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
    }

    /// Gets the state of the given virtual branch.
    ///
    /// Errors if the file cannot be read or written.
    pub fn get_stack(&self, id: StackId) -> Result<Stack> {
        self.try_stack(id)?
            .ok_or_else(|| anyhow!("branch with ID {id} not found"))
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
        read_toml_file_or_default(&self.file_path)
    }

    /// Write the given `virtual_branches` back to disk in one go.
    pub fn write_file(&self, virtual_branches: &VirtualBranches) -> Result<()> {
        write(self.file_path.as_path(), virtual_branches)
    }

    pub fn update_ordering(&self) -> Result<()> {
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

    pub fn next_order_index(&self) -> Result<usize> {
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

    pub fn delete_branch_entry(&self, branch_id: &StackId) -> Result<()> {
        let mut virtual_branches = self.read_file()?;
        virtual_branches.branches.remove(branch_id);
        self.write_file(&virtual_branches)?;
        Ok(())
    }

    /// Garbage collects branches that are not in the workspace and hold no changes:
    ///   1. They do not have a WIP commit
    ///   2. They have no regular commits
    ///
    /// Also collects branches with a head oid pointing to a commit that can't be found in the repo
    pub fn garbage_collect(&self, repo: &Repository) -> Result<()> {
        let target = self.get_default_target()?;
        let stacks_not_in_workspace = self
            .list_all_stacks()?
            .into_iter()
            .filter(|b| !b.in_workspace)
            .collect_vec();
        let mut to_remove: Vec<StackId> = vec![];
        for branch in stacks_not_in_workspace {
            if branch.not_in_workspace_wip_change_id.is_some() {
                continue; // Skip branches that have a WIP commit
            }
            if repo.find_commit(branch.head()).is_err() {
                // if the head commit cant be found, we can GC the branch
                to_remove.push(branch.id);
            } else {
                // if there are no commits between the head and the merge base,
                // i.e. the head is the merge base, we can GC the branch
                if branch.head() == repo.merge_base(branch.head(), target.sha)? {
                    to_remove.push(branch.id);
                }
            }
        }
        if !to_remove.is_empty() {
            let mut virtual_branches = self.read_file()?;
            for branch_id in to_remove {
                virtual_branches.branches.remove(&branch_id);
            }
            // Perform all removals in one go (Windows doesn't like multiple writes in quick succession)
            self.write_file(&virtual_branches)?;
        }

        Ok(())
    }

    /// Returns a base commit for use when pushing a stack for review.
    /// The last pushed base either has no parents, or either has the base
    /// that was pushed previously as the base.
    ///
    /// The returned commit will always have the same tree as
    /// `default_target.sha`.
    ///
    /// This function will return `Ok(None)` if there is no default target.
    pub fn upsert_last_pushed_base(
        &self,
        repository: &gix::Repository,
    ) -> Result<Option<gix::ObjectId>> {
        let mut virtual_branches = self.read_file()?;
        let Some(default_target) = &virtual_branches.default_target else {
            return Ok(None);
        };

        let base_tree_id = repository
            .find_commit(default_target.sha.to_gix())?
            .tree_id()?
            .detach();

        if let Some(last_pushed_base) = virtual_branches.last_pushed_base {
            let last_pushed_tree = repository
                .find_commit(last_pushed_base)?
                .tree_id()?
                .detach();

            let up_to_date = repository
                .find_commit(last_pushed_base)?
                .decode()?
                .extra_headers()
                .find(LAST_PUSHED_BASE_VERSION_HEADER)
                .unwrap_or("unversioned".into())
                == LAST_PUSHED_BASE_VERSION;

            // If the base commit's tree is the same as the previously pushed
            // one, we have no need to update it.
            if base_tree_id == last_pushed_tree && up_to_date {
                return Ok(Some(last_pushed_base));
            }

            virtual_branches.last_pushed_base = Some(alter_parentage(
                repository,
                default_target.sha.to_gix(),
                &[last_pushed_base],
            )?);
        } else {
            // There was no previous last_pushed_base to point to, so we create
            // the first base which doesn't have any parents.
            virtual_branches.last_pushed_base = Some(alter_parentage(
                repository,
                default_target.sha.to_gix(),
                &[],
            )?);
        }

        self.write_file(&virtual_branches)?;

        Ok(virtual_branches.last_pushed_base)
    }

    /// Provides direct access to the last_pushed_base. If you are actually
    /// pushing, you probably want
    pub fn last_pushed_base(&self) -> Result<Option<gix::ObjectId>> {
        let virtual_branches = self.read_file()?;
        Ok(virtual_branches.last_pushed_base)
    }
}

fn write<P: AsRef<Path>>(file_path: P, virtual_branches: &VirtualBranches) -> Result<()> {
    gitbutler_fs::write(file_path, toml::to_string(&virtual_branches)?)
}

/// Re-commit a commit with altered parentage
fn alter_parentage(
    repository: &gix::Repository,
    to_rewrite: gix::ObjectId,
    new_parents: &[gix::ObjectId],
) -> Result<gix::ObjectId> {
    let decoded = repository.find_commit(to_rewrite)?;
    let decoded = decoded.decode()?;
    let mut message = CommitMessage::new(decoded.clone());
    message
        .trailers
        .push(("Base-Commit".into(), to_rewrite.to_hex().to_string().into()));
    let mut to_rewrite: gix::objs::Commit = decoded.into();
    to_rewrite.parents = new_parents.into();
    to_rewrite.message = message.to_bstring();
    to_rewrite.extra_headers.retain(|entry| entry.0 != "gpgsig");
    to_rewrite.extra_headers.push((
        LAST_PUSHED_BASE_VERSION_HEADER.into(),
        LAST_PUSHED_BASE_VERSION.into(),
    ));
    Ok(repository.write_object(to_rewrite)?.into())
}

/// Additional functionality for the [`VirtualBranches`] structure.
mod state_extensions {}
