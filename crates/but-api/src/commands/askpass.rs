//! In place of commands.rs
use gitbutler_repo_actions::askpass::{self, AskpassRequestId};
use serde::Deserialize;

use crate::json::Error;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPromptResponseParams {
    pub id: AskpassRequestId,
    pub response: Option<String>,
}

pub async fn submit_prompt_response(params: SubmitPromptResponseParams) -> Result<(), Error> {
    askpass::get_broker()
        .handle_response(params.id, params.response)
        .await;
    Ok(())
}
