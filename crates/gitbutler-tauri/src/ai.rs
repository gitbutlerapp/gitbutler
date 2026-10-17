use anyhow::Context as _;
use but_api::json::Error;
use tauri::ipc::Channel;

#[tauri::command]
pub async fn stream_ai_response(
    system_message: String,
    prompt: String,
    on_token: Channel<String>,
) -> Result<String, Error> {
    tauri::async_runtime::spawn_blocking(move || {
        but_api::ai::stream_ai_response(&system_message, &prompt, move |token| {
            if let Err(error) = on_token.send(token.to_owned()) {
                tracing::warn!(%error, "AI token could not reach the webview");
            }
        })
    })
    .await
    .context("AI response task failed")
    .and_then(|result| result)
    .map_err(Into::into)
}
