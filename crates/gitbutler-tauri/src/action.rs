use but_action::OpenAiProvider;
use but_api::error::Error;
use but_core::ui::TreeChange;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tauri::Emitter;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn list_actions(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::ActionListing, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::list_actions(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn handle_changes(
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::handle_changes(
        ctx,
        &change_summary,
        None,
        handler,
        but_action::Source::GitButler,
        None,
    )
    .map(|(_id, outcome)| outcome)
    .map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn list_workflows(
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::WorkflowList, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    but_action::list_workflows(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn auto_commit(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    match openai {
        Some(openai) => but_action::auto_commit(emitter, ctx, &openai, changes)
            .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn auto_branch_changes(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    match openai {
        Some(openai) => but_action::branch_changes(emitter, ctx, &openai, changes)
            .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn absorb(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    match openai {
        Some(openai) => but_action::absorb(emitter, ctx, &openai, changes)
            .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn freestyle(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_action::ChatMessage>,
    model: Option<String>,
) -> anyhow::Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::freestyle(
            project_id,
            message_id,
            emitter,
            ctx,
            &openai,
            chat_messages,
            model,
        )
        .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}
