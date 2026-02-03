use but_api::json::Error;
use but_core::ui::TreeChange;
use but_ctx::Context;
use but_hunk_assignment::AbsorptionTarget;
use but_llm::LLMProvider;
use but_settings::AppSettings;
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
    let ctx = Context::new_from_legacy_project(project.clone())?;
    but_action::list_actions(&ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn handle_changes(
    project_id: ProjectId,
    change_summary: String,
    handler: but_action::ActionHandler,
) -> anyhow::Result<but_action::Outcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    but_action::handle_changes(
        &mut ctx,
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
    let ctx = Context::new_from_legacy_project(project.clone())?;
    but_action::list_workflows(&ctx, offset, limit).map_err(|e| Error::from(anyhow::anyhow!(e)))
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn auto_commit(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    target: AbsorptionTarget,
    use_ai: bool,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let absorption_plan = but_api::legacy::absorb::absorption_plan(&mut ctx, target)?;

    let llm = if use_ai {
        let git_config = gix::config::File::from_globals().map_err(|e| Error::from(anyhow::anyhow!(e)))?;
        LLMProvider::from_git_config(&git_config)
    } else {
        None
    };

    let emitter = move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    };
    let mut guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;
    let project_data_dir = ctx.project_data_dir();
    let settings = AppSettings::load_from_default_path_creating_without_customization()?;
    but_action::auto_commit(
        project_id,
        &repo,
        &project_data_dir,
        settings.context_lines,
        llm.as_ref(),
        emitter,
        absorption_plan,
        &mut guard,
    )
    .map_err(|e| Error::from(anyhow::anyhow!(e)))?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn auto_branch_changes(
    project_id: ProjectId,
    changes: Vec<TreeChange>,
    model: String,
) -> anyhow::Result<(), Error> {
    let project = gitbutler_project::get(project_id)?;
    let changes: Vec<but_core::TreeChange> = changes.into_iter().map(|change| change.into()).collect();
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let git_config = gix::config::File::from_globals().map_err(|e| Error::from(anyhow::anyhow!(e)))?;
    let llm = LLMProvider::from_git_config(&git_config);

    match llm {
        Some(llm) => {
            but_action::branch_changes(&mut ctx, &llm, changes, model).map_err(|e| Error::from(anyhow::anyhow!(e)))
        }
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}
