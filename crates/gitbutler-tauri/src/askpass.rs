use but_api::{commands::askpass, json::Error};
use gitbutler_repo_actions::askpass::AskpassRequestId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(response))]
pub async fn submit_prompt_response(
    id: AskpassRequestId,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::submit_prompt_response(askpass::SubmitPromptResponseParams { id, response }).await
}
