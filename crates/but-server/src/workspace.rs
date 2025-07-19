use std::collections::HashSet;

use anyhow::Context;
use but_graph::VirtualBranchesTomlMetadata;
use but_hunk_assignment::HunkAssignmentRequest;
use but_workspace::MoveChangesResult;
use but_workspace::commit_engine;
use but_workspace::commit_engine::StackSegmentId;
use gitbutler_branch_actions::{BranchManagerExt, update_workspace_commit};
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_oxidize::OidExt;
use gitbutler_project::{Project, ProjectId};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::RequestContext;

// Helper type for JSON parsing
#[derive(serde::Deserialize)]
struct HexHash(String);

impl From<HexHash> for git2::Oid {
    fn from(hex: HexHash) -> Self {
        git2::Oid::from_str(&hex.0).expect("Invalid hex hash")
    }
}

impl From<HexHash> for gix::ObjectId {
    fn from(hex: HexHash) -> Self {
        gix::ObjectId::from_hex(hex.0.as_bytes()).expect("Invalid hex hash")
    }
}

// Parameter structs for JSON deserialization
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StacksParams {
    project_id: ProjectId,
    filter: Option<but_workspace::StacksFilter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectIdParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackDetailsParams {
    project_id: ProjectId,
    stack_id: StackId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchDetailsParams {
    project_id: ProjectId,
    branch_name: String,
    remote: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCommitFromWorktreeChangesParams {
    project_id: ProjectId,
    stack_id: StackId,
    parent_id: Option<HexHash>,
    worktree_changes: Vec<but_workspace::DiffSpec>,
    message: String,
    stack_branch_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmendCommitFromWorktreeChangesParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    worktree_changes: Vec<but_workspace::DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscardWorktreeChangesParams {
    project_id: ProjectId,
    worktree_changes: Vec<but_workspace::DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveChangesBetweenCommitsParams {
    project_id: ProjectId,
    source_stack_id: StackId,
    source_commit_id: HexHash,
    destination_stack_id: StackId,
    destination_commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitBranchParams {
    project_id: ProjectId,
    source_stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UncommitChangesParams {
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
    assign_to: Option<StackId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StashIntoBranchParams {
    project_id: ProjectId,
    branch_name: String,
    worktree_changes: Vec<but_workspace::DiffSpec>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetCommitsParams {
    project_id: ProjectId,
    last_commit_id: Option<HexHash>,
    page_size: Option<usize>,
}

pub fn stacks(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: StacksParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let repo = command_ctx.gix_repo_for_merging_non_persisting()?;

    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let meta = ref_metadata_toml(command_ctx.project())?;
        but_workspace::stacks_v3(&repo, &meta, params.filter.unwrap_or_default())
    } else {
        but_workspace::stacks(
            &command_ctx,
            &project.gb_dir(),
            &repo,
            params.filter.unwrap_or_default(),
        )
    }?;

    Ok(serde_json::to_value(result)?)
}

#[cfg(unix)]
pub fn show_graph_svg(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ProjectIdParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let repo = command_ctx.gix_repo_minimal()?;
    let meta = ref_metadata_toml(&project)?;
    let mut graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: Some(300),
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            extra_target_commit_id: meta.data().default_target.as_ref().map(|t| t.sha.to_gix()),
        },
    )?;
    const LIMIT: usize = 3000;
    if graph.num_segments() > LIMIT {
        let mut topo = graph.topo_walk();
        let mut count = 0;

        let mut remove = Vec::new();
        while let Some(sidx) = topo.next(&*graph) {
            count += 1;
            if count > LIMIT {
                remove.push(sidx);
            }
        }
        tracing::warn!(
            "Pruning {nodes} to assure 'dot' won't hang",
            nodes = remove.len()
        );
        for sidx in remove {
            graph.remove_node(sidx);
        }
        graph.set_hard_limit_hit();
    }
    graph.open_as_svg();
    Ok(json!({}))
}

pub fn stack_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: StackDetailsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let repo = command_ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(command_ctx.project())?;
        but_workspace::stack_details_v3(params.stack_id, &repo, &meta)
    } else {
        but_workspace::stack_details(&project.gb_dir(), params.stack_id, &command_ctx)
    }?;

    Ok(serde_json::to_value(result)?)
}

pub fn branch_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: BranchDetailsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;

    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let repo = command_ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(command_ctx.project())?;
        let ref_name: gix::refs::FullName = match params.remote.as_deref() {
            None => {
                format!("refs/heads/{}", params.branch_name)
            }
            Some(remote) => {
                format!("refs/remotes/{remote}/{}", params.branch_name)
            }
        }
        .try_into()
        .map_err(anyhow::Error::from)?;
        but_workspace::branch_details_v3(&repo, ref_name.as_ref(), &meta)
    } else {
        but_workspace::branch_details(
            &project.gb_dir(),
            &params.branch_name,
            params.remote.as_deref(),
            &command_ctx,
        )
    }?;

    Ok(serde_json::to_value(result)?)
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

pub fn create_commit_from_worktree_changes(
    ctx: &RequestContext,
    params: Value,
) -> anyhow::Result<Value> {
    let params: CreateCommitFromWorktreeChangesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = command_ctx.prepare_snapshot(guard.read_permission());

    let outcome = commit_engine::create_commit_simple(
        &command_ctx,
        params.stack_id,
        params.parent_id.map(|id| id.into()),
        params.worktree_changes,
        params.message.clone(),
        params.stack_branch_name,
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        command_ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            params.message.to_owned(),
            None,
            guard.write_permission(),
        )
    });

    let outcome = outcome?;
    Ok(serde_json::to_value(
        commit_engine::ui::CreateCommitOutcome::from(outcome),
    )?)
}

pub fn amend_commit_from_worktree_changes(
    ctx: &RequestContext,
    params: Value,
) -> anyhow::Result<Value> {
    let params: AmendCommitFromWorktreeChangesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let mut guard = project.exclusive_worktree_access();
    let repo = but_core::open_repo_for_merging(project.worktree_path())?;
    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(params.stack_id),
        commit_engine::Destination::AmendCommit {
            commit_id: params.commit_id.into(),
            new_message: None,
        },
        None,
        params.worktree_changes,
        ctx.app_settings.get()?.context_lines,
        guard.write_permission(),
    )?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(serde_json::to_value(
        commit_engine::ui::CreateCommitOutcome::from(outcome),
    )?)
}

pub fn discard_worktree_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: DiscardWorktreeChangesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let repo = but_core::open_repo(project.worktree_path())?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );
    let refused = but_workspace::discard_workspace_changes(
        &repo,
        params.worktree_changes,
        ctx.app_settings.get()?.context_lines,
    )?;
    if !refused.is_empty() {
        tracing::warn!(?refused, "Failed to discard at least one hunk");
    }
    Ok(serde_json::to_value(refused)?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UIMoveChangesResult {
    replaced_commits: Vec<(String, String)>,
}

impl From<MoveChangesResult> for UIMoveChangesResult {
    fn from(value: MoveChangesResult) -> Self {
        Self {
            replaced_commits: value
                .replaced_commits
                .into_iter()
                .map(|(x, y)| (x.to_hex().to_string(), y.to_hex().to_string()))
                .collect(),
        }
    }
}

pub fn move_changes_between_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: MoveChangesBetweenCommitsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::AmendCommit),
        guard.write_permission(),
    );
    let result = but_workspace::move_changes_between_commits(
        &command_ctx,
        params.source_stack_id,
        params.source_commit_id.into(),
        params.destination_stack_id,
        params.destination_commit_id.into(),
        params.changes,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(result))?)
}

pub fn split_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: SplitBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let (_, move_changes_result) = but_workspace::split_branch(
        &command_ctx,
        params.source_stack_id,
        params.source_branch_name,
        params.new_branch_name.clone(),
        &params.file_changes_to_split_off,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    let refname = Refname::Local(LocalRefname::new(&params.new_branch_name, None));
    let branch_manager = command_ctx.branch_manager();
    branch_manager.create_virtual_branch_from_branch(
        &refname,
        None,
        None,
        guard.write_permission(),
    )?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(
        move_changes_result,
    ))?)
}

pub fn split_branch_into_dependent_branch(
    ctx: &RequestContext,
    params: Value,
) -> anyhow::Result<Value> {
    let params: SplitBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let move_changes_result = but_workspace::split_into_dependent_branch(
        &command_ctx,
        params.source_stack_id,
        params.source_branch_name,
        params.new_branch_name.clone(),
        &params.file_changes_to_split_off,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(
        move_changes_result,
    ))?)
}

pub fn uncommit_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: UncommitChangesParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let mut command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );

    let before_assignments = if params.assign_to.is_some() {
        let changes = but_hunk_assignment::assignments_with_fallback(
            &mut command_ctx,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
        )?;
        Some(changes.0)
    } else {
        None
    };

    let result = but_workspace::remove_changes_from_commit_in_stack(
        &command_ctx,
        params.stack_id,
        params.commit_id.into(),
        params.changes,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, params.assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            &mut command_ctx,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
        )?;

        let before_assignments = before_assignments
            .into_iter()
            .filter_map(|a| a.id)
            .collect::<HashSet<_>>();

        let to_assign = after_assignments
            .into_iter()
            .filter(|a| a.id.is_some_and(|id| !before_assignments.contains(&id)))
            .map(|a| HunkAssignmentRequest {
                hunk_header: a.hunk_header,
                path_bytes: a.path_bytes,
                stack_id: Some(stack_id),
            })
            .collect::<Vec<_>>();

        but_hunk_assignment::assign(&mut command_ctx, to_assign, None)?;
    }

    Ok(serde_json::to_value(UIMoveChangesResult::from(result))?)
}

pub fn stash_into_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: StashIntoBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let repo = command_ctx.gix_repo_for_merging()?;

    let mut guard = project.exclusive_worktree_access();
    let perm = guard.write_permission();

    let _ = command_ctx.snapshot_stash_into_branch(params.branch_name.clone(), perm);

    let branch_manager = command_ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(
        &gitbutler_branch::BranchCreateRequest {
            name: Some(params.branch_name.clone()),
            ..Default::default()
        },
        perm,
    )?;

    let parent_commit_id = stack.head_oid(&repo)?;
    let branch_name = stack.derived_name()?;

    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack.id),
        commit_engine::Destination::NewCommit {
            parent_commit_id: Some(parent_commit_id),
            message: "Mo-Stashed changes".into(),
            stack_segment: Some(StackSegmentId {
                stack_id: stack.id,
                segment_ref: format!("refs/heads/{branch_name}")
                    .try_into()
                    .map_err(anyhow::Error::from)?,
            }),
        },
        None,
        params.worktree_changes,
        ctx.app_settings.get()?.context_lines,
        perm,
    );

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &command_ctx)
        .context("failed to update gitbutler workspace")?;

    branch_manager.unapply(stack.id, perm, false, Vec::new())?;

    let outcome = outcome?;
    Ok(serde_json::to_value(
        commit_engine::ui::CreateCommitOutcome::from(outcome),
    )?)
}

pub fn canned_branch_name(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ProjectIdParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let template = gitbutler_stack::canned_branch_name(command_ctx.repo())?;
    let state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    let name = gitbutler_stack::Stack::next_available_name(
        &command_ctx.gix_repo()?,
        &state,
        template,
        false,
    )?;
    Ok(json!(name))
}

pub fn target_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: TargetCommitsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commits = but_workspace::log_target_first_parent(
        &command_ctx,
        params.last_commit_id.map(|id| id.into()),
        params.page_size.unwrap_or(30),
    )?;
    Ok(serde_json::to_value(commits)?)
}
