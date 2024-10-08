use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_reference::normalize_branch_name;
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::LogUntil;
use gitbutler_repo::RepoActionsExt;
use gitbutler_repo::RepositoryExt;
use gitbutler_stack::Branch;
use gitbutler_stack::Target;
use gitbutler_stack::VirtualBranchesHandle;
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
    fn initialize(&mut self, ctx: &CommandContext) -> Result<()>;

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
        preceding_head_name: Option<String>,
    ) -> Result<()>;

    /// A convinience method just like `add_series`, but adds a new branch on top of the stack.
    fn add_series_top_of_stack(
        &mut self,
        ctx: &CommandContext,
        name: String,
        description: Option<String>,
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

    /// Updates the most recent series of the stack to point to a new patch (commit or change ID).
    /// This is a helper function that is equivalent to `update_series` with the target update set.
    fn set_stack_head(&mut self, ctx: &CommandContext, commit_id: git2::Oid) -> Result<()>;

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
    /// If present, this sets the value of the description field.
    /// It is possible to set this to Some(None) which will remove an existing description.
    pub description: Option<Option<String>>,
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
    fn initialize(&mut self, ctx: &CommandContext) -> Result<()> {
        if self.initialized() {
            return Ok(());
        }
        let commit = ctx.repository().find_commit(self.head())?;
        let mut reference = PatchReference {
            target: if let Some(change_id) = commit.change_id() {
                CommitOrChangeId::ChangeId(change_id.to_string())
            } else {
                CommitOrChangeId::CommitId(commit.id().to_string())
            },
            name: normalize_branch_name(&self.name)?,
            description: None,
        };
        let state = branch_state(ctx);
        let remote_reference_exists = state
            .get_default_target()?
            .push_remote_name
            .and_then(|remote| {
                reference
                    .remote_reference(remote.as_str())
                    .and_then(|reference| reference_exists(ctx, &reference))
                    .ok()
            })
            .unwrap_or(false);

        if reference_exists(ctx, &reference.name)?
            || patch_reference_exists(&state, &reference.name)?
            || remote_reference_exists
        {
            // TODO: do something better here
            let prefix = rand::random::<u32>().to_string();
            reference.name = format!("{}-{}", &reference.name, prefix);
        }
        validate_name(&reference, ctx, &state)?;
        self.heads = vec![reference];
        state.set_branch(self.clone())
    }

    fn add_series(
        &mut self,
        ctx: &CommandContext,
        new_head: PatchReference,
        preceding_head_name: Option<String>,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let preceding_head = if let Some(preceding_head_name) = preceding_head_name {
            let (_, preceding_head) = get_head(&self.heads, &preceding_head_name)
                .context("The specified preceding_head could not be found")?;
            Some(preceding_head)
        } else {
            None
        };
        let state = branch_state(ctx);
        let patches = stack_patches(ctx, &state, self.head(), true)?;
        validate_name(&new_head, ctx, &state)?;
        validate_target(&new_head, ctx, self.head(), &state)?;
        let updated_heads = add_head(self.heads.clone(), new_head, preceding_head, patches)?;
        self.heads = updated_heads;
        state.set_branch(self.clone())
    }

    fn add_series_top_of_stack(
        &mut self,
        ctx: &CommandContext,
        name: String,
        description: Option<String>,
    ) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let current_top_head = self.heads.last().ok_or(anyhow!(
            "Stack is in an invalid state - heads list is empty"
        ))?;
        let new_head = PatchReference {
            target: current_top_head.target.clone(),
            name,
            description,
        };
        self.add_series(ctx, new_head, Some(current_top_head.name.clone()))
    }

    fn remove_series(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        (self.heads, _) = remove_head(self.heads.clone(), branch_name)?;
        let state = branch_state(ctx);
        state.set_branch(self.clone())
    }

    fn set_stack_head(&mut self, ctx: &CommandContext, commit_id: git2::Oid) -> Result<()> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        if self.head() != commit_id {
            bail!("The commit {} is not the head of the stack", commit_id);
        }
        let commit = ctx.repository().find_commit(commit_id)?;
        let patch = if let Some(change_id) = commit.change_id() {
            CommitOrChangeId::ChangeId(change_id.to_string())
        } else {
            CommitOrChangeId::CommitId(commit.id().to_string())
        };

        let state = branch_state(ctx);
        let stack_head = self.head();
        let head = self
            .heads
            .last_mut()
            .ok_or_else(|| anyhow!("Invalid state: no heads found"))?;
        head.target = patch.clone();
        validate_target(head, ctx, stack_head, &state)?;
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

        let state = branch_state(ctx);
        let patches = stack_patches(ctx, &state, self.head(), true)?;
        let mut updated_heads = self.heads.clone();

        // Handle target updates
        if let Some(target_update) = &update.target_update {
            let mut new_head = updated_heads
                .clone()
                .into_iter()
                .find(|h| h.name == branch_name)
                .ok_or_else(|| anyhow!("Series with name {} not found", branch_name))?;
            new_head.target = target_update.target.clone();
            validate_target(&new_head, ctx, self.head(), &state)?;
            let preceding_head = update
                .target_update
                .clone()
                .and_then(|update| update.preceding_head);
            // drop the old head and add the new one
            let (idx, _) = get_head(&updated_heads, &branch_name)?;
            updated_heads.remove(idx);
            if patches.last() != updated_heads.last().map(|h| &h.target) {
                bail!("This update would cause orphaned patches, which is disallowed");
            }
            updated_heads = add_head(
                updated_heads,
                new_head.clone(),
                preceding_head,
                patches.clone(),
            )?;
        }

        // Handle name updates
        if let Some(name) = update.name.clone() {
            let head = updated_heads
                .iter_mut()
                .find(|h: &&mut PatchReference| h.name == branch_name);
            if let Some(head) = head {
                // ensure that the head has not been pushed to a remote yet
                let default_target = branch_state(ctx).get_default_target()?;
                if let Some(remote) = default_target.push_remote_name {
                    if reference_exists(ctx, &head.remote_reference(remote.as_str())?)? {
                        bail!("Cannot update the name of a head that has been pushed to a remote");
                    }
                }
                head.name = name;
                validate_name(head, ctx, &state)?;
            }
        }

        // Handle description updates
        if let Some(description) = update.description.clone() {
            let head = updated_heads.iter_mut().find(|h| h.name == branch_name);
            if let Some(head) = head {
                head.description = description;
            }
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
        let commit =
            commit_by_oid_or_change_id(&reference.target, ctx, self.head(), &default_target)?;
        let remote_name = branch_state(ctx)
            .get_default_target()?
            .push_remote_name
            .ok_or(anyhow!(
                "No remote has been configured for the target branch"
            ))?;
        let upstream_refname =
            RemoteRefname::from_str(&reference.remote_reference(remote_name.as_str())?)
                .context("Failed to parse the remote reference for branch")?;
        ctx.push(
            commit.id(),
            &upstream_refname,
            with_force,
            None,
            Some(Some(self.id)),
        )
    }

    // todo: remote commits are not being populated yet
    fn list_series(&self, ctx: &CommandContext) -> Result<Vec<Series>> {
        if !self.initialized() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        let state = branch_state(ctx);
        let mut all_series: Vec<Series> = vec![];
        let repo = ctx.repository();
        let default_target = state.get_default_target()?;
        let mut previous_head = repo.merge_base(self.head(), default_target.sha)?;
        for head in self.heads.clone() {
            let head_commit =
                commit_by_oid_or_change_id(&head.target, ctx, self.head(), &default_target)?.id();
            let local_patches = repo
                .log(head_commit, LogUntil::Commit(previous_head))?
                .iter()
                .rev() // oldest commit first
                .map(|c| match c.change_id() {
                    Some(change_id) => CommitOrChangeId::ChangeId(change_id.to_string()),
                    None => CommitOrChangeId::CommitId(c.id().to_string()),
                })
                .collect_vec();

            let mut remote_patches: Vec<CommitOrChangeId> = vec![];
            if let Some(remote_name) = default_target.push_remote_name.as_ref() {
                if head.pushed(remote_name, ctx).unwrap_or_default() {
                    let head_commit = repo
                        .find_reference(&head.remote_reference(remote_name)?)?
                        .peel_to_commit()?;
                    let merge_base = repo.merge_base(head_commit.id(), default_target.sha)?;
                    repo.log(head_commit.id(), LogUntil::Commit(merge_base))?
                        .iter()
                        .rev()
                        .map(|c| match c.change_id() {
                            Some(change_id) => CommitOrChangeId::ChangeId(change_id.to_string()),
                            None => CommitOrChangeId::CommitId(c.id().to_string()),
                        })
                        .for_each(|c| remote_patches.push(c));
                }
            };
            all_series.push(Series {
                head: head.clone(),
                local_commits: local_patches,
                remote_commits: remote_patches,
            });
            previous_head = head_commit;
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
    let commit = commit_by_oid_or_change_id(&reference.target, ctx, stack_head, &default_target)?;

    let merge_base = ctx
        .repository()
        .merge_base(stack_head, default_target.sha)?;
    let mut stack_commits = ctx
        .repository()
        .log(stack_head, LogUntil::Commit(merge_base))?
        .iter()
        .map(|c| c.id())
        .collect_vec();
    stack_commits.insert(0, merge_base);
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
        if reference_exists(ctx, &reference.remote_reference(remote_name.as_str())?)? {
            return Err(anyhow!(
                "A git reference with the name {} exists",
                &reference.name
            ));
        }
    }
    // assert that there are no existing patch references with this name
    if patch_reference_exists(state, &reference.name)? {
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
    let merge_base = ctx.repository().merge_base(stack_head, target_sha)?;
    let commit = ctx
        .repository()
        .log(stack_head, LogUntil::Commit(merge_base))?
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

pub fn commit_by_oid_or_change_id<'a>(
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

fn patch_reference_exists(state: &VirtualBranchesHandle, name: &str) -> Result<bool> {
    Ok(state
        .list_all_branches()?
        .iter()
        .flat_map(|b| b.heads.iter())
        .any(|r| r.name == name))
}
