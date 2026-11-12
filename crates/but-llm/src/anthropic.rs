use anyhow::Result;
use but_secret::{Sensitive, secret};
use reqwest::{
    Response,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;

use crate::{
    AI_ANTHROPIC_SECRET_HANDLE, GITBUTLER_ACCESS_TOKEN_HANDLE, chat::ChatMessage, client::LLMClient,
};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
pub const GB_ANTHROPIC_API_BASE: &str = "https://app.gitbutler.com/api/proxy/anthropic";

#[derive(Debug, Clone)]
pub enum CredentialsKind {
    OwnAnthropicKey,
    GitButlerProxied,
}

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    api_base: &'static str,
    client: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(kind: CredentialsKind, credentials: &Sensitive<String>) -> Result<Self> {
        let (api_base, headers) = endpoint(&kind, credentials);
        let client = but_http::client_builder()?
            .default_headers(headers)
            .build()?;

        Ok(Self { api_base, client })
    }

    async fn message(&self, request: &AnthropicRequest) -> Result<AnthropicResponse> {
        let response = self.message_raw(request).await?;
        let anthropic_response: AnthropicResponse = response.json().await?;
        Ok(anthropic_response)
    }

    async fn message_raw(&self, request: &AnthropicRequest) -> Result<Response> {
        let api_base = self.api_base;
        let response = self
            .client
            .post(format!("{api_base}/messages"))
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Anthropic API error ({status}): {error_text}");
        }

        Ok(response)
    }
}

/// Where requests go, and the headers they send there. GitButler's proxy knows the user by
/// their GitButler token in `X-Auth-Token`, as GitButler's other endpoints do, and sends its
/// own key on to Anthropic.
fn endpoint(kind: &CredentialsKind, credentials: &Sensitive<String>) -> (&'static str, HeaderMap) {
    let (api_base, auth) = match kind {
        CredentialsKind::GitButlerProxied => (GB_ANTHROPIC_API_BASE, "X-Auth-Token"),
        CredentialsKind::OwnAnthropicKey => (ANTHROPIC_API_BASE, "x-api-key"),
    };
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        auth,
        credentials
            .0
            .parse()
            .unwrap_or(HeaderValue::from_static("")),
    );
    headers.insert(
        "anthropic-version",
        HeaderValue::from_static(ANTHROPIC_VERSION),
    );
    (api_base, headers)
}

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    model: Option<String>,
    credentials: (CredentialsKind, Sensitive<String>),
}

impl AnthropicProvider {
    pub fn with(kind: CredentialsKind, model: Option<String>) -> Option<Self> {
        let credentials = match kind {
            CredentialsKind::OwnAnthropicKey => AnthropicProvider::anthropic_own_key_creds(),
            CredentialsKind::GitButlerProxied => AnthropicProvider::gitbutler_proxied_creds(),
        };

        match credentials {
            Ok(credentials) => Some(Self { credentials, model }),
            Err(e) => {
                tracing::error!("Failed to retrieve Anthropic credentials: {}", e);
                None
            }
        }
    }

    pub fn client(&self) -> Result<AnthropicClient> {
        let credentials = &self.credentials.1;
        let kind = self.credentials.0.clone();
        AnthropicClient::new(kind, credentials)
    }

    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve(GITBUTLER_ACCESS_TOKEN_HANDLE, secret::Namespace::BuildKind)?
            .ok_or(anyhow::anyhow!(
                "No GitButler token available. Log-in to use the GitButler Anthropic provider"
            ))?;
        Ok((CredentialsKind::GitButlerProxied, creds))
    }

    fn anthropic_own_key_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve(AI_ANTHROPIC_SECRET_HANDLE, secret::Namespace::Global)?
            .ok_or(anyhow::anyhow!(
                "No Anthropic own key configured. Add this through the GitButler settings"
            ))?;
        Ok((CredentialsKind::OwnAnthropicKey, creds))
    }
}

impl LLMClient for AnthropicProvider {
    fn model(&self) -> Option<String> {
        self.model.clone()
    }

    fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<Option<String>> {
        stream_response_blocking(self, system_message, chat_messages, model, on_token)
    }

    fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>> {
        structured_output_blocking::<T>(self, system_message, chat_messages, model)
    }

    fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<String>> {
        response_blocking(self, system_message, chat_messages, model)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String,
    content: Vec<AnthropicContentBlock>,
    stop_reason: Option<String>,
    usage: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<serde_json::Value>,
}

impl From<ChatMessage> for AnthropicMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => AnthropicMessage {
                role: "user".to_string(),
                content: serde_json::json!([{"type": "text", "text": content}]),
            },
        }
    }
}

pub fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    anthropic: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let client = anthropic.client()?;
    let messages: Vec<AnthropicMessage> = chat_messages
        .into_iter()
        .map(AnthropicMessage::from)
        .collect();
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(structured_output::<T>(
                &client,
                messages,
                model,
                system_message,
            ))
    })
    .join()
    .unwrap()
}

async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
    client: &AnthropicClient,
    messages: Vec<AnthropicMessage>,
    model: String,
    system_message: String,
) -> anyhow::Result<Option<T>> {
    let schema = schema_for!(T);

    // For now, we'll use a system prompt to request JSON output
    // Anthropic doesn't have native structured output like OpenAI, but we can guide it
    let enhanced_system = format!(
        "{}\n\nPlease respond with valid JSON matching this schema: {}",
        system_message,
        serde_json::to_string_pretty(&schema)?
    );

    let request = AnthropicRequest {
        model,
        max_tokens: 4096,
        messages,
        system: Some(enhanced_system),
        stream: None,
    };

    let anthropic_response = client.message(&request).await?;

    for block in anthropic_response.content {
        if block.block_type == "text"
            && let Some(text) = block.text
        {
            let json_text = if text.trim().starts_with("```json") {
                text.trim()
                    .strip_prefix("```json")
                    .and_then(|s| s.strip_suffix("```"))
                    .unwrap_or(&text)
                    .trim()
            } else if text.trim().starts_with("```") {
                text.trim()
                    .strip_prefix("```")
                    .and_then(|s| s.strip_suffix("```"))
                    .unwrap_or(&text)
                    .trim()
            } else {
                text.trim()
            };

            return Ok(Some(serde_json::from_str::<T>(json_text)?));
        }
    }

    Ok(None)
}

fn response_blocking(
    client: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<String>> {
    let messages: Vec<AnthropicMessage> = chat_messages
        .into_iter()
        .map(AnthropicMessage::from)
        .collect();

    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(response(
            &client,
            messages,
            model,
            system_message,
        ))
    })
    .join()
    .unwrap()
}

async fn response(
    client: &AnthropicClient,
    messages: Vec<AnthropicMessage>,
    model: String,
    system_message: String,
) -> anyhow::Result<Option<String>> {
    let request = AnthropicRequest {
        model,
        max_tokens: 4096,
        messages,
        system: Some(system_message),
        stream: None,
    };

    let anthropic_response = client.message(&request).await?;

    for block in anthropic_response.content {
        if block.block_type == "text"
            && let Some(text) = block.text
        {
            return Ok(Some(text));
        }
    }

    Ok(None)
}

pub fn stream_response_blocking(
    client: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    let messages: Vec<AnthropicMessage> = chat_messages
        .into_iter()
        .map(AnthropicMessage::from)
        .collect();

    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(stream_response(
                &client,
                messages,
                model,
                system_message,
                on_token,
            ))
    })
    .join()
    .unwrap()
}

async fn stream_response(
    client: &AnthropicClient,
    messages: Vec<AnthropicMessage>,
    model: String,
    system_message: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    use futures::StreamExt;

    let request = AnthropicRequest {
        model,
        max_tokens: 4096,
        messages,
        system: Some(system_message),
        stream: Some(true),
    };

    let response = client.message_raw(&request).await?;

    let mut stream = response.bytes_stream();
    let mut response_text = String::new();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        let text = String::from_utf8_lossy(&bytes);

        buffer.push_str(&text);

        while let Some(line_end) = buffer.find('\n') {
            let line_owned = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line_owned.is_empty() {
                continue;
            }

            // SSE format: "data: {...}"
            if let Some(data) = line_owned.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }

                if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data)
                    && event.event_type.as_str() == "content_block_delta"
                    && let Some(delta) = event.delta
                    && let Some(text) = delta.get("text").and_then(|t| t.as_str())
                {
                    response_text.push_str(text);
                    on_token(text);
                }
            }
        }
    }

    if response_text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(response_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_endpoint_gets_the_token_in_the_header_it_reads() {
        let token = Sensitive("token".to_owned());
        let (api_base, headers) = endpoint(&CredentialsKind::GitButlerProxied, &token);
        assert_eq!(api_base, GB_ANTHROPIC_API_BASE);
        assert_eq!(
            headers.get("X-Auth-Token").map(HeaderValue::as_bytes),
            Some(b"token".as_slice()),
            "app.gitbutler.com knows the user only by X-Auth-Token and answers 401 without it"
        );

        let (api_base, headers) = endpoint(&CredentialsKind::OwnAnthropicKey, &token);
        assert_eq!(api_base, ANTHROPIC_API_BASE);
        assert_eq!(
            headers.get("x-api-key").map(HeaderValue::as_bytes),
            Some(b"token".as_slice()),
            "Anthropic's own API takes the user's key as x-api-key"
        );
    }
}
