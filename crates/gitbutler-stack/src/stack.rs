use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use gitbutler_branch::Branch;
use gitbutler_branch::Target;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_reference::normalize_branch_name;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::LogUntil;
use gitbutler_repo::RepoActionsExt;
use gitbutler_repo::RepositoryExt;
use gix::validate::reference::name_partial;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::series::Series;

/// A (series) Stack represents multiple "branches" that are dependent on each other in series.
///
/// An initialized Stack must:
/// - have at least one head (branch)
/// - include only referecences that are part of the stack
/// - always have it's commits under a reference i.e. no orphaned commits
pub trait Stack {
    /// An initialized stack has at least one head (branch).
    fn initialized(&self) -> bool;

    /// Initializes a new stack.
    /// An initialized stack means that the heads will always have at least one entry.
    /// When initialized, first stack head will point to the "Branch" head.
    /// Errors out if the stack has already been initialized.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn init(&mut self, ctx: &CommandContext) -> Result<()>;

    /// Adds a new "Branch" to the Stack.
    /// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
    /// The name cannot be the same as existing git references or existing patch references.
    /// The target must reference a commit (or change) that is part of the stack.
    /// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn add_branch(&mut self, ctx: &CommandContext, head: PatchReference) -> Result<()>;

    /// Removes a branch from the Stack.
    /// The very last branch (reference) cannot be removed (A Stack must always contains at least one reference)
    /// If there were commits/changes that were *only* referenced by the removed branch,
    /// those commits are moved to the branch underneath it (or more accurately, the precee)
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn remove_branch(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()>;

    /// Updates an existing branch in the stack.
    /// The same invariants as `add_branch` apply.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn update_branch(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: PatchReferenceUpdate,
    ) -> Result<()>;

    /// Pushes the reference (branch) to the Stack remote as derived from the default target.
    /// This operation will error out if the target has no push remote configured.
    fn push_branch(
        &self,
        ctx: &CommandContext,
        branch_name: String,
        with_force: bool,
    ) -> Result<()>;

    /// Returns a list of all branches/series in the stack.
    /// This operation will compute the current list of local and remote commits that belong to each series.
    /// The first entry is the newest in the Stack (i.e the top of the stack).
    fn list_branches(&self, ctx: &CommandContext) -> Result<Vec<Series>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PatchReferenceUpdate {
    pub target: Option<CommitOrChangeId>,
    pub name: Option<String>,
}

/// A Stack implementation for gitbutler_branch::Branch
/// This operates via a list of PatchReferences (heads) that are an attribute of gitbutler_branch::Branch.
/// In this context a (virtual) "Branch" is a stack of PatchReferences, each pointing to a commit (or change) within the stack.
///
/// This trait provides a defined interface for interacting with a Stack, the field `heads` on `Branch` should never be modified directly
/// outside of the trait implementation.
impl Stack for Branch {
    fn initialized(&self) -> bool {
        !self.heads.is_empty()
    }
    fn init(&mut self, ctx: &CommandContext) -> Result<()> {
        if self.initialized() {
            return Err(anyhow!("Stack already initialized"));
        }
        let reference = PatchReference {
            target: CommitOrChangeId::CommitId(self.head.to_string()),
            name: normalize_branch_name(&self.name)?,
        };
        let state = branch_state(ctx);
        validate_name(&reference, ctx, &state)?;
        self.heads = vec![reference];
        state.set_branch(self.clone())
    }

    fn add_branch(&mut self, ctx: &CommandContext, head: PatchReference) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let state = branch_state(ctx);
        validate_name(&head, ctx, &state)?;
        validate_target(&head, ctx, self.head, &state)?;
        self.heads.push(head);
        state.set_branch(self.clone())
    }

    fn remove_branch(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        // find the head that corresponds to the supplied name, together with its index
        let (idx, head) = get_branch(self, &branch_name)?;
        if self.heads.len() == 1 {
            return Err(anyhow!("Cannot remove the last branch from the stack"));
        }
        // The branch that is being removed is the top (last) one.
        // This means that if there are commits, they need to be moved to the branch underneath.
        if self.heads.len() - 1 == idx {
            // Getting the preceeding head  and setting it's target to the target of the head being removed
            let prior_head = self
                .heads
                .get_mut(idx - 1)
                .ok_or_else(|| anyhow!("Cannot get the head before the head being removed"))?;
            prior_head.target = head.target.clone();
        }
        self.heads.remove(idx);
        let state = branch_state(ctx);
        state.set_branch(self.clone())
    }

    fn update_branch(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: PatchReferenceUpdate,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let (idx, head) = get_branch(self, &branch_name)?;
        let mut updated_head = head.clone();
        let state = branch_state(ctx);
        if let Some(target) = update.target {
            updated_head.target = target;
            validate_target(&updated_head, ctx, self.head, &state)?;
        }
        if let Some(name) = update.name {
            updated_head.name = name;
            validate_name(&updated_head, ctx, &state)?;
        }
        // replace the value in self.heads at index idx with updated_head
        if let Some(entry) = self.heads.get_mut(idx) {
            *entry = updated_head;
        } else {
            return Err(anyhow!("Could not find the head to update"));
        }
        state.set_branch(self.clone())
    }

    fn push_branch(
        &self,
        ctx: &CommandContext,
        branch_name: String,
        with_force: bool,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let (_, reference) = get_branch(self, &branch_name)?;
        let default_target = branch_state(ctx).get_default_target()?;
        let commit = get_target_commit(&reference.target, ctx, self.head, &default_target)?;
        let remote_name = branch_state(ctx)
            .get_default_target()?
            .push_remote_name
            .ok_or(anyhow!(
                "No remote has been configured for the target branch"
            ))?;
        let upstream_refname = RemoteRefname::from_str(&reference.remote_reference(remote_name)?)
            .context("Failed to parse the remote reference for branch")?;
        ctx.push(
            commit.id(),
            &upstream_refname,
            with_force,
            None,
            Some(Some(self.id)),
        )
    }

    fn list_branches(&self, ctx: &CommandContext) -> Result<Vec<Series>> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let state = branch_state(ctx);
        let default_target = state.get_default_target()?;
        let mut all_series: Vec<Series> = vec![];
        let merge_base = ctx.repository().merge_base(self.head, default_target.sha)?;

        // All the commits between the head of the stack and the merge base (not inclusive)
        // Starts from the bottom of the stack
        let mut patches = ctx
            .repository()
            .log(self.head, LogUntil::Commit(merge_base))?
            .iter()
            .map(|c| match c.change_id() {
                Some(change_id) => CommitOrChangeId::ChangeId(change_id.to_string()),
                None => CommitOrChangeId::CommitId(c.id().to_string()),
            })
            .collect_vec();

        // We want the top of the stack to be first
        let mut heads_in_order = self.heads.clone();
        heads_in_order.reverse();

        for window in heads_in_order.windows(2) {
            if let [current_head, next_head] = window {
                let mut local_commits: Vec<CommitOrChangeId> = vec![];
                // if the patch matches the current head, add it to the local_commits list
                // if the patch matches the next head, bread out of the inner loop
                // if the patch does not either head, add it to the local_commits list and continue
                while !patches.is_empty() {
                    let patch = patches
                        .last()
                        .ok_or_else(|| anyhow!("No patches found"))?
                        .clone();
                    if current_head.target == patch {
                        local_commits.push(patch);
                        patches.pop();
                    } else if next_head.target == patch {
                        break;
                    } else {
                        local_commits.push(patch);
                        patches.pop();
                    }
                }
                let series = Series {
                    head: current_head.clone(),
                    local_commits,
                    remote_commits: vec![], // TODO
                };
                all_series.push(series);
            }
        }
        Ok(all_series)
    }
}

/// Validates that the commit in the reference target
///  - exists
///  - is between the stack (formerly vbranch) head (inclusive) and base (inclusive)
fn validate_target(
    reference: &PatchReference,
    ctx: &CommandContext,
    stack_head: git2::Oid,
    state: &VirtualBranchesHandle,
) -> Result<()> {
    let default_target = state.get_default_target()?;
    let commit = get_target_commit(&reference.target, ctx, stack_head, &default_target)?;
    let stack_commits = ctx
        .repository()
        // TODO: seems like the range that is actually needed is from head to the merge base
        .log(stack_head, LogUntil::Commit(default_target.sha))?
        .iter()
        .map(|c| c.id())
        .collect_vec();
    if !stack_commits.contains(&commit.id()) {
        return Err(anyhow!(
            "The commit {} is not between the stack head and the stack base",
            commit.id()
        ));
    }
    Ok(())
}

/// Validates the name of the stack head.
/// The name must be:
///  - unique within all stacks
///  - not the same as any existing git reference
///  - not including the `refs/heads/` prefix
fn validate_name(
    reference: &PatchReference,
    ctx: &CommandContext,
    state: &VirtualBranchesHandle,
) -> Result<()> {
    if reference.name.starts_with("refs/heads") {
        return Err(anyhow!("Stack head name cannot start with 'refs/heads'"));
    }
    // assert that the name is a valid branch name
    name_partial(reference.name.as_str().into()).context("Invalid branch name")?;
    // assert that there is no local git reference with this name
    if reference_exists(ctx, &reference.name)? {
        return Err(anyhow!(
            "A git reference with the name {} exists",
            &reference.name
        ));
    }
    let default_target = state.get_default_target()?;
    // assert that there is no remote git reference with this name
    if let Some(remote_name) = default_target.push_remote_name {
        if reference_exists(ctx, &reference.remote_reference(remote_name)?)? {
            return Err(anyhow!(
                "A git reference with the name {} exists",
                &reference.name
            ));
        }
    }
    // assert that there are no existing patch references with this name
    if state
        .list_all_branches()?
        .iter()
        .flat_map(|b| b.heads.iter())
        .any(|r| r.name == reference.name)
    {
        return Err(anyhow!(
            "A patch reference with the name {} exists",
            &reference.name
        ));
    }

    Ok(())
}

/// Given a branch id and a change id, returns the commit associated with the change id.
// TODO: We need a more efficient way of getting a commit by change id.
fn commit_by_branch_id_and_change_id<'a>(
    ctx: &'a CommandContext,
    stack_head: git2::Oid, // branch.head
    target_sha: git2::Oid, // default_target.sha
    change_id: &str,
) -> Result<git2::Commit<'a>> {
    // Find the commit with the change id
    let commit = ctx
        .repository()
        // TODO: seems like the range that is actually needed is from head to the merge base
        .log(stack_head, LogUntil::Commit(target_sha))?
        .iter()
        .map(|c| c.id())
        .find(|c| {
            let commit = ctx.repository().find_commit(*c).expect("Commit not found");
            commit.change_id().as_deref() == Some(change_id)
        })
        .and_then(|c| ctx.repository().find_commit(c).ok())
        .ok_or_else(|| anyhow!("Commit with change id {} not found", change_id))?;
    Ok(commit)
}

fn branch_state(ctx: &CommandContext) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(ctx.project().gb_dir())
}

fn get_branch(stack: &Branch, name: &str) -> Result<(usize, PatchReference)> {
    let (idx, head) = stack
        .heads
        .clone()
        .into_iter()
        .enumerate()
        .find(|(_, h)| h.name == name)
        .ok_or_else(|| anyhow!("Branch {} not found", name))?;
    Ok((idx, head))
}

fn get_target_commit<'a>(
    reference_target: &'a CommitOrChangeId,
    ctx: &'a CommandContext,
    stack_head: git2::Oid,
    default_target: &Target,
) -> Result<git2::Commit<'a>> {
    Ok(match reference_target {
        CommitOrChangeId::CommitId(commit_id) => {
            ctx.repository().find_commit(commit_id.parse()?)?
        }
        CommitOrChangeId::ChangeId(change_id) => {
            commit_by_branch_id_and_change_id(ctx, stack_head, default_target.sha, change_id)?
        }
    })
}

fn reference_exists(ctx: &CommandContext, name: &str) -> Result<bool> {
    let gix_repo = ctx.gix_repository()?;
    Ok(gix_repo.find_reference(name_partial(name.into())?).is_ok())
}
