use but_api::{
    commands::claude::{
        self, CancelSessionParams, GetMessagesParams, IsStackActiveParams, SendMessageParams,
        WritePromptTemplatesParams,
    },
    error::Error,
    App, NoParams,
};
use but_claude::{prompt_templates::PromptTemplates, ClaudeMessage, ModelType, ThinkingLevel};
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
    thinking_level: ThinkingLevel,
    model: ModelType,
) -> Result<(), Error> {
    claude::claude_send_message(
        &app,
        SendMessageParams {
            project_id,
            stack_id,
            message,
            thinking_level,
            model,
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
#[instrument(err(Debug))]
pub fn claude_get_session_details(
    project_id: ProjectId,
    session_id: String,
) -> Result<but_claude::ClaudeSessionDetails, Error> {
    claude::claude_get_session_details(claude::GetSessionDetailsParams {
        project_id,
        session_id,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_list_permission_requests(
    project_id: ProjectId,
) -> Result<Vec<but_claude::ClaudePermissionRequest>, Error> {
    claude::claude_list_permission_requests(claude::ListPermissionRequestsParams { project_id })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_update_permission_request(
    project_id: ProjectId,
    request_id: String,
    approval: bool,
) -> Result<(), Error> {
    claude::claude_update_permission_request(claude::UpdatePermissionRequestParams {
        project_id,
        request_id,
        approval,
    })
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
#[instrument(err(Debug))]
pub async fn claude_check_available() -> Result<bool, Error> {
    claude::claude_check_available(but_api::NoParams {}).await
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

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_get_prompt_templates() -> Result<PromptTemplates, Error> {
    claude::claude_get_prompt_templates(NoParams {})
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_write_prompt_templates(templates: PromptTemplates) -> Result<(), Error> {
    claude::claude_write_prompt_templates(WritePromptTemplatesParams { templates })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn claude_get_prompt_templates_path() -> Result<String, Error> {
    claude::claude_get_prompt_templates_path(NoParams {})
}
