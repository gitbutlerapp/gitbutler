use but_api::{commands::askpass, IpcContext};
use gitbutler_id::id::Id;
use gitbutler_repo_actions::askpass::AskpassRequest;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx, response))]
pub async fn submit_prompt_response(
    ipc_ctx: State<'_, IpcContext>,
    id: Id<AskpassRequest>,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::submit_prompt_response(
        &ipc_ctx,
        askpass::SubmitPromptResponseParams { id, response },
    )
    .await
}
