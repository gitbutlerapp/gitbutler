//! App-level AI streaming for Node, over [`but_api::ai::stream_ai_response`].

use anyhow::Context as _;
use napi::{
    Status,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use napi_derive::napi;

use crate::to_napi_err;

/// Stream a text response from the configured provider.
#[napi]
pub async fn stream_ai_response(
    system_message: String,
    prompt: String,
    on_token: ThreadsafeFunction<String, ()>,
) -> napi::Result<String> {
    tokio::task::spawn_blocking(move || {
        but_api::ai::stream_ai_response(&system_message, &prompt, move |token| {
            let status = on_token.call(
                Ok(token.to_string()),
                ThreadsafeFunctionCallMode::NonBlocking,
            );
            if status != Status::Ok {
                tracing::warn!(%status, "AI token callback failed");
            }
        })
    })
    .await
    .context("AI response task failed")
    .and_then(|result| result)
    .map_err(to_napi_err)
}
