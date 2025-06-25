use crate::error::Error;
use but_action::OpenAiProvider;
use but_core::ui::TreeChange;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn list_actions(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::ActionListing, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::list_actions(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn handle_changes(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(None);
    but_action::handle_changes(
        ctx,
        &openai,
        &change_summary,
        None,
        handler,
        but_action::Source::GitButler,
    )
    .map(|(_id, outcome)| outcome)
    .map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn list_workflows(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    offset: i64,
    limit: i64,
) -> anyhow::Result<but_action::WorkflowList, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    but_action::list_workflows(ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn auto_commit(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    changes: Vec<TreeChange>,
) -> anyhow::Result<(), Error> {
    let project = projects.get(project_id)?;
    let changes: Vec<but_core::TreeChange> =
        changes.into_iter().map(|change| change.into()).collect();
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_action::auto_commit(ctx, &openai, changes).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}
