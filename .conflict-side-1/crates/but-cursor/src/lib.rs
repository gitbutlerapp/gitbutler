use std::{
    collections::HashMap,
    io::{self, Read},
    str::FromStr,
};

use but_action::{ActionHandler, OpenAiProvider, Source, reword::CommitEvent};
use but_graph::VirtualBranchesTomlMetadata;
use but_hunk_assignment::HunkAssignmentRequest;
use but_settings::AppSettings;
use but_workspace::StacksFilter;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_stack::VirtualBranchesHandle;
use gix::diff::blob::{
    Algorithm, UnifiedDiff,
    unified_diff::{ConsumeBinaryHunk, ContextSize},
};
use serde::{Deserialize, Serialize};

pub mod db;
pub mod workspace_identifier;

// Re-export main functionality
pub use db::{Generation, get_generations};

/// Message returned back to Cursor after running a hook
#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CursorHookOutput {
    /// Whether the agent should continue or the loop should stop
    #[serde(rename = "continue")]
    do_continue: bool,
    /// Message shown to user in UI
    user_message: String,
    /// Message shown to the agent in next turn
    agent_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Edit {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Default)]
struct ProduceDiffHunk {
    headers: Vec<but_workspace::HunkHeader>,
}
impl gix::diff::blob::unified_diff::ConsumeBinaryHunkDelegate for ProduceDiffHunk {
    fn consume_binary_hunk(
        &mut self,
        header: gix::diff::blob::unified_diff::HunkHeader,
        _header_str: &str,
        _hunk: &[u8],
    ) -> std::io::Result<()> {
        self.headers.push(but_workspace::HunkHeader {
            old_start: header.before_hunk_start,
            old_lines: header.before_hunk_len,
            new_start: header.after_hunk_start,
            new_lines: header.after_hunk_len,
        });
        Ok(())
    }
}

impl Edit {
    fn generate_headers(&self) -> anyhow::Result<Vec<but_workspace::HunkHeader>> {
        let interner = gix::diff::blob::intern::InternedInput::new(
            self.old_string.as_bytes(),
            self.new_string.as_bytes(),
        );
        let headers = gix::diff::blob::diff(
            Algorithm::Myers,
            &interner,
            UnifiedDiff::new(
                &interner,
                ConsumeBinaryHunk::new(ProduceDiffHunk::default(), "\n"),
                ContextSize::symmetrical(0), // Zero context lines is fine since the hunk will be reconciled later with but_hunk_assignment::assignments_with_fallback
            ),
        )?
        .headers;
        Ok(headers)
    }
}

/// The payload sent to the `afterEdit` hook
#[derive(Debug, Serialize, Deserialize)]
pub struct FileEditEvent {
    pub conversation_id: String,
    pub generation_id: String,
    pub file_path: String,
    pub edits: Vec<Edit>,
    pub hook_event_name: String,
    pub workspace_roots: Vec<String>,
}

/// The payload sent to the `stop` hook
#[derive(Debug, Serialize, Deserialize)]
pub struct StopEvent {
    pub conversation_id: String,
    pub generation_id: String,
    pub status: String,
    pub hook_event_name: String,
    pub workspace_roots: Vec<String>,
}

pub async fn handle_after_edit() -> anyhow::Result<CursorHookOutput> {
    let input: FileEditEvent = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;
    let hook_headers = input
        .edits
        .last()
        .map(|edit| edit.generate_headers())
        .ok_or_else(|| anyhow::anyhow!("No hunk headers"))
        .flatten()?;

    let dir = input
        .workspace_roots
        .first()
        .ok_or_else(|| anyhow::anyhow!("No workspace roots provided"))
        .map(std::path::Path::new)?;
    let repo = gix::discover(dir)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow::anyhow!("No worktree found for repo"))?,
    )?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let vb_state = &VirtualBranchesHandle::new(ctx.project().gb_dir());

    let stacks = but_workspace::stacks_v3(&repo, &meta, StacksFilter::default(), None)?;
    let stack_id =
        but_claude::hooks::get_or_create_session(ctx, &input.conversation_id, stacks, vb_state)?;

    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.worktree_dir()?.into())?
            .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, true, Some(changes.clone()), None)?;

    let assignment_reqs: Vec<HunkAssignmentRequest> = assignments
        .into_iter()
        .filter(|a| a.stack_id.is_none())
        .filter(|a| {
            // If the hook_headers is empty, we probably created a file.
            if hook_headers.is_empty() {
                a.path.to_lowercase() == input.file_path.to_lowercase()
            } else if a.path.to_lowercase() == input.file_path.to_lowercase() {
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

    let _rejections = but_hunk_assignment::assign(ctx, assignment_reqs, None)?;

    Ok(CursorHookOutput::default())
}

pub async fn handle_stop(nightly: bool) -> anyhow::Result<CursorHookOutput> {
    let input: StopEvent = serde_json::from_str(&stdin()?)
        .map_err(|e| anyhow::anyhow!("Failed to parse input JSON: {}", e))?;
    let dir = input
        .workspace_roots
        .first()
        .ok_or_else(|| anyhow::anyhow!("No workspace roots provided"))
        .map(std::path::Path::new)?;
    let repo = gix::discover(dir)?;
    let project = Project::from_path(
        repo.workdir()
            .ok_or(anyhow::anyhow!("No worktree found for repo"))?,
    )?;

    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.worktree_dir()?.into())?
            .changes;

    if changes.is_empty() {
        return Ok(CursorHookOutput::default());
    }

    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let vb_state = &VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stacks = but_workspace::stacks_v3(&repo, &meta, StacksFilter::default(), None)?;
    let stack_id =
        but_claude::hooks::get_or_create_session(ctx, &input.conversation_id, stacks, vb_state)?;

    let summary = "".to_string();
    let prompt = crate::db::get_generations(dir, nightly)
        .map(|gens| {
            gens.iter()
                .find(|g| g.generation_uuid == input.generation_id)
                .map(|g| g.text_description.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let (id, outcome) = but_action::handle_changes(
        ctx,
        &summary,
        Some(prompt.clone()),
        ActionHandler::HandleChangesSimple,
        Source::Cursor(input.conversation_id),
        Some(stack_id),
    )?;

    let stacks = but_workspace::stacks_v3(&repo, &meta, StacksFilter::default(), None)?;

    // Trigger commit message generation for newly created commits
    // TODO: Maybe this can be done in the main app process i.e. the GitButler GUI, if avaialbe
    // Alternatively, and probably better - we could spawn a new process to do this

    if let Some(openai_client) =
        OpenAiProvider::with(None).and_then(|provider| provider.client().ok())
    {
        for branch in &outcome.updated_branches {
            let mut commit_message_mapping = HashMap::new();

            let elegibility =
                but_claude::hooks::is_branch_eligible_for_rename(ctx, &stacks, branch)?;

            for commit in &branch.new_commits {
                if let Ok(commit_id) = gix::ObjectId::from_str(commit) {
                    let commit_event = CommitEvent {
                        external_summary: summary.clone(),
                        external_prompt: prompt.clone(),
                        branch_name: branch.branch_name.clone(),
                        commit_id,
                        project: project.clone(),
                        app_settings: ctx.app_settings().clone(),
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
                but_claude::hooks::RenameEligibility::Eligible { commit_id } => {
                    let reword_result = commit_message_mapping.get(&commit_id).cloned();

                    if let Some((commit_id, commit_message)) = reword_result {
                        let params = but_action::rename_branch::RenameBranchParams {
                            commit_id,
                            commit_message,
                            stack_id: branch.stack_id,
                            current_branch_name: branch.branch_name.clone(),
                        };
                        but_action::rename_branch::rename_branch(ctx, &openai_client, params, id)
                            .await
                            .ok();
                    }
                }
                but_claude::hooks::RenameEligibility::NotEligible => {
                    // Do nothing, branch is not eligible for renaming
                }
            }
        }
    }

    Ok(CursorHookOutput::default())
}

fn stdin() -> anyhow::Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}
