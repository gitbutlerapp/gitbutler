use std::{
    collections::HashMap,
    io::{self, Read},
    path::Path,
    str::FromStr,
};

use anyhow::{Context as _, Result, anyhow};
use but_action::{
    ActionHandler, OpenAiProvider, Source, rename_branch::RenameBranchParams, reword::CommitEvent,
};
use but_ctx::{Context, access::WorktreeWritePermission};
use but_hunk_assignment::HunkAssignmentRequest;
use but_meta::VirtualBranchesTomlMetadata;
use but_workspace::{
    legacy::{StacksFilter, ui::StackEntry},
    ui::StackDetails,
};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_project::Project;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

// use crate::command::file_lock;

mod file_lock;
use but_core::{HunkHeader, ref_metadata::StackId};
use uuid::Uuid;

use crate::claude_transcript::Transcript;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudePostToolUseInput {
    pub session_id: String,
    pub transcript_path: String,
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
    pub tool_response: ToolResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolInput {
    pub file_path: String,
    pub new_string: Option<String>,
    pub old_string: Option<String>,
    pub replace_all: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponse {
    pub file_path: String,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub original_file: Option<String>,
    /// The hunk headers can't be trusted - it seems like:
    ///    - they cont account for the hunk context lines
    ///    - the new lines are not always correct
    pub structured_patch: Vec<StructuredPatch>,
    pub user_modified: Option<bool>,
    pub replace_all: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredPatch {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<String>,
}

impl From<&StructuredPatch> for HunkHeader {
    fn from(patch: &StructuredPatch) -> Self {
        HunkHeader {
            old_start: patch.old_start,
            old_lines: patch.old_lines,
            new_start: patch.new_start,
            new_lines: patch.new_lines,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeStopInput {
    pub session_id: String,
    pub transcript_path: String,
    pub hook_event_name: String,
    pub stop_hook_active: Option<bool>,
}

pub async fn handle_stop() -> anyhow::Result<ClaudeHookOutput> {
    let input: ClaudeStopInput = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;

    let transcript = Transcript::from_file(Path::new(&input.transcript_path))?;
    let cwd = transcript.dir()?;
    let repo = gix::discover(cwd)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow!("No worktree found for repo"))?,
    )?;

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(
        project.clone().worktree_dir()?.into(),
    )?
    .changes;

    // This is a naive way of handling this case.
    // If the user simply asks a question and there are no changes, we don't need to create a stack
    // nor handle any changes.
    // This should handle **most** cases, but there might be some edge cases where this is not sufficient.
    // TODO: Be smarter about this. We could try checking the transcript for any changes associated with this session,
    // that are not committed yet. And only if they are present, we proceed with the changes handling.
    if changes.is_empty() {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "No changes detected".to_string(),
            suppress_output: false,
        });
    }

    let summary = transcript.summary().unwrap_or_default();
    let prompt = transcript.prompt().unwrap_or_default();

    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let session_id = original_session_id(ctx, input.session_id.clone())?;

    if should_exit_early(ctx, &input.session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    let defer = ClearLocksGuard {
        ctx,
        session_id: session_id.clone(),
        file_path: None,
    };

    if !defer.ctx.settings().claude.auto_commit_after_completion {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "No after-hook behaviour required.".to_string(),
            suppress_output: true,
        });
    }

    let vb_state = &VirtualBranchesHandle::new(defer.ctx.project_data_dir());

    let stacks = list_stacks(defer.ctx)?;

    // If the session stopped, but there's no session persisted in the database, we create a new one.
    // If the session is already persisted, we just retrieve it.
    let stack_id = get_or_create_session(defer.ctx, &session_id, stacks, vb_state)?;

    let (id, outcome) = but_action::handle_changes(
        defer.ctx,
        &summary,
        Some(prompt.clone()),
        ActionHandler::HandleChangesSimple,
        Source::ClaudeCode(session_id.clone()),
        Some(stack_id),
    )?;

    let stacks = list_stacks(defer.ctx)?;

    // Trigger commit message generation for newly created commits
    // TODO: Maybe this can be done in the main app process i.e. the GitButler GUI, if available
    // Alternatively, and probably better - we could spawn a new process to do this

    if let Some(openai_client) =
        OpenAiProvider::with(None).and_then(|provider| provider.client().ok())
    {
        for branch in &outcome.updated_branches {
            let mut commit_message_mapping = HashMap::new();

            let eligibility = is_branch_eligible_for_rename(defer.ctx, &stacks, branch)?;

            for commit in &branch.new_commits {
                if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                    let commit_event = CommitEvent {
                        external_summary: summary.clone(),
                        external_prompt: prompt.clone(),
                        branch_name: branch.branch_name.clone(),
                        commit_id,
                        project: project.clone(),
                        app_settings: defer.ctx.settings().clone(),
                        trigger: id,
                    };
                    let reword_result = but_action::reword::commit(&openai_client, commit_event)
                        .await
                        .ok()
                        .unwrap_or_default();

                    // Update the commit mapping with the new commit ID
                    if let Some(reword_result) = reword_result {
                        commit_message_mapping.insert(commit_id, reword_result);
                    }
                }
            }

            let final_branch_name = match eligibility {
                RenameEligibility::Eligible { commit_id } => {
                    let reword_result = commit_message_mapping.get(&commit_id).cloned();

                    if let Some((commit_id, commit_message)) = reword_result {
                        let params = RenameBranchParams {
                            commit_id,
                            commit_message,
                            stack_id: branch.stack_id,
                            current_branch_name: branch.branch_name.clone(),
                        };
                        but_action::rename_branch::rename_branch(
                            defer.ctx,
                            &openai_client,
                            params,
                            id,
                        )
                        .await
                        .ok()
                        .unwrap_or_else(|| branch.branch_name.clone())
                    } else {
                        branch.branch_name.clone()
                    }
                }
                RenameEligibility::NotEligible => branch.branch_name.clone(),
            };

            // Build final commit IDs list - using reworded IDs if available, original otherwise
            let final_commit_ids: Vec<String> = branch
                .new_commits
                .iter()
                .map(|commit| {
                    if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                        commit_message_mapping
                            .get(&commit_id)
                            .map(|(new_id, _)| new_id.to_string())
                            .unwrap_or_else(|| commit.clone())
                    } else {
                        commit.clone()
                    }
                })
                .collect();

            // Write commit notification messages to the database
            // These will be broadcasted by the main process after Claude completes
            let session_uuid = uuid::Uuid::parse_str(&session_id)?;
            let commit_message = crate::MessagePayload::GitButler(
                crate::GitButlerUpdate::CommitCreated(crate::CommitCreatedDetails {
                    stack_id: Some(branch.stack_id.to_string()),
                    branch_name: Some(final_branch_name),
                    commit_ids: Some(final_commit_ids),
                }),
            );

            crate::db::save_new_message(defer.ctx, session_uuid, commit_message)?;
        }
    }

    // For now, we just return a response indicating that the tool call was handled
    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

pub enum RenameEligibility {
    Eligible { commit_id: gix::ObjectId },
    NotEligible,
}

/// Determines whether a branch can and should be renamed based on the current state of the stack and the branch.
///
/// The conditions for renaming a branch are:
/// - The branch has exactly one commit.
/// - The branch is unpushed.
///
/// ## Intention
///
/// The intention behind this implementation is to ensure that the more costly operation (getting the stack details)
/// is only performed if necessary.
/// This is determined by first checking if the newly added commits are only one and the branch tip matches the commit ID.
pub fn is_branch_eligible_for_rename(
    ctx: &Context,
    stacks: &[but_workspace::legacy::ui::StackEntry],
    branch: &but_action::UpdatedBranch,
) -> Result<RenameEligibility, anyhow::Error> {
    // Find the stack entry for this branch
    let stack_entry = stacks
        .iter()
        .find(|s| s.id == Some(branch.stack_id))
        .ok_or_else(|| anyhow::anyhow!("Stack not found"))?;

    // Only eligible if exactly one new commit
    if branch.new_commits.len() != 1 {
        return Ok(RenameEligibility::NotEligible);
    }
    let commit_id = &branch.new_commits[0];

    // Find the branch head in the stack
    let branch_head = stack_entry
        .heads
        .iter()
        .find(|h| h.name == branch.branch_name)
        .ok_or_else(|| anyhow::anyhow!("Branch head not found"))?;

    // Commit id must match branch tip
    if gix::ObjectId::from_str(commit_id)? != branch_head.tip {
        return Ok(RenameEligibility::NotEligible);
    }

    // Get stack details and branch details
    let details = stack_details(ctx, stack_entry.id.context("BUG(opt-stack-id)")?)?;
    let branch_details = details
        .branch_details
        .iter()
        .find(|b| b.name == branch.branch_name)
        .ok_or_else(|| anyhow::anyhow!("Branch details not found"))?;

    // Must have exactly one commit and be unpushed
    if branch_details.commits.len() == 1
        && matches!(
            branch_details.push_status,
            but_workspace::ui::PushStatus::CompletelyUnpushed
        )
    {
        Ok(RenameEligibility::Eligible {
            commit_id: branch_head.tip,
        })
    } else {
        Ok(RenameEligibility::NotEligible)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudePreToolUseInput {
    pub session_id: String,
    pub transcript_path: String,
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
}

pub fn handle_pre_tool_call() -> anyhow::Result<ClaudeHookOutput> {
    let mut input: ClaudePreToolUseInput = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;

    let dir = std::path::Path::new(&input.tool_input.file_path)
        .parent()
        .ok_or(anyhow!("Failed to get parent directory of file path"))?;
    let repo = gix::discover(dir)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow!("No worktree found for repo"))?,
    )?;
    let relative_file_path = std::path::PathBuf::from(&input.tool_input.file_path)
        .strip_prefix(project.worktree_dir()?)?
        .to_string_lossy()
        .to_string();
    input.tool_input.file_path = relative_file_path;

    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let session_id = original_session_id(ctx, input.session_id.clone())?;

    if should_exit_early(ctx, &input.session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    file_lock::obtain(ctx, session_id, input.tool_input.file_path.clone())?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

pub fn handle_post_tool_call() -> anyhow::Result<ClaudeHookOutput> {
    let mut input: ClaudePostToolUseInput = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;

    let hook_headers = input
        .tool_response
        .structured_patch
        .iter()
        .map(|p| p.into())
        .collect::<Vec<HunkHeader>>();

    let dir = std::path::Path::new(&input.tool_response.file_path)
        .parent()
        .ok_or(anyhow!("Failed to get parent directory of file path"))?;
    let repo = gix::discover(dir)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow!("No worktree found for repo"))?,
    )?;

    let relative_file_path = std::path::PathBuf::from(&input.tool_response.file_path)
        .strip_prefix(project.worktree_dir()?)?
        .to_string_lossy()
        .to_string();
    input.tool_response.file_path = relative_file_path.clone();

    let ctx = &mut Context::new_from_legacy_project(project.clone())?;

    if should_exit_early(ctx, &input.session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    let session_id = original_session_id(ctx, input.session_id.clone())?;

    let defer = ClearLocksGuard {
        ctx,
        session_id: session_id.clone(),
        file_path: Some(input.tool_response.file_path.clone()),
    };

    let stacks = list_stacks(defer.ctx)?;

    let vb_state = &VirtualBranchesHandle::new(defer.ctx.project_data_dir());

    let stack_id = get_or_create_session(defer.ctx, &session_id, stacks, vb_state)?;

    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.worktree_dir()?.into())?
            .changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        defer.ctx,
        true,
        Some(changes.clone()),
        None,
    )?;

    let assignment_reqs: Vec<HunkAssignmentRequest> = assignments
        .into_iter()
        .filter(|a| a.stack_id.is_none())
        .filter(|a| {
            // If the hook_headers is empty, we probably created a file.
            if hook_headers.is_empty() {
                a.path.to_lowercase() == relative_file_path.to_lowercase()
            } else if a.path.to_lowercase() == relative_file_path.to_lowercase() {
                if let Some(a) = a.hunk_header {
                    hook_headers
                        .iter()
                        .any(|h| h.new_range().intersects(a.new_range()))
                } else {
                    true // If no header is present, then the whole file is considered, in which case intersection is true
                }
            } else {
                false
            }
        })
        .map(|a| HunkAssignmentRequest {
            hunk_header: a.hunk_header,
            path_bytes: a.path_bytes,
            stack_id: Some(stack_id),
        })
        .collect();

    let _rejections = but_hunk_assignment::assign(defer.ctx, assignment_reqs, None)?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

fn original_session_id(ctx: &mut Context, current_id: String) -> Result<String> {
    let original_session_id =
        crate::db::get_session_by_current_id(ctx, Uuid::parse_str(&current_id)?)?;
    if let Some(session) = original_session_id {
        Ok(session.id.to_string())
    } else {
        Ok(current_id)
    }
}

pub fn get_or_create_session(
    ctx: &mut Context,
    session_id: &str,
    stacks: Vec<but_workspace::legacy::ui::StackEntry>,
    vb_state: &VirtualBranchesHandle,
) -> Result<StackId, anyhow::Error> {
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();

    if crate::db::get_session_by_id(ctx, Uuid::parse_str(session_id)?)?.is_none() {
        crate::db::save_new_session(ctx, Uuid::parse_str(session_id)?)?;
    }

    let stack_id = if let Some(rule) = crate::rules::list_claude_assignment_rules(ctx)?
        .into_iter()
        .find(|r| r.session_id.to_string() == session_id)
    {
        if let Some(stack_id) = stacks.iter().find_map(|s| {
            let id = s.id?;
            (id == rule.stack_id).then_some(id)
        }) {
            stack_id
        } else {
            let stack_id = create_stack(ctx, vb_state, perm)?;
            crate::rules::update_claude_assignment_rule_target(ctx, rule.id, stack_id)?;
            stack_id
        }
    } else {
        // If the session is not in the list of sessions, then create a new stack + session entry
        // Create a new stack
        let stack_id = create_stack(ctx, vb_state, perm)?;
        crate::rules::create_claude_assignment_rule(ctx, Uuid::parse_str(session_id)?, stack_id)?;
        stack_id
    };
    Ok(stack_id)
}

fn stdin() -> anyhow::Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn create_stack(
    ctx: &Context,
    vb_state: &VirtualBranchesHandle,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<StackId> {
    let template = gitbutler_stack::canned_branch_name(&*ctx.git2_repo.get()?)?;
    let branch_name =
        gitbutler_stack::Stack::next_available_name(&*ctx.repo.get()?, vb_state, template, false)?;
    let create_req = BranchCreateRequest {
        name: Some(branch_name),
        ownership: None,
        order: None,
        selected_for_changes: None,
    };
    let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
    Ok(stack.id)
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHookOutput {
    #[serde(rename = "continue")]
    do_continue: bool,
    stop_reason: String,
    suppress_output: bool,
}

pub(crate) struct ClearLocksGuard<'a> {
    pub ctx: &'a mut Context,
    session_id: String,
    file_path: Option<String>,
}

impl Drop for ClearLocksGuard<'_> {
    fn drop(&mut self) {
        file_lock::clear(self.ctx, self.session_id.clone(), self.file_path.clone()).ok();
    }
}

pub trait OutputClaudeJson {
    fn output_claude_json(self) -> Self;
}

impl OutputClaudeJson for Result<ClaudeHookOutput> {
    fn output_claude_json(self) -> Self {
        match &self {
            Ok(output) => println!("{}", serde_json::to_string(output).unwrap_or_default()),
            Err(e) => eprintln!(
                "{}",
                serde_json::to_string(&ClaudeHookOutput {
                    do_continue: false,
                    stop_reason: e.to_string(),
                    suppress_output: false,
                })
                .unwrap_or_default()
            ),
        }
        self
    }
}

fn stack_details(ctx: &Context, stack_id: StackId) -> anyhow::Result<StackDetails> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project_data_dir().join("virtual_branches.toml"),
    )?;
    but_workspace::legacy::stack_details_v3(Some(stack_id), &repo, &meta)
}

fn list_stacks(ctx: &Context) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project_data_dir().join("virtual_branches.toml"),
    )?;
    but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
}

/// Returns true if the session has `is_gui` set to true, and `GUTBUTLER_IN_GUI` is unset
fn should_exit_early(ctx: &mut Context, session_id: &str) -> anyhow::Result<bool> {
    let in_gui = std::env::var("GITBUTLER_IN_GUI").unwrap_or("0".into()) == "1";
    if in_gui {
        return Ok(false);
    }

    let session_uuid = Uuid::parse_str(session_id)?;
    if let Ok(Some(session)) = crate::db::get_session_by_current_id(ctx, session_uuid) {
        return Ok(session.in_gui);
    }

    Ok(false)
}
