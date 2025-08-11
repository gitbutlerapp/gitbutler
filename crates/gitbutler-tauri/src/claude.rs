use but_api::{
    commands::claude::{self, GetTranscriptParams, SendMessageParams},
    error::Error,
    App,
};
use but_claude::ClaudeMessage;
use but_workspace::StackId;
use gitbutler_project::ProjectId;
use tauri::State;

#[tauri::command(async)]
pub async fn claude_send_message(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
    message: String,
) -> Result<(), Error> {
    claude::claude_send_message(
        &app,
        SendMessageParams {
            project_id,
            stack_id,
            message,
        },
    )
    .await
}

#[tauri::command(async)]
pub async fn claude_get_transcript(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<Vec<ClaudeMessage>, Error> {
    claude::claude_get_transcript(
        &app,
        GetTranscriptParams {
            project_id,
            stack_id,
        },
    )
    .await
}
