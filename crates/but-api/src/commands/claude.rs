use std::sync::Arc;

use but_claude::{ClaudeMessage, ThinkingLevel};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{App, NoParams, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub message: String,
    pub thinking_level: ThinkingLevel,
}

pub async fn claude_send_message(app: &App, params: SendMessageParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = Arc::new(Mutex::new(CommandContext::open(
        &project,
        app.app_settings.get()?.clone(),
    )?));
    app.claudes
        .send_message(
            ctx,
            app.broadcaster.clone(),
            params.stack_id,
            &params.message,
            params.thinking_level,
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
    let mut ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let messages = app.claudes.get_messages(&mut ctx, params.stack_id)?;
    Ok(messages)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSessionDetailsParams {
    pub project_id: ProjectId,
    pub session_id: String,
}

pub fn claude_get_session_details(
    _app: &App,
    params: GetSessionDetailsParams,
) -> Result<but_claude::ClaudeSessionDetails, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let session_id = uuid::Uuid::parse_str(&params.session_id).map_err(anyhow::Error::from)?;
    let transcript_path = but_claude::Transcript::get_transcript_path(&project.path, session_id)?;
    let transcript = but_claude::Transcript::from_file(&transcript_path)?;
    Ok(but_claude::ClaudeSessionDetails {
        summary: transcript.summary(),
        last_prompt: transcript.prompt(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPermissionRequestsParams {
    pub project_id: ProjectId,
}

pub fn claude_list_permission_requests(
    app: &App,
    params: ListPermissionRequestsParams,
) -> Result<Vec<but_claude::ClaudePermissionRequest>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let mut ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(but_claude::db::list_all_permission_requests(&mut ctx)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePermissionRequestParams {
    pub project_id: ProjectId,
    pub request_id: String,
    pub approval: bool,
}

pub fn claude_update_permission_request(
    app: &App,
    params: UpdatePermissionRequestParams,
) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let mut ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    Ok(but_claude::db::update_permission_request(
        &mut ctx,
        &params.request_id,
        params.approval,
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

pub async fn claude_check_available(app: &App, _params: NoParams) -> Result<bool, Error> {
    let claude_executable = app.app_settings.get()?.claude.executable.clone();
    let is_available = but_claude::bridge::check_claude_available(&claude_executable).await;
    Ok(is_available)
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
