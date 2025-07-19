use std::collections::HashSet;

use anyhow::Context;
use but_graph::VirtualBranchesTomlMetadata;
use but_hunk_assignment::HunkAssignmentRequest;
use but_workspace::commit_engine::StackSegmentId;
use but_workspace::MoveChangesResult;
use but_workspace::commit_engine;
use gitbutler_branch_actions::{update_workspace_commit, BranchManagerExt};
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_oxidize::OidExt;
use gitbutler_project::{Project, ProjectId};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use serde::Serialize;
use serde_json::{json, Value};

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

pub fn stacks(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let filter: Option<but_workspace::StacksFilter> = serde_json::from_value(params.get("filter").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let repo = command_ctx.gix_repo_for_merging_non_persisting()?;
    
    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let meta = ref_metadata_toml(command_ctx.project())?;
        but_workspace::stacks_v3(&repo, &meta, filter.unwrap_or_default())
    } else {
        but_workspace::stacks(&command_ctx, &project.gb_dir(), &repo, filter.unwrap_or_default())
    }?;
    
    Ok(serde_json::to_value(result)?)
}

#[cfg(unix)]
pub fn show_graph_svg(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
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
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    
    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let repo = command_ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(command_ctx.project())?;
        but_workspace::stack_details_v3(stack_id, &repo, &meta)
    } else {
        but_workspace::stack_details(&project.gb_dir(), stack_id, &command_ctx)
    }?;
    
    Ok(serde_json::to_value(result)?)
}

pub fn branch_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    let remote: Option<String> = serde_json::from_value(params.get("remote").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    
    let result = if command_ctx.app_settings().feature_flags.ws3 {
        let repo = command_ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(command_ctx.project())?;
        let ref_name: gix::refs::FullName = match remote.as_deref() {
            None => {
                format!("refs/heads/{branch_name}")
            }
            Some(remote) => {
                format!("refs/remotes/{remote}/{branch_name}")
            }
        }
        .try_into()
        .map_err(anyhow::Error::from)?;
        but_workspace::branch_details_v3(&repo, ref_name.as_ref(), &meta)
    } else {
        but_workspace::branch_details(&project.gb_dir(), &branch_name, remote.as_deref(), &command_ctx)
    }?;
    
    Ok(serde_json::to_value(result)?)
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

pub fn create_commit_from_worktree_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let parent_id: Option<HexHash> = serde_json::from_value(params.get("parentId").cloned().unwrap_or_default())?;
    let worktree_changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("worktreeChanges").cloned().unwrap_or_default())?;
    let message: String = serde_json::from_value(params.get("message").cloned().unwrap_or_default())?;
    let stack_branch_name: String = serde_json::from_value(params.get("stackBranchName").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = command_ctx.prepare_snapshot(guard.read_permission());

    let outcome = commit_engine::create_commit_simple(
        &command_ctx,
        stack_id,
        parent_id.map(|id| id.into()),
        worktree_changes,
        message.clone(),
        stack_branch_name,
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        command_ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });

    let outcome = outcome?;
    Ok(serde_json::to_value(commit_engine::ui::CreateCommitOutcome::from(outcome))?)
}

pub fn amend_commit_from_worktree_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: HexHash = serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let worktree_changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("worktreeChanges").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let mut guard = project.exclusive_worktree_access();
    let repo = but_core::open_repo_for_merging(project.worktree_path())?;
    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::AmendCommit {
            commit_id: commit_id.into(),
            new_message: None,
        },
        None,
        worktree_changes,
        ctx.app_settings.get()?.context_lines,
        guard.write_permission(),
    )?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(serde_json::to_value(commit_engine::ui::CreateCommitOutcome::from(outcome))?)
}

pub fn discard_worktree_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let worktree_changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("worktreeChanges").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let repo = but_core::open_repo(project.worktree_path())?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );
    let refused = but_workspace::discard_workspace_changes(
        &repo,
        worktree_changes,
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
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let source_stack_id: StackId = serde_json::from_value(params.get("sourceStackId").cloned().unwrap_or_default())?;
    let source_commit_id: HexHash = serde_json::from_value(params.get("sourceCommitId").cloned().unwrap_or_default())?;
    let destination_stack_id: StackId = serde_json::from_value(params.get("destinationStackId").cloned().unwrap_or_default())?;
    let destination_commit_id: HexHash = serde_json::from_value(params.get("destinationCommitId").cloned().unwrap_or_default())?;
    let changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("changes").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::AmendCommit),
        guard.write_permission(),
    );
    let result = but_workspace::move_changes_between_commits(
        &command_ctx,
        source_stack_id,
        source_commit_id.into(),
        destination_stack_id,
        destination_commit_id.into(),
        changes,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(result))?)
}

pub fn split_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let source_stack_id: StackId = serde_json::from_value(params.get("sourceStackId").cloned().unwrap_or_default())?;
    let source_branch_name: String = serde_json::from_value(params.get("sourceBranchName").cloned().unwrap_or_default())?;
    let new_branch_name: String = serde_json::from_value(params.get("newBranchName").cloned().unwrap_or_default())?;
    let file_changes_to_split_off: Vec<String> = serde_json::from_value(params.get("fileChangesToSplitOff").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let (_, move_changes_result) = but_workspace::split_branch(
        &command_ctx,
        source_stack_id,
        source_branch_name,
        new_branch_name.clone(),
        &file_changes_to_split_off,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    let refname = Refname::Local(LocalRefname::new(&new_branch_name, None));
    let branch_manager = command_ctx.branch_manager();
    branch_manager.create_virtual_branch_from_branch(
        &refname,
        None,
        None,
        guard.write_permission(),
    )?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(move_changes_result))?)
}

pub fn split_branch_into_dependent_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let source_stack_id: StackId = serde_json::from_value(params.get("sourceStackId").cloned().unwrap_or_default())?;
    let source_branch_name: String = serde_json::from_value(params.get("sourceBranchName").cloned().unwrap_or_default())?;
    let new_branch_name: String = serde_json::from_value(params.get("newBranchName").cloned().unwrap_or_default())?;
    let file_changes_to_split_off: Vec<String> = serde_json::from_value(params.get("fileChangesToSplitOff").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let move_changes_result = but_workspace::split_into_dependent_branch(
        &command_ctx,
        source_stack_id,
        source_branch_name,
        new_branch_name.clone(),
        &file_changes_to_split_off,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    Ok(serde_json::to_value(UIMoveChangesResult::from(move_changes_result))?)
}

pub fn uncommit_changes(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let stack_id: StackId = serde_json::from_value(params.get("stackId").cloned().unwrap_or_default())?;
    let commit_id: HexHash = serde_json::from_value(params.get("commitId").cloned().unwrap_or_default())?;
    let changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("changes").cloned().unwrap_or_default())?;
    let assign_to: Option<StackId> = serde_json::from_value(params.get("assignTo").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let mut command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = command_ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );

    let before_assignments = if assign_to.is_some() {
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
        stack_id,
        commit_id.into(),
        changes,
        ctx.app_settings.get()?.context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &command_ctx)?;

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
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
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let branch_name: String = serde_json::from_value(params.get("branchName").cloned().unwrap_or_default())?;
    let worktree_changes: Vec<but_workspace::DiffSpec> = serde_json::from_value(params.get("worktreeChanges").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let repo = command_ctx.gix_repo_for_merging()?;

    let mut guard = project.exclusive_worktree_access();
    let perm = guard.write_permission();

    let _ = command_ctx.snapshot_stash_into_branch(branch_name.clone(), perm);

    let branch_manager = command_ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(
        &gitbutler_branch::BranchCreateRequest {
            name: Some(branch_name.clone()),
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
        worktree_changes,
        ctx.app_settings.get()?.context_lines,
        perm,
    );

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &command_ctx)
        .context("failed to update gitbutler workspace")?;

    branch_manager.unapply(stack.id, perm, false, Vec::new())?;

    let outcome = outcome?;
    Ok(serde_json::to_value(commit_engine::ui::CreateCommitOutcome::from(outcome))?)
}

pub fn canned_branch_name(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let template = gitbutler_stack::canned_branch_name(command_ctx.repo())?;
    let state = VirtualBranchesHandle::new(command_ctx.project().gb_dir());
    let name = gitbutler_stack::Stack::next_available_name(&command_ctx.gix_repo()?, &state, template, false)?;
    Ok(json!(name))
}

pub fn target_commits(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let project_id: ProjectId = serde_json::from_value(params.get("projectId").cloned().unwrap_or_default())?;
    let last_commit_id: Option<HexHash> = serde_json::from_value(params.get("lastCommitId").cloned().unwrap_or_default())?;
    let page_size: Option<usize> = serde_json::from_value(params.get("pageSize").cloned().unwrap_or_default())?;
    
    let project = ctx.project_controller.get(project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let commits = but_workspace::log_target_first_parent(
        &command_ctx,
        last_commit_id.map(|id| id.into()),
        page_size.unwrap_or(30),
    )?;
    Ok(serde_json::to_value(commits)?)
}