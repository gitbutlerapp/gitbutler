use but_api::{
    json::Error,
    legacy::claude::{
        self, CancelSessionParams, CompactHistoryParams, GetMessagesParams, IsStackActiveParams,
        SendMessageParams,
    },
};
use but_claude::{
    Claude, ClaudeMessage, ClaudeUserParams, ModelType, PermissionMode, PromptAttachment,
    ThinkingLevel,
};
use but_core::ref_metadata::StackId;
use gitbutler_project::ProjectId;
use tauri::State;
use tracing::instrument;

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
#[instrument(skip(claude), err(Debug))]
pub async fn claude_send_message(
    claude: State<'_, Claude>,
    project_id: ProjectId,
    stack_id: StackId,
    message: String,
    thinking_level: ThinkingLevel,
    model: ModelType,
    permission_mode: PermissionMode,
    disabled_mcp_servers: Vec<String>,
    add_dirs: Vec<String>,
    attachments: Option<Vec<PromptAttachment>>,
) -> Result<(), Error> {
    claude::claude_send_message(
        &claude,
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
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(claude), err(Debug))]
pub fn claude_get_messages(
    claude: State<'_, Claude>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<Vec<ClaudeMessage>, Error> {
    claude::claude_get_messages(
        &claude,
        GetMessagesParams {
            project_id,
            stack_id,
        },
    )
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(claude), err(Debug))]
pub async fn claude_cancel_session(
    claude: State<'_, Claude>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<bool, Error> {
    claude::claude_cancel_session(
        &claude,
        CancelSessionParams {
            project_id,
            stack_id,
        },
    )
    .await
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(claude), err(Debug))]
pub async fn claude_is_stack_active(
    claude: State<'_, Claude>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<bool, Error> {
    claude::claude_is_stack_active(
        &claude,
        IsStackActiveParams {
            project_id,
            stack_id,
        },
    )
    .await
    .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(claude), err(Debug))]
pub async fn claude_compact_history(
    claude: State<'_, Claude>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<(), Error> {
    claude::claude_compact_history(
        &claude,
        CompactHistoryParams {
            project_id,
            stack_id,
        },
    )
    .await
    .map_err(Into::into)
}
