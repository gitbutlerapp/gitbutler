use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::{Context as _, Result};
use but_secret::{Sensitive, secret};
use but_tools::tool::Toolset;
use reqwest::{
    Response,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;

use crate::{
    StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent, chat::ChatMessage, client::LLMClient,
    key::CredentialsKeyOption,
};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_KEY_OPTION: &str = "gitbutler.aiAnthropicKeyOption";
const ANTHROPIC_MODEL_NAME: &str = "gitbutler.aiAnthropicModelName";
pub const GB_ANTHROPIC_API_BASE: &str = "https://app.gitbutler.com/api/proxy/anthropic";

/// Result of a tool calling loop with streaming
pub struct ConversationResult {
    pub final_response: String,
    pub message_history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarAnthropicKey,
    OwnAnthropicKey,
    GitButlerProxied,
}

impl CredentialsKind {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self> {
        let key_option_str = config.string(ANTHROPIC_KEY_OPTION).map(|v| v.to_string())?;
        let key_option = CredentialsKeyOption::from_str(&key_option_str)?;
        match key_option {
            CredentialsKeyOption::BringYourOwn => Some(CredentialsKind::OwnAnthropicKey),
            CredentialsKeyOption::ButlerApi => Some(CredentialsKind::GitButlerProxied),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    kind: CredentialsKind,
    client: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(kind: CredentialsKind, credentials: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            credentials.0.parse().unwrap_or(HeaderValue::from_static("")),
        );
        headers.insert("anthropic-version", HeaderValue::from_static(ANTHROPIC_VERSION));

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        Ok(Self { kind, client })
    }

    /// Send a message request to the Anthropic Messages API
    async fn message(&self, request: &AnthropicRequest) -> Result<AnthropicResponse> {
        let response = self.message_raw(request).await?;
        let anthropic_response: AnthropicResponse = response.json().await?;
        Ok(anthropic_response)
    }

    async fn message_raw(&self, request: &AnthropicRequest) -> Result<Response> {
        let api_base = self.api_base();
        let response = self
            .client
            .post(format!("{}/messages", api_base))
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        Ok(response)
    }

    fn api_base(&self) -> &str {
        match self.kind {
            CredentialsKind::GitButlerProxied => GB_ANTHROPIC_API_BASE,
            _ => ANTHROPIC_API_BASE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    model: Option<String>,
    credentials: (CredentialsKind, Sensitive<String>),
}

impl AnthropicProvider {
    pub fn with(preferred_creds: Option<CredentialsKind>, model: Option<String>) -> Option<Self> {
        let credentials = if let Some(kind) = preferred_creds {
            match kind {
                CredentialsKind::EnvVarAnthropicKey => AnthropicProvider::anthropic_env_var_creds(),
                CredentialsKind::OwnAnthropicKey => AnthropicProvider::anthropic_own_key_creds(),
                CredentialsKind::GitButlerProxied => AnthropicProvider::gitbutler_proxied_creds(),
            }
        } else {
            AnthropicProvider::gitbutler_proxied_creds()
                .or_else(|_| AnthropicProvider::anthropic_own_key_creds())
                .or_else(|_| AnthropicProvider::anthropic_env_var_creds())
                .context("No Anthropic credentials found. This can be configured in the app or read from a ANTHROPIC_API_KEY environment variable")
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

    pub fn credentials_kind(&self) -> CredentialsKind {
        self.credentials.0.clone()
    }
    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?.ok_or(
            anyhow::anyhow!("No GitButler token available. Log-in to use the GitButler Anthropic provider"),
        )?;
        Ok((CredentialsKind::GitButlerProxied, creds))
    }

    fn anthropic_own_key_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("aiAnthropicKey", secret::Namespace::Global)?.ok_or(anyhow::anyhow!(
            "No Anthropic own key configured. Add this through the GitButler settings"
        ))?;
        Ok((CredentialsKind::OwnAnthropicKey, creds))
    }

    fn anthropic_env_var_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = Sensitive(
            std::env::var_os("ANTHROPIC_API_KEY")
                .ok_or(anyhow::anyhow!("Environment variable ANTHROPIC_API_KEY is not set"))?
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in ANTHROPIC_API_KEY"))?,
        );
        Ok((CredentialsKind::EnvVarAnthropicKey, creds))
    }
}

impl LLMClient for AnthropicProvider {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self>
    where
        Self: Sized,
    {
        let credentials_kind = CredentialsKind::from_git_config(config)?;
        let model = config.string(ANTHROPIC_MODEL_NAME).map(|v| v.to_string());
        AnthropicProvider::with(Some(credentials_kind), model)
    }

    fn model(&self) -> Option<String> {
        self.model.clone()
    }

    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<(String, Vec<ChatMessage>)> {
        let result = tool_calling_loop_stream(self, system_message, chat_messages, tool_set, model, on_token)?;
        Ok((result.final_response, result.message_history))
    }

    fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: &str,
    ) -> Result<String> {
        tool_calling_loop(self, system_message, chat_messages, tool_set, model)
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

    fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static>(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>> {
        structured_output_blocking::<T>(self, system_message, chat_messages, model)
    }

    fn response(&self, system_message: &str, chat_messages: Vec<ChatMessage>, model: &str) -> Result<Option<String>> {
        response_blocking(self, system_message, chat_messages, model)
    }
}

// Anthropic API types
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
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<serde_json::Value>,
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
    index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_block: Option<AnthropicContentBlock>,
}

impl From<ChatMessage> for AnthropicMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => AnthropicMessage {
                role: "user".to_string(),
                content: serde_json::json!([{"type": "text", "text": content}]),
            },
            ChatMessage::Assistant(content) => AnthropicMessage {
                role: "assistant".to_string(),
                content: serde_json::json!([{"type": "text", "text": content}]),
            },
            ChatMessage::ToolCall(content) => AnthropicMessage {
                role: "assistant".to_string(),
                content: serde_json::json!([{
                    "type": "tool_use",
                    "id": content.id,
                    "name": content.name,
                    "input": serde_json::from_str::<serde_json::Value>(&content.arguments).unwrap_or(serde_json::json!({}))
                }]),
            },
            ChatMessage::ToolResponse(content) => AnthropicMessage {
                role: "user".to_string(),
                content: serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": content.id,
                    "content": content.result
                }]),
            },
        }
    }
}

fn from_anthropic_messages(messages: Vec<AnthropicMessage>) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();

    for msg in messages {
        match msg.role.as_str() {
            "user" => {
                if let Ok(blocks) = serde_json::from_value::<Vec<AnthropicContentBlock>>(msg.content) {
                    for block in blocks {
                        match block.block_type.as_str() {
                            "text" => {
                                if let Some(text) = block.text {
                                    chat_messages.push(ChatMessage::User(text));
                                }
                            }
                            "tool_result" => {
                                if let (Some(id), Some(input)) = (block.id, block.input) {
                                    let result = input.as_str().unwrap_or("").to_string();
                                    chat_messages.push(ChatMessage::ToolResponse(ToolResponseContent { id, result }));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "assistant" => {
                if let Ok(blocks) = serde_json::from_value::<Vec<AnthropicContentBlock>>(msg.content) {
                    for block in blocks {
                        match block.block_type.as_str() {
                            "text" => {
                                if let Some(text) = block.text {
                                    chat_messages.push(ChatMessage::Assistant(text));
                                }
                            }
                            "tool_use" => {
                                if let (Some(id), Some(name), Some(input)) = (block.id, block.name, block.input) {
                                    let arguments = serde_json::to_string(&input).unwrap_or_default();
                                    chat_messages.push(ChatMessage::ToolCall(ToolCallContent { id, name, arguments }));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    chat_messages
}

pub fn structured_output_blocking<T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static>(
    anthropic: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let client = anthropic.client()?;
    let messages: Vec<AnthropicMessage> = chat_messages.into_iter().map(AnthropicMessage::from).collect();
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(structured_output::<T>(
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
        tools: None,
        stream: None,
    };

    let anthropic_response = client.message(&request).await?;

    for block in anthropic_response.content {
        if block.block_type == "text"
            && let Some(text) = block.text
        {
            // Try to extract JSON from the text
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
    let messages: Vec<AnthropicMessage> = chat_messages.into_iter().map(AnthropicMessage::from).collect();

    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(response(&client, messages, model, system_message))
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
        tools: None,
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
    let messages: Vec<AnthropicMessage> = chat_messages.into_iter().map(AnthropicMessage::from).collect();

    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(stream_response(
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
        tools: None,
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

        // Process complete lines
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

fn tool_calling_blocking(
    client: &AnthropicProvider,
    messages: Vec<AnthropicMessage>,
    tools: Vec<serde_json::Value>,
    model: &str,
    system_message: &str,
) -> anyhow::Result<AnthropicResponse> {
    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling(&client, messages, tools, model, system_message))
    })
    .join()
    .unwrap()
}

async fn tool_calling(
    client: &AnthropicClient,
    messages: Vec<AnthropicMessage>,
    tools: Vec<serde_json::Value>,
    model: String,
    system_message: String,
) -> anyhow::Result<AnthropicResponse> {
    let request = AnthropicRequest {
        model,
        max_tokens: 4096,
        messages,
        system: Some(system_message),
        tools: Some(tools),
        stream: None,
    };

    let anthropic_response = client.message(&request).await?;
    Ok(anthropic_response)
}

fn tool_calling_stream_blocking(
    client: &AnthropicProvider,
    messages: Vec<AnthropicMessage>,
    tools: Vec<serde_json::Value>,
    model: &str,
    system_message: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let client = client.client()?;
    let model = model.to_string();
    let system_message = system_message.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(tool_calling_stream(
            &client,
            messages,
            tools,
            model,
            system_message,
            on_token,
        ))
    })
    .join()
    .unwrap()
}

async fn tool_calling_stream(
    client: &AnthropicClient,
    messages: Vec<AnthropicMessage>,
    tools: Vec<serde_json::Value>,
    model: String,
    system_message: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    use futures::StreamExt;

    let request = AnthropicRequest {
        model,
        max_tokens: 4096,
        messages,
        system: Some(system_message),
        tools: Some(tools),
        stream: Some(true),
    };

    let response = client.message_raw(&request).await?;

    let mut stream = response.bytes_stream();
    let mut response_text = String::new();
    let mut buffer = String::new();
    let mut tool_calls: HashMap<String, ToolCall> = HashMap::new();
    let mut current_tool_id: Option<String> = None;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        let text = String::from_utf8_lossy(&bytes);

        buffer.push_str(&text);

        // Process complete lines
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

                if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                    match event.event_type.as_str() {
                        "content_block_start" => {
                            if let Some(block) = event.content_block
                                && block.block_type == "tool_use"
                                && let (Some(id), Some(name)) = (block.id, block.name)
                            {
                                current_tool_id = Some(id.clone());
                                tool_calls.insert(
                                    id.clone(),
                                    ToolCall {
                                        id,
                                        name,
                                        arguments: String::new(),
                                    },
                                );
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event.delta {
                                // Handle text delta
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    response_text.push_str(text);
                                    on_token(text);
                                }

                                // Handle tool input delta
                                if let Some(partial_json) = delta.get("partial_json").and_then(|t| t.as_str())
                                    && let Some(tool_id) = &current_tool_id
                                    && let Some(tool_call) = tool_calls.get_mut(tool_id)
                                {
                                    tool_call.arguments.push_str(partial_json);
                                }
                            }
                        }
                        "message_stop" => {
                            // Check if we have tool calls to return
                            if !tool_calls.is_empty() {
                                let calls: Vec<ToolCall> = tool_calls.values().cloned().collect();
                                let text = if response_text.is_empty() {
                                    None
                                } else {
                                    Some(response_text.clone())
                                };
                                return Ok((Some(calls), text));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // If we have tool calls, return them
    if !tool_calls.is_empty() {
        let calls: Vec<ToolCall> = tool_calls.values().cloned().collect();
        let text = if response_text.is_empty() {
            None
        } else {
            Some(response_text)
        };
        Ok((Some(calls), text))
    } else {
        // Otherwise return just the text
        let text = if response_text.is_empty() {
            None
        } else {
            Some(response_text)
        };
        Ok((None, text))
    }
}

/// Helper function to handle text response from streaming
fn handle_text_response(
    text_response: String,
    text_response_buffer: &mut Vec<String>,
    messages: &mut Vec<AnthropicMessage>,
) {
    text_response_buffer.push(text_response.clone());
    messages.push(AnthropicMessage {
        role: "assistant".to_string(),
        content: serde_json::json!([{"type": "text", "text": text_response}]),
    });
}

fn convert_tool_to_anthropic_format(tool: &dyn but_tools::tool::Tool) -> serde_json::Value {
    serde_json::json!({
        "name": tool.name(),
        "description": tool.description(),
        "input_schema": tool.parameters()
    })
}

pub fn tool_calling_loop(
    provider: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
) -> Result<String> {
    let mut messages: Vec<AnthropicMessage> = chat_messages.into_iter().map(AnthropicMessage::from).collect();

    let anthropic_tools: Vec<serde_json::Value> = tool_set
        .list()
        .iter()
        .map(|t| convert_tool_to_anthropic_format(t.deref()))
        .collect();

    let mut response = tool_calling_blocking(
        provider,
        messages.clone(),
        anthropic_tools.clone(),
        model,
        system_message,
    )?;

    let mut text_response_buffer = vec![];

    // Check for text content
    for block in &response.content {
        if block.block_type == "text"
            && let Some(text) = &block.text
        {
            handle_text_response(text.clone(), &mut text_response_buffer, &mut messages);
        }
    }

    // Loop while we have tool_use blocks
    while response.content.iter().any(|b| b.block_type == "tool_use") {
        let mut tool_call_blocks = vec![];
        let mut tool_response_messages = vec![];

        for block in &response.content {
            if block.block_type == "tool_use"
                && let (Some(id), Some(name), Some(input)) = (&block.id, &block.name, &block.input)
            {
                let arguments = serde_json::to_string(input).context("Failed to serialize tool input")?;

                let tool_response = tool_set.call_tool(name, &arguments);
                let tool_response_str =
                    serde_json::to_string(&tool_response).context("Failed to serialize tool response")?;

                tool_call_blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input
                }));

                tool_response_messages.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": tool_response_str
                }));
            }
        }

        // Add assistant message with tool calls
        if !tool_call_blocks.is_empty() {
            messages.push(AnthropicMessage {
                role: "assistant".to_string(),
                content: serde_json::Value::Array(tool_call_blocks),
            });

            // Add user message with tool results
            messages.push(AnthropicMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(tool_response_messages),
            });
        }

        response = tool_calling_blocking(
            provider,
            messages.clone(),
            anthropic_tools.clone(),
            model,
            system_message,
        )?;

        // Check for text content
        for block in &response.content {
            if block.block_type == "text"
                && let Some(text) = &block.text
            {
                handle_text_response(text.clone(), &mut text_response_buffer, &mut messages);
            }
        }
    }

    let response = text_response_buffer
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n");

    Ok(response)
}

pub fn tool_calling_loop_stream(
    provider: &AnthropicProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<ConversationResult> {
    let mut messages: Vec<AnthropicMessage> = chat_messages.into_iter().map(AnthropicMessage::from).collect();

    let anthropic_tools: Vec<serde_json::Value> = tool_set
        .list()
        .iter()
        .map(|t| convert_tool_to_anthropic_format(t.deref()))
        .collect();

    let on_token = Arc::new(on_token);

    let mut response = tool_calling_stream_blocking(
        provider,
        messages.clone(),
        anthropic_tools.clone(),
        model,
        system_message,
        {
            let on_token = Arc::clone(&on_token);
            move |s: &str| on_token(s)
        },
    )?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = response.1 {
        handle_text_response(text_response, &mut text_response_buffer, &mut messages);
    }

    while let Some(tool_calls) = response.0 {
        let mut tool_call_blocks = vec![];
        let mut tool_response_messages = vec![];

        for call in tool_calls {
            let ToolCall {
                id,
                name: function_name,
                arguments: function_args,
            } = call;

            let tool_response = tool_set.call_tool(&function_name, &function_args);
            let tool_response_str =
                serde_json::to_string(&tool_response).context("Failed to serialize tool response")?;

            let input: serde_json::Value = serde_json::from_str(&function_args).unwrap_or(serde_json::json!({}));

            tool_call_blocks.push(serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": function_name,
                "input": input
            }));

            tool_response_messages.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": id,
                "content": tool_response_str
            }));
        }

        // Add assistant message with tool calls
        messages.push(AnthropicMessage {
            role: "assistant".to_string(),
            content: serde_json::Value::Array(tool_call_blocks),
        });

        // Add user message with tool results
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: serde_json::Value::Array(tool_response_messages),
        });

        response = tool_calling_stream_blocking(
            provider,
            messages.clone(),
            anthropic_tools.clone(),
            model,
            system_message,
            {
                let on_token = Arc::clone(&on_token);
                move |s: &str| on_token(s)
            },
        )?;

        if let Some(text_response) = response.1 {
            handle_text_response(text_response, &mut text_response_buffer, &mut messages);
        }
    }

    let chat_messages = from_anthropic_messages(messages);
    let final_response = text_response_buffer
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(ConversationResult {
        final_response,
        message_history: chat_messages,
    })
}
