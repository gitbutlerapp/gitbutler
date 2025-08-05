use but_workspace::StackId;
use gitbutler_project::ProjectId;
use serde::Deserialize;

use crate::{App, error::Error};

#[derive(Deserialize)]
pub struct SendMessageParams {
    pub project_id: ProjectId,
    pub stack_id: StackId,
    pub message: String,
}

pub async fn claude_send_message(app: &App, params: SendMessageParams) -> Result<(), Error> {
    app.claudes.lock().await.send_message(
        app.broadcaster.clone(),
        params.project_id,
        params.stack_id,
        &params.message,
    )?;

    Ok(())
}
