use but_api::{json::Error, legacy::askpass};
use but_askpass::{AskpassRequestId, Context, PromptEvent};
use but_core::ref_metadata::StackId;
use serde::Serialize;
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

/// A credential prompt in the shape Lite reads from the SDK.
#[derive(Debug, Clone, Serialize)]
pub struct LitePromptEvent {
    id: AskpassRequestId,
    prompt: String,
    context: LiteContext,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum LiteContext {
    Push {
        #[serde(skip_serializing_if = "Option::is_none")]
        branch_id: Option<StackId>,
    },
    Fetch {
        action: String,
    },
    SignedCommit {
        #[serde(skip_serializing_if = "Option::is_none")]
        branch_id: Option<StackId>,
    },
    Clone {
        url: String,
    },
}

impl From<PromptEvent<Context>> for LitePromptEvent {
    fn from(event: PromptEvent<Context>) -> Self {
        let context = match event.context {
            Context::Push { branch_id } => LiteContext::Push { branch_id },
            Context::Fetch { action } => LiteContext::Fetch { action },
            Context::SignedCommit { branch_id } => LiteContext::SignedCommit { branch_id },
            Context::Clone { url } => LiteContext::Clone { url },
        };
        LitePromptEvent {
            id: event.id,
            prompt: event.prompt,
            context,
        }
    }
}
