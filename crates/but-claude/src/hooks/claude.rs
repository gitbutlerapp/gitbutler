use std::collections::HashMap;
use std::io::{self, Read};
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use but_action::rename_branch::RenameBranchParams;
use but_action::{ActionHandler, OpenAiProvider, Source, reword::CommitEvent};
use but_db::ClaudeCodeSession;
use but_hunk_assignment::HunkAssignmentRequest;
use but_settings::AppSettings;
use but_workspace::{HunkHeader, StackId};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, access::WorktreeWritePermission};
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

use crate::command::file_lock;

use super::claude_transcript::Transcript;

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

pub(crate) async fn handle_stop() -> anyhow::Result<ClaudeHookOutput> {
    let input: ClaudeStopInput = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;
    let transcript = Transcript::from_file(input.transcript_path)?;
    let cwd = transcript.dir()?;
    let repo = gix::discover(cwd)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow!("No worktree found for repo"))?,
    )?;

    let summary = transcript.summary().unwrap_or_default();
    let prompt = transcript.prompt().unwrap_or_default();

    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let defer = ClearLocksGuard {
        ctx,
        session_id: input.session_id.clone(),
        file_path: None,
    };

    let vb_state = &VirtualBranchesHandle::new(defer.ctx.project().gb_dir());

    let stacks = crate::log::stacks(defer.ctx)?;

    // If the session stopped, but there's no session persisted in the database, we create a new one.
    // If the session is already persisted, we just retrieve it.
    let (_, stack_id) = get_or_create_session(defer.ctx, &input.session_id, stacks, vb_state)?;

    let (id, outcome) = but_action::handle_changes(
        defer.ctx,
        &summary,
        Some(prompt.clone()),
        ActionHandler::HandleChangesSimple,
        Source::ClaudeCode(input.session_id),
        Some(stack_id),
    )?;

    let stacks = crate::log::stacks(defer.ctx)?;

    // Trigger commit message generation for newly created commits
    // TODO: Maybe this can be done in the main app process i.e. the GitButler GUI, if avaialbe
    // Alternatively, and probably better - we could spawn a new process to do this

    if let Some(openai_client) =
        OpenAiProvider::with(None).and_then(|provider| provider.client().ok())
    {
        for branch in &outcome.updated_branches {
            let mut commit_message_mapping = HashMap::new();

            let elegibility = is_branch_eligible_for_rename(&defer, &stacks, branch)?;

            for commit in &branch.new_commits {
                if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                    let commit_event = CommitEvent {
                        external_summary: summary.clone(),
                        external_prompt: prompt.clone(),
                        branch_name: branch.branch_name.clone(),
                        commit_id,
                        project: project.clone(),
                        app_settings: defer.ctx.app_settings().clone(),
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

            match elegibility {
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
                        .ok();
                    }
                }
                RenameEligibility::NotEligible => {
                    // Do nothing, branch is not eligible for renaming
                }
            }
        }
    }

    // For now, we just return a response indicating that the tool call was handled
    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

enum RenameEligibility {
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
fn is_branch_eligible_for_rename(
    defer: &ClearLocksGuard<'_>,
    stacks: &[but_workspace::ui::StackEntry],
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
    let details =
        crate::log::stack_details(defer.ctx, stack_entry.id.context("BUG(opt-stack-id)")?)?;
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

pub(crate) fn handle_pre_tool_call() -> anyhow::Result<ClaudeHookOutput> {
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
        .strip_prefix(project.path.clone())?
        .to_string_lossy()
        .to_string();
    input.tool_input.file_path = relative_file_path;

    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    file_lock::obtain(ctx, input.session_id, input.tool_input.file_path.clone())?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

pub(crate) fn handle_post_tool_call() -> anyhow::Result<ClaudeHookOutput> {
    let mut input: ClaudePostToolUseInput = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;
    let dir = std::path::Path::new(&input.tool_response.file_path)
        .parent()
        .ok_or(anyhow!("Failed to get parent directory of file path"))?;
    let repo = gix::discover(dir)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow!("No worktree found for repo"))?,
    )?;

    let relative_file_path = std::path::PathBuf::from(&input.tool_response.file_path)
        .strip_prefix(project.path.clone())?
        .to_string_lossy()
        .to_string();
    input.tool_response.file_path = relative_file_path;

    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let defer = ClearLocksGuard {
        ctx,
        session_id: input.session_id.clone(),
        file_path: Some(input.tool_response.file_path.clone()),
    };

    let stacks = crate::log::stacks(defer.ctx)?;

    let vb_state = &VirtualBranchesHandle::new(defer.ctx.project().gb_dir());

    let (_, stack_id) = get_or_create_session(defer.ctx, &input.session_id, stacks, vb_state)?;

    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(project.path)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        defer.ctx,
        true,
        Some(changes.clone()),
        None,
    )?;

    let hook_headers = input
        .tool_response
        .structured_patch
        .iter()
        .map(|p| p.into())
        .collect::<Vec<HunkHeader>>();
    let assignment_reqs: Vec<HunkAssignmentRequest> = assignments
        .into_iter()
        .filter(|a| a.stack_id.is_none())
        .filter(|a| {
            if let Some(a) = a.hunk_header {
                hook_headers
                    .iter()
                    .any(|h| h.new_range().intersects(a.new_range()))
            } else {
                true // If no header is present, then the whole file is considered, in which case intersection is true
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

fn get_or_create_session(
    ctx: &mut CommandContext,
    session_id: &str,
    stacks: Vec<but_workspace::ui::StackEntry>,
    vb_state: &VirtualBranchesHandle,
) -> Result<(ClaudeCodeSession, StackId), anyhow::Error> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let sessions = list_sessions(ctx)?;
    let session_and_stack_id = if let Some(session) = sessions.iter().find(|s| s.id == session_id) {
        // If the stack referenced by the session is in the list of applied stacks do nothing
        // Otherwise, create a new stack and update the session
        if let Some(stack_id) = stacks.iter().find_map(|s| {
            let id = s.id?;
            (id.to_string() == session.stack_id).then_some(id)
        }) {
            (session.to_owned(), stack_id)
        } else {
            let stack_id = create_stack(ctx, vb_state, perm)?;
            ctx.db()?
                .claude_code_sessions()
                .update_stack_id(session_id, &stack_id.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to update session stack ID: {}", e))?;
            (session.to_owned(), stack_id)
        }
    } else {
        // If the session is not in the list of sessions, then create a new stack + session entry
        // Create a new stack
        let stack_id = create_stack(ctx, vb_state, perm)?;
        let new_session = ClaudeCodeSession {
            id: session_id.to_owned(),
            created_at: chrono::Local::now().naive_local(),
            stack_id: stack_id.to_string(),
        };
        ctx.db()?
            .claude_code_sessions()
            .insert(new_session.clone())
            .map_err(|e| anyhow::anyhow!("Failed to insert new session: {}", e))?;
        (new_session, stack_id)
    };
    Ok(session_and_stack_id)
}

fn stdin() -> anyhow::Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn create_stack(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<StackId> {
    let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
    let branch_name =
        gitbutler_stack::Stack::next_available_name(&ctx.gix_repo()?, vb_state, template, false)?;
    let create_req = BranchCreateRequest {
        name: Some(branch_name),
        ownership: None,
        order: None,
        selected_for_changes: None,
    };
    let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
    Ok(stack.id)
}

fn list_sessions(ctx: &mut CommandContext) -> anyhow::Result<Vec<ClaudeCodeSession>> {
    let sessions = ctx
        .db()?
        .claude_code_sessions()
        .list()
        .map_err(|e| anyhow::anyhow!("Failed to list Claude code sessions: {}", e))?;
    Ok(sessions)
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
    pub ctx: &'a mut CommandContext,
    session_id: String,
    file_path: Option<String>,
}

impl Drop for ClearLocksGuard<'_> {
    fn drop(&mut self) {
        file_lock::clear(self.ctx, self.session_id.clone(), self.file_path.clone()).ok();
    }
}

pub trait OutputAsJson {
    fn out_json(&self);
}

impl OutputAsJson for Result<ClaudeHookOutput> {
    fn out_json(&self) {
        match self {
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
    }
}
