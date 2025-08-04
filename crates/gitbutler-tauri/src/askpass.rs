use but_api::{commands::askpass, App};
use gitbutler_id::id::Id;
use gitbutler_repo_actions::askpass::AskpassRequest;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app, response))]
pub async fn submit_prompt_response(
    app: State<'_, App>,
    id: Id<AskpassRequest>,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::submit_prompt_response(&app, askpass::SubmitPromptResponseParams { id, response })
        .await
}
