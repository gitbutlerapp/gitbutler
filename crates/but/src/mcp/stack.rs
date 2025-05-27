use std::path::Path;

use but_core::TreeChange;
use but_settings::AppSettings;
use but_workspace::{DiffSpec, StackId, commit_engine::StackSegmentId};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use super::project::{RepositoryOpenMode, project_from_path, repo_and_maybe_project};

pub fn add_checkpoint(project_dir: &str, prompt: &str) -> Result<String, anyhow::Error> {
    let project_dir_path = std::path::PathBuf::from(project_dir);
    let (repo, project) = repo_and_maybe_project(&project_dir_path, RepositoryOpenMode::Merge)?;

    let changes = to_whole_file_diffspec(but_core::diff::worktree_changes(&repo)?.changes);
    let (stack_segment_id, parent_commit_id) =
        get_or_create_checkpoint_destination(&project_dir_path)?;

    Ok("Checkpoint added successfully".to_string())
}

/// Find the destionation for the checkpoint commit.
///
/// If none is found, this will create one.
fn get_or_create_checkpoint_destination(
    current_dir: &Path,
) -> Result<(StackSegmentId, gix::ObjectId), anyhow::Error> {
    let stacks = crate::mcp::status::list_applied_stacks(current_dir)?;

    // TODO: We need a more robust way to determine the checkpoint branch.
    let checkpoint_ref = "checkpoint";
    let normalized_ref = normalize_stack_segment_ref(checkpoint_ref)?;
    let checkpoint_stack = stacks
        .iter()
        .find(|s| s.heads.iter().any(|h| h.name == checkpoint_ref))
        .unwrap_or_else(|| {
            &create_branch(None, checkpoint_ref, Some(checkpoint_ref), current_dir)?
        });

    let stack_segment_id = StackSegmentId {
        segment_ref: normalized_ref,
        stack_id: checkpoint_stack.id,
    };
    let parent_commit_id = checkpoint_stack.tip;
    Ok((stack_segment_id, parent_commit_id))
}

fn commit_with_project(
    repo: &gix::Repository,
    project: &Project,
    message: Option<&str>,
    parent_commit_id: gix::ObjectId,
    stack_segment_id: StackSegmentId,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<String> {
    let destination = but_workspace::commit_engine::Destination::NewCommit {
        parent_commit_id: Some(parent_commit_id),
        message: message.unwrap_or_default().to_owned(),
        stack_segment: Some(stack_segment_id),
    };

    let mut guard = project.exclusive_worktree_access();
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs_with_project(
        repo,
        project,
        None,
        destination,
        None,
        changes,
        0,
        guard.write_permission(),
    )?;

    let outcome = but_workspace::commit_engine::ui::CreateCommitOutcome::from(outcome);
    let json = serde_json::to_string_pretty(&outcome)?;

    Ok(json)
}

/// Create a new branch in the current project.
///
/// If `id` is provided, it will be used to add the branch to an existing stack.
/// If `id` is not provided, a new stack will be created with the branch.
pub fn create_branch(
    id: Option<StackId>,
    name: &str,
    description: Option<&str>,
    current_dir: &Path,
) -> anyhow::Result<but_workspace::ui::StackEntry> {
    let project = project_from_path(current_dir)?;
    // Enable v3 feature flags for the command context
    let app_settings = AppSettings {
        feature_flags: but_settings::app_settings::FeatureFlags {
            v3: true,
            // Keep this off until it caught up at least.
            ws3: false,
        },
        ..AppSettings::default()
    };

    let ctx = CommandContext::open(&project, app_settings)?;
    let repo = ctx.gix_repo()?;

    let stack_entry = match id {
        Some(id) => add_branch_to_stack(&ctx, id, name, description, project.clone(), &repo)?,
        None => create_stack_with_branch(&ctx, name, description)?,
    };

    Ok(stack_entry)
}

/// Create a new stack containing only a branch with the given name.
fn create_stack_with_branch(
    ctx: &CommandContext,
    name: &str,
    description: Option<&str>,
) -> anyhow::Result<but_workspace::ui::StackEntry> {
    let creation_request = gitbutler_branch::BranchCreateRequest {
        name: Some(name.to_string()),
        ..Default::default()
    };
    let stack_entry = gitbutler_branch_actions::create_virtual_branch(ctx, &creation_request)?;

    if description.is_some() {
        gitbutler_branch_actions::stack::update_branch_description(
            ctx,
            stack_entry.id,
            name.to_string(),
            description.map(ToOwned::to_owned),
        )?;
    }

    Ok(stack_entry)
}

/// Add a branch to an existing stack.
fn add_branch_to_stack(
    ctx: &CommandContext,
    stack_id: StackId,
    name: &str,
    description: Option<&str>,
    project: gitbutler_project::Project,
    repo: &gix::Repository,
) -> anyhow::Result<but_workspace::ui::StackEntry> {
    let creation_request = gitbutler_branch_actions::stack::CreateSeriesRequest {
        name: name.to_string(),
        description: description.map(|d| d.to_string()),
        target_patch: None,
        preceding_head: None,
    };

    gitbutler_branch_actions::stack::create_branch(ctx, stack_id, creation_request)?;
    let stack_entries = but_workspace::stacks(ctx, &project.gb_dir(), repo, Default::default())?;

    let stack_entry = stack_entries
        .into_iter()
        .find(|entry| entry.id == stack_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to find stack with ID: {stack_id}"))?;

    Ok(stack_entry)
}

/// Get the normalized branch reference
fn normalize_stack_segment_ref(
    stack_segment_ref: &str,
) -> Result<gix::refs::FullName, gix::refs::name::Error> {
    let full_name = if stack_segment_ref.starts_with("refs/heads/") {
        stack_segment_ref.to_string()
    } else {
        format!("refs/heads/{}", stack_segment_ref)
    };
    gix::refs::FullName::try_from(full_name)
}

/// Converts a vector of `TreeChange` into a vector of `DiffSpec`,
/// assuming that each change represents a whole file change.
fn to_whole_file_diffspec(changes: Vec<TreeChange>) -> Vec<DiffSpec> {
    changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect()
}
