use but_claude::ClaudeMessage;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use serde::Deserialize;
use tokio::sync::Mutex;

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
    let ctx = Mutex::new(CommandContext::open(
        &project,
        app.app_settings.get()?.clone(),
    )?);
    app.claudes
        .send_message(
            ctx,
            app.broadcaster.clone(),
            params.stack_id,
            &params.message,
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

pub async fn claude_get_messages(
    app: &App,
    params: GetMessagesParams,
) -> Result<Vec<ClaudeMessage>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let mut ctx = CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let messages = app.claudes.get_messages(&mut ctx, params.stack_id)?;
    Ok(messages)
}
