use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_claude::{
    Claude, ClaudeCheckResult, ClaudeMessage, ClaudeUserParams, Transcript,
    claude_mcp::{ClaudeMcpConfig, McpConfig},
    claude_settings::ClaudeSettings,
    prompt_templates,
};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_settings::AppSettings;
use gitbutler_project::ProjectId;
use serde::Deserialize;
use tracing::instrument;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    #[serde(flatten)]
    pub user_params: ClaudeUserParams,
}

pub async fn claude_send_message(claude: &Claude, params: SendMessageParams) -> Result<()> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    claude
        .instance_by_stack
        .send_message(
            ctx.into_sync(),
            claude.broadcaster.clone(),
            params.stack_id,
            params.user_params,
        )
        .await?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMessagesParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub fn claude_get_messages(
    claude: &Claude,
    params: GetMessagesParams,
) -> Result<Vec<ClaudeMessage>> {
    let project = gitbutler_project::get(params.project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let messages = claude
        .instance_by_stack
        .get_messages(&mut ctx, params.stack_id)?;
    Ok(messages)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_session_details(
    project_id: ProjectId,
    session_id: String,
) -> Result<but_claude::ClaudeSessionDetails> {
    let project = gitbutler_project::get(project_id)?;
    let (worktree_dir, session) = {
        let mut ctx = Context::new_from_legacy_project(project.clone())?;
        let session_id = uuid::Uuid::parse_str(&session_id).map_err(anyhow::Error::from)?;
        let session = but_claude::db::get_session_by_id(&mut ctx, session_id)?
            .context("Could not find session")?;
        let worktree_dir = project.worktree_dir()?;
        (worktree_dir, session)
    };
    let current_id = Transcript::current_valid_session_id(worktree_dir, &session).await?;
    if let Some(current_id) = current_id {
        let transcript_path =
            but_claude::Transcript::get_transcript_path(worktree_dir, current_id)?;
        let transcript = but_claude::Transcript::from_file(&transcript_path)?;
        Ok(but_claude::ClaudeSessionDetails {
            summary: transcript.summary(),
            last_prompt: transcript.prompt(),
            in_gui: session.in_gui,
        })
    } else {
        Ok(but_claude::ClaudeSessionDetails {
            summary: None,
            last_prompt: None,
            in_gui: session.in_gui,
        })
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_get_user_message(
    project_id: ProjectId,
    offset: Option<i64>,
) -> Result<Option<ClaudeMessage>> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    but_claude::db::get_user_message(&mut ctx, offset)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_list_permission_requests(
    project_id: ProjectId,
) -> Result<Vec<but_claude::ClaudePermissionRequest>> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    but_claude::db::list_all_permission_requests(&mut ctx)
}
#[but_api]
#[instrument(err(Debug))]
pub fn claude_update_permission_request(
    project_id: ProjectId,
    request_id: String,
    decision: but_claude::PermissionDecision,
    use_wildcard: bool,
) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    but_claude::db::update_permission_request(&mut ctx, &request_id, decision, use_wildcard)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSessionParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_cancel_session(claude: &Claude, params: CancelSessionParams) -> Result<bool> {
    let cancelled = claude
        .instance_by_stack
        .cancel_session(params.stack_id)
        .await?;
    Ok(cancelled)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_check_available() -> Result<ClaudeCheckResult> {
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let claude_executable = app_settings.claude.executable.clone();
    Ok(but_claude::bridge::check_claude_available(&claude_executable).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsStackActiveParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_is_stack_active(claude: &Claude, params: IsStackActiveParams) -> Result<bool> {
    let is_active = claude
        .instance_by_stack
        .is_stack_active(params.stack_id)
        .await;
    Ok(is_active)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactHistoryParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_compact_history(claude: &Claude, params: CompactHistoryParams) -> Result<()> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    claude
        .instance_by_stack
        .compact_history(ctx.into_sync(), claude.broadcaster.clone(), params.stack_id)
        .await?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_list_prompt_templates(
    project_id: ProjectId,
) -> Result<Vec<prompt_templates::PromptTemplate>> {
    let project = gitbutler_project::get(project_id)?;
    let templates = prompt_templates::list_templates(&project)?;
    Ok(templates)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_get_prompt_dirs(project_id: ProjectId) -> Result<Vec<prompt_templates::PromptDir>> {
    let project = gitbutler_project::get(project_id)?;
    let dirs = prompt_templates::prompt_dirs(&project)?;
    Ok(dirs)
}

#[but_api]
#[instrument(err(Debug))]
pub fn claude_maybe_create_prompt_dir(project_id: ProjectId, path: String) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    prompt_templates::maybe_create_dir(&project, &path)?;
    Ok(())
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_mcp_config(project_id: ProjectId) -> Result<McpConfig> {
    let project = gitbutler_project::get(project_id)?;
    let worktree_dir = project.worktree_dir()?;
    let settings = ClaudeSettings::open(worktree_dir).await;
    let mcp_config = ClaudeMcpConfig::open(&settings, worktree_dir).await;
    Ok(mcp_config.mcp_servers())
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_get_sub_agents(project_id: ProjectId) -> Result<Vec<but_claude::SubAgent>> {
    let project = gitbutler_project::get(project_id)?;
    let sub_agents =
        but_claude::claude_sub_agents::read_claude_sub_agents(project.worktree_dir()?).await;
    Ok(sub_agents)
}

#[but_api]
#[instrument(err(Debug))]
pub async fn claude_verify_path(project_id: ProjectId, path: String) -> Result<bool> {
    let project = gitbutler_project::get(project_id)?;

    // Check if it's an absolute path first
    let path = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(&path)
    } else {
        // If relative, make it relative to project path
        project.worktree_dir()?.join(&path)
    };

    // Check if the path exists and is a directory
    match tokio::fs::try_exists(&path).await {
        Ok(exists) => {
            if exists {
                match tokio::fs::metadata(&path).await {
                    Ok(metadata) => Ok(metadata.is_dir()),
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false),
    }
}
