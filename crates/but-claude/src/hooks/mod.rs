use std::{collections::HashMap, path::Path, str::FromStr};

use anyhow::{Context as _, Result};
use but_action::{ActionHandler, Source, rename_branch::RenameBranchParams, reword::CommitEvent};
use but_ctx::{Context, access::RepoExclusive};
use but_hunk_assignment::HunkAssignmentRequest;
use but_llm::LLMProvider;
use but_workspace::{
    legacy::{StacksFilter, ui::StackEntry},
    ui::StackDetails,
};
use gitbutler_branch::BranchCreateRequest;
use serde::{Deserialize, Serialize};

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

pub fn handle_stop(ctx: Context, read: impl std::io::Read) -> anyhow::Result<ClaudeHookOutput> {
    let input: ClaudeStopInput =
        serde_json::from_reader(read).map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;

    handle_session_stop(ctx, &input.session_id, &input.transcript_path, false)
}

pub fn handle_session_stop(
    mut ctx: Context,
    session_id: &str,
    transcript_path: &str,
    skip_gui_check: bool,
) -> anyhow::Result<ClaudeHookOutput> {
    let resolved_session_id = original_session_id(&mut ctx, session_id.to_string())?;

    // ClearLocksGuard ensures all file locks for this session are cleared on drop,
    // including early returns (no changes, GUI check, auto-commit disabled).
    let mut defer = ClearLocksGuard {
        ctx,
        session_id: resolved_session_id.clone(),
        file_path: None,
    };

    let transcript = Transcript::from_file(Path::new(transcript_path))?;
    let changes = but_core::diff::ui::worktree_changes(&*defer.ctx.repo.get()?)?.changes;

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

    if !skip_gui_check && should_exit_early(&mut defer.ctx, session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    if !defer.ctx.settings.claude.auto_commit_after_completion {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "No after-hook behaviour required.".to_string(),
            suppress_output: true,
        });
    }

    // Create repo and workspace once at the entry point
    let mut guard = defer.ctx.exclusive_worktree_access();
    let stacks = list_stacks(&defer.ctx)?;

    // If the session stopped, but there's no session persisted in the database, we create a new one.
    // If the session is already persisted, we just retrieve it.
    let stack_id = get_or_create_session(&mut defer.ctx, guard.write_permission(), &resolved_session_id, stacks)?;

    // Drop the guard we made above, certain commands below are also getting their own exclusive
    // lock so we need to drop this here to ensure we don't end up with a deadlock.
    drop(guard);

    let (id, outcome) = but_action::handle_changes(
        &mut defer.ctx,
        &summary,
        Some(prompt.clone()),
        ActionHandler::HandleChangesSimple,
        Source::ClaudeCode(resolved_session_id.clone()),
        Some(stack_id),
    )?;

    let stacks = list_stacks(&defer.ctx)?;

    // Trigger commit message generation for newly created commits
    // TODO: Maybe this can be done in the main app process i.e. the GitButler GUI, if available
    // Alternatively, and probably better - we could spawn a new process to do this

    if let Some(llm) = LLMProvider::default_openai() {
        for branch in &outcome.updated_branches {
            let mut commit_message_mapping = HashMap::new();

            let eligibility = is_branch_eligible_for_rename(&defer.ctx, &stacks, branch)?;

            for commit in &branch.new_commits {
                if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                    let commit_event = CommitEvent {
                        external_summary: summary.clone(),
                        external_prompt: prompt.clone(),
                        branch_name: branch.branch_name.clone(),
                        commit_id,
                        ctx: defer.ctx.to_sync(),
                        trigger: id,
                    };
                    let reword_result = but_action::reword::commit(&llm, commit_event).ok().unwrap_or_default();

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
                        but_action::rename_branch::rename_branch(&mut defer.ctx, &llm, params, id)
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
            let session_uuid = uuid::Uuid::parse_str(&resolved_session_id)?;
            let commit_message =
                crate::MessagePayload::GitButler(crate::GitButlerUpdate::CommitCreated(crate::CommitCreatedDetails {
                    stack_id: Some(branch.stack_id.to_string()),
                    branch_name: Some(final_branch_name),
                    commit_ids: Some(final_commit_ids),
                }));

            crate::db::save_new_message(&mut defer.ctx, session_uuid, commit_message)?;
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

pub fn handle_pre_tool_call(mut ctx: Context, read: impl std::io::Read) -> anyhow::Result<ClaudeHookOutput> {
    let input: ClaudePreToolUseInput =
        serde_json::from_reader(read).map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;

    let session_id = original_session_id(&mut ctx, input.session_id.clone())?;

    if should_exit_early(&mut ctx, &input.session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    file_lock::obtain_or_insert(&mut ctx, session_id, input.tool_input.file_path)?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

/// SDK variant of pre-tool-call handler that performs file locking.
/// This is called from the SDK hook and skips the GUI check.
pub fn lock_file_for_tool_call(
    sync_ctx: but_ctx::ThreadSafeContext,
    session_id: &str,
    file_path: &str,
) -> anyhow::Result<ClaudeHookOutput> {
    let mut ctx: Context = sync_ctx.into_thread_local();
    let resolved_session_id = original_session_id(&mut ctx, session_id.to_string())?;
    file_lock::obtain_or_insert(&mut ctx, resolved_session_id, file_path.to_string())?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

pub fn handle_post_tool_call(ctx: Context, read: impl std::io::Read) -> anyhow::Result<ClaudeHookOutput> {
    let input: ClaudePostToolUseInput =
        serde_json::from_reader(read).map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {e}"))?;

    assign_hunks_post_tool_call(
        ctx,
        &input.session_id,
        &input.tool_response.file_path,
        &input.tool_response.structured_patch,
        false,
    )
}

pub fn assign_hunks_post_tool_call(
    mut ctx: Context,
    session_id: &str,
    file_path: &str,
    structured_patch: &[StructuredPatch],
    skip_gui_check: bool,
) -> anyhow::Result<ClaudeHookOutput> {
    let hook_headers: Vec<HunkHeader> = structured_patch.iter().map(|p| p.into()).collect();

    let resolved_session_id = original_session_id(&mut ctx, session_id.to_string())?;

    // ClearLocksGuard ensures the file lock is cleared on drop, including early returns.
    let mut defer = ClearLocksGuard {
        ctx,
        session_id: resolved_session_id.clone(),
        file_path: Some(file_path.to_string()),
    };

    if !skip_gui_check && should_exit_early(&mut defer.ctx, session_id)? {
        return Ok(ClaudeHookOutput {
            do_continue: true,
            stop_reason: "Session running in GUI, skipping hook".to_string(),
            suppress_output: true,
        });
    }

    let relative_file_path = defer.ctx.workdir_or_fail().ok().and_then(|worktree_dir| {
        std::path::PathBuf::from(file_path)
            .strip_prefix(worktree_dir)
            .ok()
            .map(|rel| rel.to_string_lossy().to_string())
    });

    let mut guard = defer.ctx.exclusive_worktree_access();
    let stacks = list_stacks(&defer.ctx)?;

    let stack_id = get_or_create_session(&mut defer.ctx, guard.write_permission(), &resolved_session_id, stacks)?;

    let changes = but_core::diff::ui::worktree_changes(&*defer.ctx.repo.get()?)?.changes;
    let context_lines = defer.ctx.settings.context_lines;
    let (repo, ws, mut db) = defer.ctx.workspace_and_db_mut_with_perm(guard.read_permission())?;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        true,
        Some(changes.clone()),
        None,
        context_lines,
    )?;

    let assignment_reqs: Vec<HunkAssignmentRequest> = assignments
        .into_iter()
        .filter(|a| a.stack_id.is_none())
        .filter(|a| {
            let Some(ref relative_file_path) = relative_file_path else {
                return false;
            };
            let path_matches = a.path.to_lowercase() == relative_file_path.to_lowercase();
            if hook_headers.is_empty() {
                path_matches
            } else if path_matches {
                if let Some(a) = a.hunk_header {
                    hook_headers.iter().any(|h| h.new_range().intersects(a.new_range()))
                } else {
                    true
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

    let _rejections = but_hunk_assignment::assign(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        assignment_reqs,
        None,
        context_lines,
    )?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
}

fn original_session_id(ctx: &mut Context, current_id: String) -> Result<String> {
    let original_session_id = crate::db::get_session_by_current_id(ctx, Uuid::parse_str(&current_id)?)?;
    if let Some(session) = original_session_id {
        Ok(session.id.to_string())
    } else {
        Ok(current_id)
    }
}

pub fn get_or_create_session(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    session_id: &str,
    stacks: Vec<but_workspace::legacy::ui::StackEntry>,
) -> Result<StackId, anyhow::Error> {
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
            let stack_id = create_stack(ctx, perm)?;
            crate::rules::update_claude_assignment_rule_target(ctx, rule.id, stack_id, perm)?;
            stack_id
        }
    } else {
        // If the session is not in the list of sessions, then create a new stack + session entry
        // Create a new stack
        let stack_id = create_stack(ctx, perm)?;
        crate::rules::create_claude_assignment_rule(ctx, Uuid::parse_str(session_id)?, stack_id, perm)?;
        stack_id
    };
    Ok(stack_id)
}

fn create_stack(ctx: &Context, perm: &mut RepoExclusive) -> anyhow::Result<StackId> {
    let branch_name = but_core::branch::unique_canned_refname(&*ctx.repo.get()?)?;
    let create_req = BranchCreateRequest {
        name: Some(branch_name.shorten().to_string()),
        order: None,
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

impl ClaudeHookOutput {
    pub fn should_continue(&self) -> bool {
        self.do_continue
    }
}

pub(crate) struct ClearLocksGuard {
    pub ctx: Context,
    session_id: String,
    file_path: Option<String>,
}

impl Drop for ClearLocksGuard {
    fn drop(&mut self) {
        file_lock::clear(&mut self.ctx, self.session_id.clone(), self.file_path.clone()).ok();
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
    let meta = ctx.legacy_meta()?;
    but_workspace::legacy::stack_details_v3(Some(stack_id), &repo, &meta)
}

fn list_stacks(ctx: &Context) -> anyhow::Result<Vec<StackEntry>> {
    let repo = ctx.repo.get()?;
    let meta = ctx.legacy_meta()?;
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
