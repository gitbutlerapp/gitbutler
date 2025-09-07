use but_action::OpenAiProvider;
use but_api::commands::action;
use but_api::error::Error;
use but_core::ui::TreeChange;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tauri::Emitter;

#[tauri::command(async)]
pub fn list_actions(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::ActionListing, Error> {
    action::list_actions(project_id, offset, limit)
}

#[tauri::command(async)]
pub fn handle_changes(
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    action::handle_changes(project_id, change_summary, handler)
}

#[tauri::command(async)]
pub fn list_workflows(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::WorkflowList, Error> {
    action::list_workflows(project_id, offset, limit)
}

#[tauri::command(async)]
pub fn auto_commit(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    action::auto_commit_internal(emitter, project_id, changes)
}

#[tauri::command(async)]
pub fn auto_branch_changes(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    action::auto_branch_changes_internal(emitter, project_id, changes)
}

#[tauri::command(async)]
pub fn absorb(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    action::absorb_internal(emitter, project_id, changes)
}

#[tauri::command(async)]
pub fn freestyle(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_action::ChatMessage>,
    model: Option<String>,
) -> anyhow::Result<String, Error> {
    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    action::freestyle_internal(emitter, project_id, message_id, chat_messages, model)
}
