use anyhow::{Context as _, Result};
use but_core::ref_metadata::StackId;
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_workspace::legacy::{stack_heads_info, ui};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_reference::{Refname, RemoteRefname};

use crate::{
    base,
    base::BaseBranch,
    branch_manager::BranchManagerExt,
    branch_upstream_integration,
    branch_upstream_integration::IntegrationStrategy,
    upstream_integration::{
        self, BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
        StackStatuses, UpstreamIntegrationContext,
    },
};

pub fn create_virtual_branch(
    ctx: &Context,
    create: &BranchCreateRequest,
    perm: &mut RepoExclusive,
) -> Result<ui::StackEntryNoOpt> {
    ctx.verify(perm)?;
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Creating a branch requires open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(create, perm)?;
    let repo = ctx.repo.get()?;
    Ok(ui::StackEntryNoOpt {
        id: stack.id,
        heads: stack_heads_info(&stack, &repo)?,
        tip: stack.head_oid(ctx)?,
        order: Some(stack.order),
        is_checked_out: false,
    })
}

pub fn set_base_branch(
    ctx: &Context,
    target_branch: &RemoteRefname,
    perm: &mut RepoExclusive,
) -> Result<BaseBranch> {
    let _ = ctx.create_snapshot(SnapshotDetails::new(OperationKind::SetBaseBranch), perm);
    base::set_base_branch(ctx, perm.read_permission(), target_branch)
}

pub fn set_target_push_remote(ctx: &mut Context, push_remote: &str) -> Result<()> {
    base::set_target_push_remote(ctx, push_remote)
}

pub fn push_base_branch(ctx: &Context, with_force: bool) -> Result<()> {
    base::push(ctx, with_force)
}

pub fn integrate_upstream_commits(
    ctx: &mut Context,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Integrating upstream commits requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.write_permission(),
    );
    branch_upstream_integration::integrate_upstream_commits_for_series(
        ctx,
        stack_id,
        guard.write_permission(),
        series_name,
        integration_strategy,
    )
}

pub fn get_initial_integration_steps_for_branch(
    ctx: &Context,
    stack_id: Option<StackId>,
    branch_name: String,
    perm: &RepoShared,
) -> Result<Vec<branch_upstream_integration::InteractiveIntegrationStep>> {
    ensure_open_workspace_mode(ctx, perm)
        .context("Getting initial integration steps requires open workspace mode")?;
    branch_upstream_integration::get_initial_integration_steps_for_branch(
        ctx,
        stack_id,
        branch_name,
    )
}

pub fn integrate_branch_with_steps(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    steps: Vec<branch_upstream_integration::InteractiveIntegrationStep>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    ctx.verify(guard.write_permission())?;
    ensure_open_workspace_mode(ctx, guard.read_permission())
        .context("Integrating a branch with steps requires open workspace mode")?;
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::MergeUpstream),
        guard.write_permission(),
    );
    branch_upstream_integration::integrate_branch_with_steps(
        ctx,
        stack_id,
        branch_name,
        steps,
        guard.write_permission(),
    )
}

pub fn create_virtual_branch_from_branch_with_perm(
    ctx: &mut Context,
    branch: &Refname,
    pr_number: Option<usize>,
    perm: &mut RepoExclusive,
) -> Result<(StackId, Vec<StackId>, Vec<String>)> {
    ctx.verify(perm)?;
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Creating a virtual branch from a branch open workspace mode")?;
    let branch_manager = ctx.branch_manager();
    branch_manager.create_virtual_branch_from_branch(branch, pr_number, perm)
}

pub fn upstream_integration_statuses(
    ctx: &mut Context,
    target_commit_oid: Option<gix::ObjectId>,
    review_map: &std::collections::HashMap<String, but_forge::ForgeReview>,
) -> Result<StackStatuses> {
    let mut guard = ctx.exclusive_worktree_access();

    let repo = ctx.repo.get()?;
    let context = UpstreamIntegrationContext::open(
        ctx,
        target_commit_oid,
        guard.write_permission(),
        &repo,
        review_map,
    )?;

    upstream_integration::upstream_integration_statuses(&context)
}

pub fn integrate_upstream(
    ctx: &mut Context,
    resolutions: &[Resolution],
    base_branch_resolution: Option<BaseBranchResolution>,
    review_map: &std::collections::HashMap<String, but_forge::ForgeReview>,
) -> Result<IntegrationOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::UpdateWorkspaceBase),
        guard.write_permission(),
    );

    upstream_integration::integrate_upstream(
        ctx,
        resolutions,
        base_branch_resolution,
        review_map,
        guard.write_permission(),
    )
}

pub fn resolve_upstream_integration(
    ctx: &mut Context,
    resolution_approach: BaseBranchResolutionApproach,
    review_map: &std::collections::HashMap<String, but_forge::ForgeReview>,
) -> Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();

    upstream_integration::resolve_upstream_integration(
        ctx,
        resolution_approach,
        review_map,
        guard.write_permission(),
    )
}

pub(crate) trait Verify {
    fn verify(&self, perm: &mut RepoExclusive) -> Result<()>;
}

impl Verify for Context {
    fn verify(&self, perm: &mut RepoExclusive) -> Result<()> {
        crate::integration::verify_branch(self, perm)
    }
}
