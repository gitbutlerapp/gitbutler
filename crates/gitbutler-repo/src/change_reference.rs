use std::str::FromStr;

use crate::{LogUntil, RepoActionsExt, RepositoryExt as _};
use anyhow::Context;
use anyhow::{anyhow, Result};
use gitbutler_branch::ChangeReference;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_branch::{Branch, BranchId};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_reference::ReferenceName;
use itertools::Itertools;

/// Given a branch id, returns the the GitButler references associated with the branch.
/// References within the same branch effectively represent a stack of sub-branches.
pub fn list_branch_references(
    ctx: &CommandContext,
    branch_id: BranchId,
) -> Result<Vec<ChangeReference>> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let vbranch = handle.get_branch(branch_id)?;
    Ok(vbranch.references)
}

/// Creates a new virtual branch reference and associates it with the branch.
/// However this will return an error if:
///   - a reference for the same commit already exists, an error is returned.
///   - the reference name already exists, an error is returned.
pub fn create_change_reference(
    ctx: &CommandContext,
    branch_id: BranchId,
    name: ReferenceName,
    change_id: String,
) -> Result<ChangeReference> {
    // The reference must be parseable as a remote reference
    gitbutler_reference::RemoteRefname::from_str(&name)
        .context("Failed to parse the provided reference")?;
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());

    // The branch must exist
    let mut vbranch = handle.get_branch(branch_id)?;

    // Enusre that the commit acutally exists
    let commit = commit_by_branch_id_and_change_id(ctx, &vbranch, &handle, change_id)?;

    let change_id = commit
        .change_id()
        .ok_or(anyhow!("Commit {} does not have a change id", commit.id()))?;

    let branch_reference = ChangeReference {
        name,
        branch_id,
        change_id: change_id.clone(),
    };
    let all_references = handle
        .list_all_branches()?
        .into_iter()
        .flat_map(|branch| branch.references)
        .collect_vec();
    // Ensure the reference name does not already exist
    if all_references
        .iter()
        .any(|r| r.name == branch_reference.name)
    {
        return Err(anyhow!(
            "A reference {} already exists",
            branch_reference.name
        ));
    }
    // Ensure the change is not already referenced
    if all_references.iter().any(|r| r.change_id == change_id) {
        return Err(anyhow!(
            "A reference for change {} already exists",
            change_id
        ));
    }
    validate_commit(&vbranch, commit.id(), ctx, &handle)?;
    vbranch.references.push(branch_reference.clone());
    handle.set_branch(vbranch)?;
    Ok(branch_reference)
}

/// Updates an existing branch reference to point to a different commit.
/// Only the commit and change_id can be updated.
/// The reference is identified by the branch id and the reference name.
/// This function will return an error if:
/// - this reference does not exist
/// - the reference exists, but the commit id is not in the branch
/// - the reference exists, but the commit id is already associated with another reference
///
/// If the commit ID is the same as the current commit ID, the function is a no-op.
pub fn update_change_reference(
    ctx: &CommandContext,
    branch_id: BranchId,
    name: ReferenceName,
    new_change_id: String,
) -> Result<ChangeReference> {
    // The reference must be parseable as a remote reference
    gitbutler_reference::RemoteRefname::from_str(&name)
        .context("Failed to parse the provided reference")?;
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    // The branch must exist
    let mut vbranch = handle.get_branch(branch_id)?;

    // Enusre that the commit acutally exists
    let new_commit =
        commit_by_branch_id_and_change_id(ctx, &vbranch, &handle, new_change_id.clone())
            .context(anyhow!("Commit for change_id {} not found", new_change_id))?;

    // Fail early if the commit is not valid
    validate_commit(&vbranch, new_commit.id(), ctx, &handle)?;

    let reference = vbranch
        .references
        .iter_mut()
        .find(|r| r.name == name)
        .ok_or(anyhow!(
            "Reference {} not found for branch {}",
            name,
            branch_id
        ))?;
    reference.change_id = new_change_id;
    let new_reference = reference.clone();
    handle.set_branch(vbranch)?;
    Ok(new_reference)
}

/// Pushes a gitbutler branch reference to the remote repository.
pub fn push_change_reference(
    ctx: &CommandContext,
    branch_id: BranchId,
    name: ReferenceName,
    with_force: bool,
) -> Result<()> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let vbranch = handle.get_branch(branch_id)?;
    let reference = vbranch
        .references
        .iter()
        .find(|r| r.name == name)
        .ok_or_else(|| anyhow!("Reference {} not found", name))?;
    let upstream_refname = gitbutler_reference::RemoteRefname::from_str(&reference.name)
        .context("Failed to parse the provided reference")?;
    let commit =
        commit_by_branch_id_and_change_id(ctx, &vbranch, &handle, reference.change_id.clone())?;
    ctx.push(
        commit.id(),
        &upstream_refname,
        with_force,
        None,
        Some(Some(branch_id)),
    )
}

/// Given a branch id and a change id, returns the commit associated with the change id.
// TODO: We need a more efficient way of getting a commit by change id.
fn commit_by_branch_id_and_change_id<'a>(
    ctx: &'a CommandContext,
    vbranch: &Branch,
    handle: &VirtualBranchesHandle,
    change_id: String,
) -> Result<git2::Commit<'a>> {
    let target = handle.get_default_target()?;
    // Find the commit with the change id
    let commit = ctx
        .repository()
        .log(vbranch.head, LogUntil::Commit(target.sha))?
        .iter()
        .map(|c| c.id())
        .find(|c| {
            let commit = ctx.repository().find_commit(*c).expect("Commit not found");
            commit.change_id().as_deref() == Some(&change_id)
        })
        .and_then(|c| ctx.repository().find_commit(c).ok())
        .ok_or_else(|| anyhow!("Commit with change id {} not found", change_id))?;
    Ok(commit)
}

/// Validates a commit in the following ways:
/// - The reference does not already exists for any other branch
/// - There is no other reference already pointing to the commit
/// - The commit is between the branch base and the branch head
fn validate_commit(
    vbranch: &Branch,
    commit_id: git2::Oid,
    ctx: &CommandContext,
    handle: &VirtualBranchesHandle,
) -> Result<()> {
    let target = handle.get_default_target()?;
    let branch_commits = ctx
        .repository()
        .log(vbranch.head, LogUntil::Commit(target.sha))?
        .iter()
        .map(|c| c.id())
        .collect_vec();

    // Assert that the commit is between the branch base and the branch head
    if !branch_commits.contains(&commit_id) {
        return Err(anyhow!(
            "The commit {} is not between the branch base and the branch head",
            commit_id
        ));
    }
    Ok(())
}
