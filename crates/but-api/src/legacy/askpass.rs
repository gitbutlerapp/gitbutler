//! In place of commands.rs
use gitbutler_repo_actions::askpass::{self, AskpassRequestId};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitPromptResponseParams {
    pub id: AskpassRequestId,
    pub response: Option<String>,
}

pub async fn submit_prompt_response(params: SubmitPromptResponseParams) -> anyhow::Result<()> {
    if let Some(broker) = askpass::get_broker() {
        broker.handle_response(params.id, params.response).await;
    } else {
        tracing::warn!("received askpass response but broker is not initialized; ignoring");
    }
    Ok(())
}
