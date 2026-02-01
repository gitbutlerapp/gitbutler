use but_api::json::Error;
use but_ctx::Context;
use gitbutler_project::ProjectId;
use tauri::Emitter;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub fn bot(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_llm::ChatMessage>,
) -> anyhow::Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    let git_config =
        gix::config::File::from_globals().map_err(|e| Error::from(anyhow::anyhow!(e)))?;

    let llm = but_llm::LLMProvider::from_git_config(&git_config);
    match llm {
        Some(llm) => but_bot::bot(
            project_id,
            message_id,
            emitter,
            &mut ctx,
            &llm,
            chat_messages,
        )
        .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}

#[tauri::command(async)]
#[instrument(skip(app_handle), err(Debug))]
pub async fn forge_branch_chat(
    app_handle: tauri::AppHandle,
    project_id: ProjectId,
    branch: String,
    message_id: String,
    chat_messages: Vec<but_llm::ChatMessage>,
    filter: Option<but_forge::ForgeReviewFilter>,
    model: String,
) -> anyhow::Result<String, Error> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    let reviews =
        but_api::legacy::forge::list_reviews_for_branch(ctx.into_sync(), branch.clone(), filter)
            .await?;
    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    let git_config =
        gix::config::File::from_globals().map_err(|e| Error::from(anyhow::anyhow!(e)))?;
    let llm = but_llm::LLMProvider::from_git_config(&git_config);
    match llm {
        Some(llm) => but_bot::forge_branch_chat(
            project_id,
            branch,
            message_id,
            emitter,
            &llm,
            model,
            chat_messages,
            reviews,
        )
        .map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => Err(Error::from(anyhow::anyhow!(
            "No valid credentials found for AI provider. Please configure your GitButler account credentials."
        ))),
    }
}
