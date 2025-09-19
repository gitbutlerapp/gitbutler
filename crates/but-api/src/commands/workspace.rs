use std::collections::{BTreeSet, HashSet, VecDeque};

use crate::error::Error;
use crate::hex_hash::HexHash;
use anyhow::Context;
use but_api_macros::api_cmd;
use but_graph::VirtualBranchesTomlMetadata;
use but_graph::petgraph::Direction;
use but_hunk_assignment::HunkAssignmentRequest;
use but_settings::AppSettings;
use but_workspace::MoveChangesResult;
use but_workspace::commit_engine::StackSegmentId;
use but_workspace::{commit_engine, ui::StackEntry};
use gitbutler_branch_actions::{BranchManagerExt, update_workspace_commit};
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project::{Project, ProjectId};
use gitbutler_reference::{LocalRefname, Refname};
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use serde::Serialize;
use tracing::instrument;

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn head_info(project_id: ProjectId) -> Result<but_workspace::ui::RefInfo, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    let meta = ref_metadata_toml(ctx.project())?;
    but_workspace::head_info(
        &repo,
        &meta,
        but_workspace::ref_info::Options {
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: true,
        },
    )
    .map_err(Into::into)
    .and_then(|info| {
        but_workspace::ui::RefInfo::for_ui(info, &repo)
            .map(|ref_info| ref_info.pruned_to_entrypoint())
            .map_err(Into::into)
    })
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stacks(
    project_id: ProjectId,
    filter: Option<but_workspace::StacksFilter>,
) -> Result<Vec<StackEntry>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    if ctx.app_settings().feature_flags.ws3 {
        let meta = ref_metadata_toml(ctx.project())?;
        but_workspace::stacks_v3(&repo, &meta, filter.unwrap_or_default(), None)
    } else {
        but_workspace::stacks(&ctx, &project.gb_dir(), &repo, filter.unwrap_or_default())
    }
    .map_err(Into::into)
}

#[cfg(unix)]
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn show_graph_svg(project_id: ProjectId) -> Result<(), Error> {
    use but_settings::AppSettings;

    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let repo = ctx.gix_repo_local_only()?;
    let meta = ref_metadata_toml(&project)?;
    let mut graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_graph::init::Options {
            collect_tags: true,
            extra_target_commit_id: meta.data().default_target.as_ref().map(|t| t.sha),
            ..but_graph::init::Options::limited()
        },
    )?;
    // It's OK if it takes a while, prefer complete graphs.
    const LIMIT: usize = 5000;
    let mut to_remove = graph.num_segments().saturating_sub(LIMIT);
    if to_remove > 0 {
        tracing::warn!(
            "Pruning at most {to_remove} nodes from the bottom to assure 'dot' won't hang",
        );
        let mut next = VecDeque::new();
        next.extend(graph.base_segments());
        let mut seen = BTreeSet::new();
        while let Some(sidx) = next.pop_front() {
            if to_remove == 0 {
                break;
            }
            if let Some(s) = graph.node_weight(sidx)
                && (s.metadata.is_some() || s.sibling_segment_id.is_some())
            {
                continue;
            }
            next.extend(
                graph
                    .neighbors_directed(sidx, Direction::Incoming)
                    .filter(|n| seen.insert(*n)),
            );
            graph.remove_node(sidx);
            to_remove -= 1;
        }
        if to_remove != 0 {
            tracing::warn!("{to_remove} extra nodes were kept to keep vital portions of the graph");
        }
        graph.set_hard_limit_hit();
    }
    graph.open_as_svg();
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stack_details(
    project_id: ProjectId,
    stack_id: Option<StackId>,
) -> Result<but_workspace::ui::StackDetails, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(ctx.project())?;
        but_workspace::stack_details_v3(stack_id, &repo, &meta)
    } else {
        but_workspace::stack_details(
            &project.gb_dir(),
            stack_id.context("BUG(opt-stack-id)")?,
            &ctx,
        )
    }
    .map_err(Into::into)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn branch_details(
    project_id: ProjectId,
    branch_name: String,
    remote: Option<String>,
) -> Result<but_workspace::ui::BranchDetails, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    if ctx.app_settings().feature_flags.ws3 {
        let repo = ctx.gix_repo_for_merging_non_persisting()?;
        let meta = ref_metadata_toml(ctx.project())?;
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
        but_workspace::branch_details(&project.gb_dir(), &branch_name, remote.as_deref(), &ctx)
    }
    .map_err(Into::into)
}

/// Create a new commit with `message` on top of `parent_id` that contains all `changes`.
/// If `parent_id` is `None`, this API will infer the parent to be the head of the provided `stack_branch_name`.
/// `stack_id` is the stack that contains the `parent_id`, and it's fatal if that's not the case.
/// All `changes` are meant to be relative to the worktree.
/// Note that submodules *must* be provided as diffspec without hunks, as attempting to generate
/// hunks would fail.
/// `stack_branch_name` is the short name of the reference that the UI knows is present in a given segment.
/// It is necessary to insert the new commit into the right bucket.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn create_commit_from_worktree_changes(
    project_id: ProjectId,
    stack_id: StackId,
    parent_id: Option<HexHash>,
    worktree_changes: Vec<but_workspace::DiffSpec>,
    message: String,
    stack_branch_name: String,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());

    let outcome = commit_engine::create_commit_simple(
        &ctx,
        stack_id,
        parent_id.map(|id| id.into()),
        worktree_changes,
        message.clone(),
        stack_branch_name,
        guard.write_permission(),
    );

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });

    let outcome = outcome?;
    Ok(outcome.into())
}

/// Amend all `changes` to `commit_id`, keeping its commit message exactly as is.
/// `stack_id` is the stack that contains the `commit_id`, and it's fatal if that's not the case.
/// All `changes` are meant to be relative to the worktree.
/// Note that submodules *must* be provided as diffspec without hunks, as attempting to generate
/// hunks would fail.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn amend_commit_from_worktree_changes(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut guard = project.exclusive_worktree_access();
    let repo = but_core::open_repo_for_merging(project.worktree_path())?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::AmendCommit {
            commit_id: commit_id.into(),
            // TODO: Expose this in the UI for 'edit message' functionality.
            new_message: None,
        },
        None,
        worktree_changes,
        app_settings.context_lines,
        guard.write_permission(),
    )?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(outcome.into())
}

/// Discard all worktree changes that match the specs in `worktree_changes`.
///
/// If whole files should be discarded, be sure to not pass any [hunks](but_workspace::discard::ui::DiscardSpec::hunk_headers)
///
/// Returns the `worktree_changes` that couldn't be applied,
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn discard_worktree_changes(
    project_id: ProjectId,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<Vec<but_workspace::DiffSpec>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let repo = but_core::open_repo(project.worktree_path())?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );
    let refused = but_workspace::discard_workspace_changes(
        &repo,
        worktree_changes,
        ctx.app_settings().context_lines,
    )?;
    if !refused.is_empty() {
        tracing::warn!(?refused, "Failed to discard at least one hunk");
    }
    Ok(refused)
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

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn move_changes_between_commits(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_commit_id: HexHash,
    destination_stack_id: StackId,
    destination_commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
) -> Result<UIMoveChangesResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::AmendCommit),
        guard.write_permission(),
    );
    let result = but_workspace::move_changes_between_commits(
        &ctx,
        source_stack_id,
        source_commit_id.into(),
        destination_stack_id,
        destination_commit_id.into(),
        changes,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &ctx)?;

    Ok(result.into())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn split_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: Vec<String>,
) -> Result<UIMoveChangesResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let (_, move_changes_result) = but_workspace::split_branch(
        &ctx,
        source_stack_id,
        source_branch_name,
        new_branch_name.clone(),
        &file_changes_to_split_off,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &ctx)?;

    let refname = Refname::Local(LocalRefname::new(&new_branch_name, None));
    let branch_manager = ctx.branch_manager();
    branch_manager.create_virtual_branch_from_branch(
        &refname,
        None,
        None,
        guard.write_permission(),
    )?;

    Ok(move_changes_result.into())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn split_branch_into_dependent_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: Vec<String>,
) -> Result<UIMoveChangesResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::SplitBranch),
        guard.write_permission(),
    );

    let move_changes_result = but_workspace::split_into_dependent_branch(
        &ctx,
        source_stack_id,
        source_branch_name,
        new_branch_name.clone(),
        &file_changes_to_split_off,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &ctx)?;

    Ok(move_changes_result.into())
}

/// Uncommits the changes specified in the `diffspec`.
///
/// If `assign_to` is provided, the changes will be assigned to the stack
/// specified.
/// If `assign_to` is not provided, the changes will be unassigned.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn uncommit_changes(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
    assign_to: Option<StackId>,
) -> Result<UIMoveChangesResult, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );

    // If we want to assign the changes after uncommitting, we could try to do
    // something with the hunk headers, but this is not precise as the hunk
    // headers might have changed from what they were like when they were
    // committed.
    //
    // As such, we take all the old assignments, and all the new assignments from after the
    // uncommit, and find the ones that are not present in the old assignments.
    // We then convert those into assignment requests for the given stack.
    let before_assignments = if assign_to.is_some() {
        let changes = but_hunk_assignment::assignments_with_fallback(
            &mut ctx,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
        )?;
        Some(changes.0)
    } else {
        None
    };

    let result = but_workspace::remove_changes_from_commit_in_stack(
        &ctx,
        stack_id,
        commit_id.into(),
        changes,
        ctx.app_settings().context_lines,
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    update_workspace_commit(&vb_state, &ctx)?;

    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            &mut ctx,
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

        but_hunk_assignment::assign(&mut ctx, to_assign, None)?;
    }

    Ok(result.into())
}

/// This API allows the user to quickly "stash" a bunch of uncommitted changes - getting them out of the worktree.
/// Unlike the regular stash, the user specifies a new branch where those changes will be 'saved'/committed.
/// Immediately after the changes are committed, the branch is unapplied from the workspace, and the "stash" branch can be re-applied at a later time
/// In theory it should be possible to specify an existing "dumping" branch for this, but currently this endpoint expects a new branch.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stash_into_branch(
    project_id: ProjectId,
    branch_name: String,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let repo = ctx.gix_repo_for_merging()?;

    let mut guard = project.exclusive_worktree_access();
    let perm = guard.write_permission();

    let _ = ctx.snapshot_stash_into_branch(branch_name.clone(), perm);

    let branch_manager = ctx.branch_manager();
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
        ctx.app_settings().context_lines,
        perm,
    );

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &ctx)
        .context("failed to update gitbutler workspace")?;

    branch_manager.unapply(
        stack.id,
        perm,
        false,
        Vec::new(),
        ctx.app_settings().feature_flags.cv3,
    )?;

    let outcome = outcome?;
    Ok(outcome.into())
}

/// Returns a new available branch name based on a simple template - user_initials-branch-count
/// The main point of this is to be able to provide branch names that are not already taken.
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn canned_branch_name(project_id: ProjectId) -> Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    gitbutler_stack::Stack::next_available_name(&ctx.gix_repo()?, &state, template, false)
        .map_err(Into::into)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn target_commits(
    project_id: ProjectId,
    last_commit_id: Option<HexHash>,
    page_size: Option<usize>,
) -> Result<Vec<but_workspace::ui::Commit>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_workspace::log_target_first_parent(
        &ctx,
        last_commit_id.map(|id| id.into()),
        page_size.unwrap_or(30),
    )
    .map_err(Into::into)
}
