use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
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

use crate::heads::add_head;
use crate::heads::get_head;
use crate::heads::remove_head;
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
    /// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
    /// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
    /// If there are multiple heads pointing to the same patch and `preceding_head` is not spcified,
    /// that means the new head will be first in order for that patch.
    /// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn add_series(
        &mut self,
        ctx: &CommandContext,
        head: PatchReference,
        preceding_head: Option<PatchReference>,
    ) -> Result<()>;

    /// Removes a branch from the Stack.
    /// The very last branch (reference) cannot be removed (A Stack must always contains at least one reference)
    /// If there were commits/changes that were *only* referenced by the removed branch,
    /// those commits are moved to the branch underneath it (or more accurately, the precee)
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn remove_series(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()>;

    /// Updates an existing branch in the stack.
    /// The same invariants as `add_branch` apply.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    fn update_series(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: &PatchReferenceUpdate,
    ) -> Result<()>;

    /// Pushes the reference (branch) to the Stack remote as derived from the default target.
    /// This operation will error out if the target has no push remote configured.
    fn push_series(
        &self,
        ctx: &CommandContext,
        branch_name: String,
        with_force: bool,
    ) -> Result<()>;

    /// Returns a list of all branches/series in the stack.
    /// This operation will compute the current list of local and remote commits that belong to each series.
    /// The first entry is the newest in the Stack (i.e the top of the stack).
    fn list_series(&self, ctx: &CommandContext) -> Result<Vec<Series>>;
}

/// Request to update a PatchReference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PatchReferenceUpdate {
    pub target_update: Option<TargetUpdate>,
    pub name: Option<String>,
}

/// Request to update the target of a PatchReference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetUpdate {
    /// The new patch (commit or change ID) that the reference should point to.
    pub target: CommitOrChangeId,
    /// If there are multiple heads that point to the same patch, the order can be disambiguated by specifying the `preceding_head`.
    /// Leaving this field empty will make the new head first in relation to other references pointing to this commit.
    pub preceding_head: Option<PatchReference>,
}

/// A Stack implementation for gitbutler_branch::Branch
/// This operates via a list of PatchReferences (heads) that are an attribute of gitbutler_branch::Branch.
/// In this context a (virtual) "Branch" is a stack of PatchReferences, each pointing to a commit (or change) within the stack.
///
/// This trait provides a defined interface for interacting with a Stack, the field `heads` on `Branch` should never be modified directly
/// outside of the trait implementation.
///
/// The heads must always be sorted in accordance with the order of the patches in the stack.
/// The first patches are in the beginning of the list and the most recent patches are at the end of the list (top of the stack)
/// Similarly, heads that point to earlier commits are first in the order, and the last head always points to the most recent patch.
/// If there are multiple heads that point to the same patch, the `add` and `update` operations can specify the intended order.
impl Stack for Branch {
    fn initialized(&self) -> bool {
        !self.heads.is_empty()
    }
    fn init(&mut self, ctx: &CommandContext) -> Result<()> {
        if self.initialized() {
            return Err(anyhow!("Stack already initialized"));
        }
        let commit = ctx.repository().find_commit(self.head)?;
        let reference = PatchReference {
            target: if let Some(change_id) = commit.change_id() {
                CommitOrChangeId::ChangeId(change_id.to_string())
            } else {
                CommitOrChangeId::CommitId(commit.id().to_string())
            },
            name: normalize_branch_name(&self.name)?,
        };
        let state = branch_state(ctx);
        validate_name(&reference, ctx, &state)?;
        self.heads = vec![reference];
        state.set_branch(self.clone())
    }

    fn add_series(
        &mut self,
        ctx: &CommandContext,
        new_head: PatchReference,
        preceding_head: Option<PatchReference>,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let state = branch_state(ctx);
        let patches = stack_patches(ctx, &state, self.head, true)?;
        validate_name(&new_head, ctx, &state)?;
        validate_target(&new_head, ctx, self.head, &state)?;
        let updated_heads = add_head(self.heads.clone(), new_head, preceding_head, patches)?;
        self.heads = updated_heads;
        state.set_branch(self.clone())
    }

    fn remove_series(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        (self.heads, _) = remove_head(self.heads.clone(), branch_name)?;
        let state = branch_state(ctx);
        state.set_branch(self.clone())
    }

    fn update_series(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: &PatchReferenceUpdate,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        if update == &PatchReferenceUpdate::default() {
            return Ok(()); // noop
        }
        let (_, head) = get_head(&self.heads, &branch_name)?;
        let mut new_head = head.clone();
        let state = branch_state(ctx);
        if let Some(target_update) = &update.target_update {
            new_head.target = target_update.target.clone();
            validate_target(&new_head, ctx, self.head, &state)?;
        }
        if let Some(name) = update.name.clone() {
            new_head.name = name;
            validate_name(&new_head, ctx, &state)?;
        }
        let patches = stack_patches(ctx, &state, self.head, true)?;
        let preceding_head = update
            .target_update
            .clone()
            .and_then(|update| update.preceding_head);
        let updated_heads = add_head(self.heads.clone(), new_head, preceding_head, patches)?;
        // first add the updated head, then remove the old one (otherwise remove_heads may will fail due to the last head being removed)
        let (updated_heads, moved_another_reference) =
            remove_head(updated_heads, head.name.clone())?;
        // Remove head will never leave the stack in an invalid state (i.e. no orphaned patches)
        // However, in case there were orphaned patches, we want to error out the entire update operation since that is less surprising
        if update.target_update.is_some() && moved_another_reference {
            bail!("This update would cause orphaned patches, which is disallowed");
        }
        self.heads = updated_heads;
        state.set_branch(self.clone())
    }

    fn push_series(
        &self,
        ctx: &CommandContext,
        branch_name: String,
        with_force: bool,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let (_, reference) = get_head(&self.heads, &branch_name)?;
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

    fn list_series(&self, ctx: &CommandContext) -> Result<Vec<Series>> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let state = branch_state(ctx);
        let mut all_series: Vec<Series> = vec![];

        // All the commits between the head of the stack and the merge base (not inclusive)
        // Starts from the bottom of the stack
        let mut patches = stack_patches(ctx, &state, self.head, false)?;

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
///
/// If the patch reference is a commit ID, it must be the case that the the commit has no change ID associated with it.
/// In other words, change IDs are enforeced to be preferred over commit IDs when available.
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
    // Enforce that change ids are used when available
    if let CommitOrChangeId::CommitId(_) = reference.target {
        if commit.change_id().is_some() {
            return Err(anyhow!(
                "The commit {} has a change id associated with it. Use the change id instead",
                commit.id()
            ));
        }
    }
    Ok(())
}

/// Returns the list of patches between the stack head and the merge base.
/// The most recent patch is at the top of the 'stack' (i.e. the last element in the vector)
fn stack_patches(
    ctx: &CommandContext,
    state: &VirtualBranchesHandle,
    stack_head: git2::Oid,
    include_merge_base: bool,
) -> Result<Vec<CommitOrChangeId>> {
    let default_target = state.get_default_target()?;
    let merge_base = ctx
        .repository()
        .merge_base(stack_head, default_target.sha)?;
    let mut patches = ctx
        .repository()
        .log(stack_head, LogUntil::Commit(merge_base))?
        .iter()
        .map(|c| match c.change_id() {
            Some(change_id) => CommitOrChangeId::ChangeId(change_id.to_string()),
            None => CommitOrChangeId::CommitId(c.id().to_string()),
        })
        .collect_vec();
    if include_merge_base {
        patches.push(CommitOrChangeId::CommitId(merge_base.to_string()));
    }
    patches.reverse();
    Ok(patches)
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
