use but_action::OpenAiProvider;
use but_api::error::Error;
use but_core::ui::TreeChange;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn list_actions(
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::ActionListing, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::list_actions(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(settings), err(Debug))]
pub fn handle_changes(
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
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
#[instrument(skip(settings), err(Debug))]
pub fn list_workflows(
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::WorkflowList, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::list_workflows(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(app_handle, settings), err(Debug))]
pub fn auto_commit(
    app_handle: tauri::AppHandle,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::auto_commit(&app_handle, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle, settings), err(Debug))]
pub fn auto_branch_changes(
    app_handle: tauri::AppHandle,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::branch_changes(&app_handle, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle, settings), err(Debug))]
pub fn absorb(
    app_handle: tauri::AppHandle,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::absorb(&app_handle, ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle, settings), err(Debug))]
pub fn freestyle(
    app_handle: tauri::AppHandle,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_action::ChatMessage>,
    model: Option<String>,
) -> anyhow::Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::freestyle(project_id, message_id, &app_handle, ctx, &openai, chat_messages, model).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}
