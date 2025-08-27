use but_action::OpenAiProvider;
use but_api::error::Error;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tauri::Emitter;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(app_handle, app), err(Debug))]
pub fn bot(
    app_handle: tauri::AppHandle,
    app: tauri::State<'_, but_api::App>,
    project_id: ProjectId,
    message_id: String,
    chat_messages: Vec<but_action::ChatMessage>,
) -> anyhow::Result<String, Error> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut CommandContext::open(&project, app.app_settings.get()?.clone())?;

    let emitter = std::sync::Arc::new(move |name: &str, payload: serde_json::Value| {
        app_handle.emit(name, payload).unwrap_or_else(|e| {
            tracing::error!("Failed to emit event '{}': {}", name, e);
        });
    });

    let openai = OpenAiProvider::with(Some(but_action::CredentialsKind::GitButlerProxied));
    match openai {
        Some(openai) => but_bot::bot(project_id, message_id, emitter, ctx, &openai, chat_messages).map_err(|e| Error::from(anyhow::anyhow!(e))),
        None => {
            Err(Error::from(anyhow::anyhow!(
                "No valid credentials found for AI provider. Please configure your GitButler account credentials."
            )))
        }
    }
}
