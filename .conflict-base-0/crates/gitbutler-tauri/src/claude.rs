use but_api::{
    commands::claude::{
        self, CancelSessionParams, GetMessagesParams, IsStackActiveParams, SendMessageParams,
    },
    error::Error,
    App,
};
use but_claude::{ClaudeMessage, ClaudeUserParams, ModelType, PermissionMode, ThinkingLevel};
use but_workspace::StackId;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn claude_send_message(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
    message: String,
    thinking_level: ThinkingLevel,
    model: ModelType,
    permission_mode: PermissionMode,
    disabled_mcp_servers: Vec<String>,
    add_dirs: Vec<String>,
) -> Result<(), Error> {
    claude::claude_send_message(
        &app,
        SendMessageParams {
            project_id,
            stack_id,
            user_params: ClaudeUserParams {
                message,
                thinking_level,
                model,
                permission_mode,
                disabled_mcp_servers,
                add_dirs,
            },
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
pub async fn claude_cancel_session(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<bool, Error> {
    claude::claude_cancel_session(
        &app,
        CancelSessionParams {
            project_id,
            stack_id,
        },
    )
    .await
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn claude_is_stack_active(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<bool, Error> {
    claude::claude_is_stack_active(
        &app,
        IsStackActiveParams {
            project_id,
            stack_id,
        },
    )
    .await
}
