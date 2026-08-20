//! App-level AI streaming.
//!
//! The configuration calls live in `but-api`, where the `#[but_api]` macro
//! generates their wrappers. Streaming stays here: it takes a callback, which
//! that macro does not carry across the boundary.

use anyhow::{Context as _, Result, bail};
use but_llm::{ChatMessage, LLMProvider};
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
    let response = tokio::task::spawn_blocking(move || -> Result<String> {
        let config = gix::config::File::from_globals()?;
        let provider = LLMProvider::from_git_config(&config)
            .context("AI provider is not completely configured")?;
        let model = provider.model_or_default();
        let response = provider.stream_response(
            required_message(&system_message, "System message")?,
            vec![ChatMessage::User(
                required_message(&prompt, "Prompt")?.into(),
            )],
            &model,
            move |token| {
                let status = on_token.call(
                    Ok(token.to_string()),
                    ThreadsafeFunctionCallMode::NonBlocking,
                );
                if status != Status::Ok {
                    tracing::warn!(%status, "AI token callback failed");
                }
            },
        )?;
        response
            .filter(|response| !response.trim().is_empty())
            .context("AI provider returned an empty response")
    })
    .await
    .context("AI response task failed")
    .and_then(|result| result)
    .map_err(to_napi_err)?;

    Ok(response)
}

fn required_message<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{field} is required")
    }
    Ok(value)
}
