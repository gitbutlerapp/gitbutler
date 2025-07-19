use crate::RequestContext;
use gitbutler_id::id::Id;
use gitbutler_repo_actions::askpass::{self, AskpassRequest};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitPromptParams {
    id: Id<AskpassRequest>,
    response: Option<String>,
}

pub async fn submit_prompt_response(
    _ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: SubmitPromptParams = serde_json::from_value(params)?;
    askpass::get_broker()
        .handle_response(params.id, params.response)
        .await;
    Ok(serde_json::Value::Null)
}
