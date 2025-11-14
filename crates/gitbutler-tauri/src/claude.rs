use but_api::{
    App,
    commands::claude::{
        self, CancelSessionParams, CompactHistoryParams, GetMessagesParams, IsStackActiveParams,
        SendMessageParams,
    },
    error::Error,
};
use but_claude::{
    ClaudeMessage, ClaudeUserParams, ModelType, PermissionMode, PromptAttachment, ThinkingLevel,
};
use but_core::ref_metadata::StackId;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn claude_send_message(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: Option<StackId>,
    message: String,
    thinking_level: ThinkingLevel,
    model: ModelType,
    permission_mode: PermissionMode,
    disabled_mcp_servers: Vec<String>,
    add_dirs: Vec<String>,
    attachments: Option<Vec<PromptAttachment>>,
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
                attachments,
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
    stack_id: Option<StackId>,
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
    stack_id: Option<StackId>,
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
    stack_id: Option<StackId>,
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

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub async fn claude_compact_history(
    app: State<'_, App>,
    project_id: ProjectId,
    stack_id: Option<StackId>,
) -> Result<(), Error> {
    claude::claude_compact_history(
        &app,
        CompactHistoryParams {
            project_id,
            stack_id,
        },
    )
    .await
}
