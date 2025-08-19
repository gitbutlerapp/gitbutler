use but_api::{
    commands::claude::{self, GetMessagesParams, SendMessageParams},
    error::Error,
    App,
};
use but_claude::ClaudeMessage;
use but_workspace::StackId;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
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
#[instrument(skip(app), err(Debug))]
pub fn claude_get_messages(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<Vec<ClaudeMessage>, Error> {
    claude::claude_get_messages(
        &app,
        GetMessagesParams {
            project_id,
            stack_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn claude_get_session_details(
    app: State<'_, App>,
    project_id: ProjectId,
    session_id: String,
) -> Result<but_claude::ClaudeSessionDetails, Error> {
    claude::claude_get_session_details(
        &app,
        claude::GetSessionDetailsParams {
            project_id,
            session_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn claude_list_permission_requests(
    app: State<'_, App>,
    project_id: ProjectId,
) -> Result<Vec<but_claude::ClaudePermissionRequest>, Error> {
    claude::claude_list_permission_requests(
        &app,
        claude::ListPermissionRequestsParams { project_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn claude_update_permission_request(
    app: State<'_, App>,
    project_id: ProjectId,
    request_id: String,
    approval: bool,
) -> Result<(), Error> {
    claude::claude_update_permission_request(
        &app,
        claude::UpdatePermissionRequestParams {
            project_id,
            request_id,
            approval,
        },
    )
}
