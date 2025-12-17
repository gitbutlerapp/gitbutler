use but_api::{json::Error, legacy::askpass};
use gitbutler_repo_actions::askpass::AskpassRequestId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(response))]
pub async fn submit_prompt_response(
    id: AskpassRequestId,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::submit_prompt_response(askpass::SubmitPromptResponseParams { id, response })
        .await
        .map_err(Into::into)
}
