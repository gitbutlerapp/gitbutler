use but_api::commands::askpass;
use gitbutler_id::id::Id;
use gitbutler_repo_actions::askpass::AskpassRequest;

use but_api::error::Error;

#[tauri::command(async)]
pub async fn submit_prompt_response(
    id: Id<AskpassRequest>,
    response: Option<String>,
) -> Result<(), Error> {
    askpass::submit_prompt_response(id, response).await
}
