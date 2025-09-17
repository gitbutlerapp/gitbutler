use std::sync::Arc;

use anyhow::Context;
use but_api_macros::api_cmd;
use but_claude::{
    ClaudeCheckResult, ClaudeMessage, ModelType, PermissionMode, ThinkingLevel, Transcript,
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
    pub message: String,
    pub thinking_level: ThinkingLevel,
    pub model: ModelType,
    pub permission_mode: PermissionMode,
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
            &params.message,
            params.thinking_level,
            params.model,
            params.permission_mode,
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
