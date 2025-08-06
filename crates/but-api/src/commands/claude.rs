use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub message: String,
}

pub async fn claude_send_message(app: &App, params: SendMessageParams) -> Result<(), Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    app.claudes.lock().await.send_message(
        &ctx,
        app.broadcaster.clone(),
        params.stack_id,
        &params.message,
    )?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTranscriptParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
}

pub async fn claude_get_transcript(
    app: &App,
    params: GetTranscriptParams,
) -> Result<Vec<serde_json::Value>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let transcript = app
        .claudes
        .lock()
        .await
        .get_transcript(&ctx, params.stack_id)?;
    Ok(transcript)
}
