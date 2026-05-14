use anyhow::{Context as _, Result};
use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_workspace::legacy::{stack_heads_info, ui};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_operating_modes::ensure_open_workspace_mode;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails, Trailer},
};
use gitbutler_reference::{Refname, RemoteRefname};

use crate::{
    VirtualBranchesExt, base, base::BaseBranch, branch_manager::BranchManagerExt,
    branch_upstream_integration, branch_upstream_integration::IntegrationStrategy,
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
    base::set_base_branch(ctx, target_branch)
}

pub fn set_target_push_remote(ctx: &Context, push_remote: &str) -> Result<()> {
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

/// Unapplies a virtual branch and deletes the branch entry from the virtual branch state.
pub fn unapply_stack(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    stack_id: StackId,
    assigned_diffspec: Vec<DiffSpec>,
) -> Result<String> {
    ctx.verify(perm)?;
    ensure_open_workspace_mode(ctx, perm.read_permission())
        .context("Unapplying a stack requires open workspace mode")?;
    let stack = ctx.virtual_branches().get_stack_in_workspace(stack_id)?;

    let trailers = stack
        .heads
        .iter()
        .map(|head| Trailer::Branch(head.name.clone()));

    let details = SnapshotDetails::new(OperationKind::UnapplyBranch).with_trailers(trailers);
    let _snapshot = ctx.create_snapshot(details, perm).ok();
    let branch_manager = ctx.branch_manager();
    // NB: unapply_without_saving is also called from save_and_unapply
    let branch_name = branch_manager.unapply(
        stack_id,
        perm,
        false,
        assigned_diffspec,
        ctx.settings.feature_flags.cv3,
    )?;
    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    ws.refresh_from_head(&repo, &meta)?;
    Ok(branch_name)
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

pub(crate) trait Verify {
    fn verify(&self, perm: &mut RepoExclusive) -> Result<()>;
}

impl Verify for Context {
    fn verify(&self, perm: &mut RepoExclusive) -> Result<()> {
        crate::integration::verify_branch(self, perm)
    }
}
