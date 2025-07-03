use anyhow::anyhow;
use but_db::ClaudeCodeSession;
use but_hunk_assignment::HunkAssignmentRequest;
use but_settings::AppSettings;
use but_workspace::{HunkHeader, StackId};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, access::WorktreeWritePermission};
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

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
    pub new_string: String,
    pub old_string: String,
    pub replace_all: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponse {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    pub original_file: String,
    pub structured_patch: Vec<StructuredPatch>,
    pub user_modified: bool,
    pub replace_all: bool,
}

impl ToolResponse {
    pub fn to_assignment_requests(&self, stack_id: StackId) -> Vec<HunkAssignmentRequest> {
        self.structured_patch
            .iter()
            .map(|patch| HunkAssignmentRequest {
                hunk_header: Some(patch.into()),
                path_bytes: bstr::BString::from(self.file_path.clone()),
                stack_id: Some(stack_id),
            })
            .collect()
    }
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

pub(crate) fn handle_post_tool_call(input: String) -> anyhow::Result<ClaudeHookOutput> {
    let mut input: ClaudePostToolUseInput = serde_json::from_str(&input)
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

    let stacks = crate::log::stacks(ctx)?;
    let sessions = list_sessions(ctx)?;

    let vb_state = &VirtualBranchesHandle::new(ctx.project().gb_dir());

    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();

    let stack_id = if let Some(session) = sessions.iter().find(|s| s.id == input.session_id) {
        // If the stack referenced by the session is in the list of applied stacks do nothing
        // Otherwise, create a new stack and update the session
        if let Some(stack) = stacks.iter().find(|s| s.id.to_string() == session.stack_id) {
            stack.id
        } else {
            let stack_id = create_stack(ctx, vb_state, perm)?;
            ctx.db()?
                .claude_code_sessions()
                .update_stack_id(&input.session_id, &stack_id.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to update session stack ID: {}", e))?;
            stack_id
        }
    } else {
        // If the session is not in the list of sessions, then create a new stack + session entry
        // Create a new stack
        let stack_id = create_stack(ctx, vb_state, perm)?;
        let new_session = ClaudeCodeSession {
            id: input.session_id.clone(),
            created_at: chrono::Local::now().naive_local(),
            stack_id: stack_id.to_string(),
        };
        ctx.db()?
            .claude_code_sessions()
            .insert(new_session)
            .map_err(|e| anyhow::anyhow!("Failed to insert new session: {}", e))?;
        stack_id
    };

    let assignment_reqs = input.tool_response.to_assignment_requests(stack_id);
    let _rejections = but_hunk_assignment::assign(ctx, assignment_reqs, None)?;

    Ok(ClaudeHookOutput {
        do_continue: true,
        stop_reason: String::default(),
        suppress_output: true,
    })
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
