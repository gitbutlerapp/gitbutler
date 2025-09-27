use std::sync::Arc;

use anyhow::Context;
use but_api_macros::api_cmd;
use but_claude::{
    ClaudeCheckResult, ClaudeMessage, ClaudeUserParams, Transcript,
    claude_mcp::{ClaudeMcpConfig, McpConfig},
    claude_settings::ClaudeSettings,
    prompt_templates,
};
use but_settings::AppSettings;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::instrument;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    #[serde(flatten)]
    pub user_params: ClaudeUserParams,
}

pub async fn claude_send_message(app: &App, params: SendMessageParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Arc::new(Mutex::new(CommandContext::open(
        &project,
        AppSettings::load_from_default_path_creating()?,
    )?));
    app.claudes
        .send_message(
            ctx,
            app.broadcaster.clone(),
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
    app: &App,
    params: GetMessagesParams,
) -> Result<Vec<ClaudeMessage>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let messages = app.claudes.get_messages(&mut ctx, params.stack_id)?;
    Ok(messages)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn claude_get_session_details(
    project_id: ProjectId,
    session_id: String,
) -> Result<but_claude::ClaudeSessionDetails, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let session_id = uuid::Uuid::parse_str(&session_id).map_err(anyhow::Error::from)?;
    let session = but_claude::db::get_session_by_id(&mut ctx, session_id)?
        .context("Could not find session")?;
    let current_id = Transcript::current_valid_session_id(&project.path, &session).await?;
    if let Some(current_id) = current_id {
        let transcript_path =
            but_claude::Transcript::get_transcript_path(&project.path, current_id)?;
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

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_list_permission_requests(
    project_id: ProjectId,
) -> Result<Vec<but_claude::ClaudePermissionRequest>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(but_claude::db::list_all_permission_requests(&mut ctx)?)
}
#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_update_permission_request(
    project_id: ProjectId,
    request_id: String,
    approval: bool,
) -> Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    Ok(but_claude::db::update_permission_request(
        &mut ctx,
        &request_id,
        approval,
    )?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSessionParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_cancel_session(app: &App, params: CancelSessionParams) -> Result<bool, Error> {
    let cancelled = app.claudes.cancel_session(params.stack_id).await?;
    Ok(cancelled)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn claude_check_available() -> Result<ClaudeCheckResult, Error> {
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

pub async fn claude_is_stack_active(app: &App, params: IsStackActiveParams) -> Result<bool, Error> {
    let is_active = app.claudes.is_stack_active(params.stack_id).await;
    Ok(is_active)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactHistoryParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_compact_history(app: &App, params: CompactHistoryParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Arc::new(Mutex::new(CommandContext::open(
        &project,
        AppSettings::load_from_default_path_creating()?,
    )?));
    app.claudes
        .compact_history(ctx, app.broadcaster.clone(), params.stack_id)
        .await?;
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_get_prompt_templates() -> Result<prompt_templates::PromptTemplates, Error> {
    let templates = prompt_templates::load_prompt_templates()?;
    Ok(templates)
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_write_prompt_templates(
    templates: prompt_templates::PromptTemplates,
) -> Result<(), Error> {
    prompt_templates::write_prompt_templates(&templates)?;
    Ok(())
}

#[api_cmd]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_get_prompt_templates_path() -> Result<String, Error> {
    let path = prompt_templates::get_prompt_templates_path_string()?;
    Ok(path)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn claude_get_mcp_config(project_id: ProjectId) -> Result<McpConfig, Error> {
    let project = gitbutler_project::get(project_id)?;
    let settings = ClaudeSettings::open(&project.path).await;
    let mcp_config = ClaudeMcpConfig::open(&settings, &project.path).await;
    Ok(mcp_config.mcp_servers())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn claude_get_sub_agents(
    project_id: ProjectId,
) -> Result<Vec<but_claude::SubAgent>, Error> {
    let project = gitbutler_project::get(project_id)?;
    let sub_agents = but_claude::claude_sub_agents::read_claude_sub_agents(&project.path).await;
    Ok(sub_agents)
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub async fn claude_verify_path(project_id: ProjectId, path: String) -> Result<bool, Error> {
    let project = gitbutler_project::get(project_id)?;

    // Check if it's an absolute path first
    let path = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(&path)
    } else {
        // If relative, make it relative to project path
        project.path.join(&path)
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
