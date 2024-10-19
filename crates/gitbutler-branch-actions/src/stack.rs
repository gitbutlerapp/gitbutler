use std::collections::HashMap;

use anyhow::{Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_project::Project;
use gitbutler_stack::{Stack, StackId, Target};
use gitbutler_stack_api::{commit_by_oid_or_change_id, PatchReferenceUpdate, StackExt};
use serde::{Deserialize, Serialize};

use crate::{
    actions::open_with_verify,
    commit::{commit_to_vbranch_commit, VirtualBranchCommit},
    r#virtual::{CommitData, IsCommitIntegrated, PatchSeries},
    VirtualBranchesExt,
};
use gitbutler_operating_modes::assure_open_workspace_mode;

/// Adds a new "series/branch" to the Stack.
/// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
/// The name cannot be the same as existing git references or existing patch references.
/// The target must reference a commit (or change) that is part of the stack.
/// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
///
/// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
/// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
/// If there are multiple heads pointing to the same patch and `preceding_head` is not specified,
/// that means the new head will be first in order for that patch.
/// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
pub fn create_series(
    project: &Project,
    branch_id: StackId,
    req: CreateSeriesRequest,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    // If target_patch is None, create a new head that points to the top of the stack (most recent patch)
    if let Some(target_patch) = req.target_patch {
        stack.add_series(
            ctx,
            PatchReference {
                target: target_patch,
                name: req.name,
                description: req.description,
            },
            req.preceding_head,
        )
    } else {
        stack.add_series_top_of_stack(ctx, req.name, req.description)
    }
}

/// Request to create a new series in a stack
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateSeriesRequest {
    /// Name of the new series
    name: String,
    /// Description of the new series - can be markdown or anything really
    description: Option<String>,
    /// The target patch (head) to create these series for. If let None, the new series will be at the top of the stack
    target_patch: Option<CommitOrChangeId>,
    /// The name of the series that preceded the newly created series.
    /// This is used to disambiguate the order when they point to the same patch
    preceding_head: Option<String>,
}

/// Removes series grouping from the Stack. This will not touch the patches / commits contained in the series.
/// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
/// If there were commits/changes that were *only* referenced by the removed branch,
/// those commits are moved to the branch underneath it (or more accurately, the preceding it)
pub fn remove_series(project: &Project, branch_id: StackId, head_name: String) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.remove_series(ctx, head_name)
}

/// Updates the name an existing series in the stack.
/// Same invariants as `create_series` apply.
/// If the series have been pushed to a remote, the name can not be changed as it corresponds to a remote ref.
pub fn update_series_name(
    project: &Project,
    branch_id: StackId,
    head_name: String,
    new_head_name: String,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            name: Some(new_head_name),
            ..Default::default()
        },
    )
}

/// Updates the description of an existing series in the stack.
/// The description can be set to `None` to remove it.
pub fn update_series_description(
    project: &Project,
    branch_id: StackId,
    head_name: String,
    description: Option<String>,
) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let mut stack = ctx.project().virtual_branches().get_branch(branch_id)?;
    stack.update_series(
        ctx,
        head_name,
        &PatchReferenceUpdate {
            description: Some(description),
            ..Default::default()
        },
    )
}

/// Pushes all series in the stack to the remote.
/// This operation will error out if the target has no push remote configured.
pub fn push_stack(project: &Project, branch_id: StackId, with_force: bool) -> Result<()> {
    let ctx = &open_with_verify(project)?;
    assure_open_workspace_mode(ctx).context("Requires an open workspace mode")?;
    let state = ctx.project().virtual_branches();
    let stack = state.get_branch(branch_id)?;

    let repo = ctx.repository();
    let merge_base =
        repo.find_commit(repo.merge_base(stack.head(), state.get_default_target()?.sha)?)?;
    let merge_base = if let Some(change_id) = merge_base.change_id() {
        CommitOrChangeId::ChangeId(change_id)
    } else {
        CommitOrChangeId::CommitId(merge_base.id().to_string())
    };

    let stack_series = stack.list_series(ctx)?;
    for series in stack_series {
        if series.head.target == merge_base {
            // Nothing to push for this one
            continue;
        }
        stack.push_series(ctx, series.head.name, with_force)?;
    }
    Ok(())
}

/// Returns the stack series for the API.
/// Newest first, oldest last in the list
pub(crate) fn stack_series(
    ctx: &CommandContext,
    branch: &mut Stack,
    default_target: &Target,
    check_commit: &IsCommitIntegrated,
    remote_commit_data: HashMap<CommitData, git2::Oid>,
) -> Result<(Vec<PatchSeries>, bool)> {
    let mut requires_force = false;
    let mut api_series: Vec<PatchSeries> = vec![];
    let stack_series = branch.list_series(ctx)?;
    let merge_base = ctx
        .repository()
        .merge_base(branch.head(), default_target.sha)?;
    for series in stack_series.clone() {
        let remote = default_target.push_remote_name();
        let upstream_reference = if series.head.pushed(remote.as_str(), ctx)? {
            series.head.remote_reference(remote.as_str()).ok()
        } else {
            None
        };
        let mut patches: Vec<VirtualBranchCommit> = vec![];
        for patch in series.clone().local_commits {
            let commit =
                commit_by_oid_or_change_id(&patch, ctx.repository(), branch.head(), merge_base)?;
            let is_integrated = check_commit.is_integrated(&commit)?;
            let copied_from_remote_id = CommitData::try_from(&commit)
                .ok()
                .and_then(|data| remote_commit_data.get(&data).copied());
            let remote_commit_id = commit
                .change_id()
                .and_then(|change_id| {
                    series
                        .remote_commit_ids_by_change_id
                        .get(&change_id)
                        .cloned()
                })
                .or(copied_from_remote_id)
                .or(if series.remote(&patch) {
                    Some(commit.id())
                } else {
                    None
                });
            if remote_commit_id.map_or(false, |id| commit.id() != id) {
                requires_force = true;
            }
            let vcommit = commit_to_vbranch_commit(
                ctx,
                branch,
                &commit,
                is_integrated,
                series.remote(&patch),
                copied_from_remote_id,
                remote_commit_id,
            )?;
            patches.push(vcommit);
        }
        patches.reverse();
        let mut upstream_patches = vec![];
        if let Some(upstream_reference) = upstream_reference.clone() {
            let remote_head = ctx
                .repository()
                .find_reference(&upstream_reference)?
                .peel_to_commit()?;
            for patch in series.upstream_only_commits {
                if let Ok(commit) = commit_by_oid_or_change_id(
                    &patch,
                    ctx.repository(),
                    remote_head.id(),
                    merge_base,
                ) {
                    let is_integrated = check_commit.is_integrated(&commit)?;
                    let vcommit = commit_to_vbranch_commit(
                        ctx,
                        branch,
                        &commit,
                        is_integrated,
                        true, // per definition
                        None, // per definition
                        Some(commit.id()),
                    )?;
                    upstream_patches.push(vcommit);
                };
            }
        }
        upstream_patches.reverse();
        if !upstream_patches.is_empty() {
            requires_force = true;
        }
        api_series.push(PatchSeries {
            name: series.head.name,
            description: series.head.description,
            upstream_reference,
            patches,
            upstream_patches,
        });
    }
    api_series.reverse();

    // This is done for compatibility with the legacy flow.
    // After a couple of weeks we can get rid of this.
    if let Err(e) = branch.set_legacy_compatible_stack_reference(ctx) {
        tracing::warn!("failed to set legacy compatible stack reference: {:?}", e);
    }

    Ok((api_series, requires_force))
}
