use but_tools::tool::Toolset;
use ollama_rs::generation::parameters::{FormatType, JsonStructure};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::vec;
use uuid::Uuid;

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use serde_json::Value;

use crate::client::LLMClient;
use crate::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent};

const OLLAMA_ENDPOINT: &str = "gitbutler.aiOllamaEndpoint";
const OLLAMA_MODEL_NAME: &str = "gitbutler.aiOllamaModelName";

#[derive(Debug, Clone, Default)]
pub struct OllamaProvider {
    model: Option<String>,
    client: Ollama,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaHostConfig {
    pub host: String,
    pub port: u16,
}

impl From<String> for OllamaHostConfig {
    fn from(endpoint: String) -> Self {
        let parts: Vec<&str> = endpoint.split(':').collect();
        let host = parts.first().cloned().unwrap_or("localhost").to_string();
        let port = parts
            .get(1)
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(11434);
        OllamaHostConfig { host, port }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct OllamaConfig {
    pub host_config: Option<OllamaHostConfig>,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig, model: Option<String>) -> Self {
        let client = if let Some(host_config) = config.host_config {
            Ollama::new(host_config.host, host_config.port)
        } else {
            Ollama::default()
        };

        Self { client, model }
    }

    pub fn client(&self) -> &Ollama {
        &self.client
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        match self.client.list_local_models().await {
            Ok(models) => Ok(models.into_iter().map(|m| m.name).collect()),
            Err(e) => Err(anyhow::anyhow!("Failed to list models: {}", e)),
        }
    }
}

impl LLMClient for OllamaProvider {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self>
    where
        Self: Sized,
    {
        let endpoint = config
            .string(OLLAMA_ENDPOINT)
            .map(|v| v.to_string())
            .map(OllamaHostConfig::from);
        let model = config.string(OLLAMA_MODEL_NAME).map(|v| v.to_string());
        let ollama_config = OllamaConfig {
            host_config: endpoint,
        };
        Some(OllamaProvider::new(ollama_config, model))
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
        tool_calling_loop_stream(
            self,
            system_message,
            chat_messages,
            tool_set,
            model,
            on_token,
        )
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

    fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>> {
        structured_output_blocking(self, system_message, chat_messages, model)
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

pub fn tool_calling_loop_stream(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<(String, Vec<ChatMessage>)> {
    let mut messages: Vec<ollama_rs::generation::chat::ChatMessage> =
        vec![ollama_rs::generation::chat::ChatMessage::system(
            system_message.to_string(),
        )];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ollama_rs::generation::chat::ChatMessage::from),
    );

    let ollama_tool_infos = tool_set
        .list()
        .into_iter()
        .map(|t| t.deref().try_into())
        .collect::<Result<Vec<ollama_rs::generation::tools::ToolInfo>, _>>()?;

    // Needs to be Arc to be cloned into async closures
    let on_token = Arc::new(on_token);

    let mut response = tool_calling_stream_blocking(
        provider,
        messages.clone(),
        ollama_tool_infos.clone(),
        model,
        {
            let on_token = Arc::clone(&on_token);
            move |s: &str| on_token(s)
        },
    )?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = &response.1 {
        handle_text_response(&mut messages, &mut text_response_buffer, text_response);
    }

    while let Some(tool_calls) = response.0 {
        for call in tool_calls {
            let ToolCall {
                id: _,
                name: function_name,
                arguments: function_args,
            } = call;

            let tool_response = tool_set.call_tool(&function_name, &function_args);
            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            messages.push(ollama_rs::generation::chat::ChatMessage {
                role: ollama_rs::generation::chat::MessageRole::Assistant,
                content: "".to_string(),
                tool_calls: vec![ollama_rs::generation::tools::ToolCall {
                    function: ollama_rs::generation::tools::ToolCallFunction {
                        name: function_name,
                        arguments: serde_json::from_str(&function_args).unwrap_or(Value::Null),
                    },
                }],
                images: None,
                thinking: None,
            });

            messages.push(ollama_rs::generation::chat::ChatMessage::tool(
                tool_response_str,
            ));
        }

        response = tool_calling_stream_blocking(
            provider,
            messages.clone(),
            ollama_tool_infos.clone(),
            model,
            {
                let on_token = Arc::clone(&on_token);
                move |s: &str| on_token(s)
            },
        )?;

        if let Some(text_response) = &response.1 {
            handle_text_response(&mut messages, &mut text_response_buffer, text_response);
        }
    }

    let chat_messages = from_ollama_chat_messages(messages);

    let text_response = text_response_buffer
        .into_iter()
        .filter(|s: &String| !s.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n");

    Ok((text_response, chat_messages))
}

fn handle_text_response(
    messages: &mut Vec<ollama_rs::generation::chat::ChatMessage>,
    text_response_buffer: &mut Vec<String>,
    text_response: &str,
) {
    text_response_buffer.push(text_response.to_string());
    messages.push(ollama_rs::generation::chat::ChatMessage::assistant(
        text_response.to_string(),
    ));
}

fn tool_calling_stream_blocking(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    tool_infos: Vec<ollama_rs::generation::tools::ToolInfo>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<StreamToolCallResult> {
    let provider = provider.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling_stream(
                &provider, messages, tool_infos, model, on_token,
            ))
    })
    .join()
    .unwrap()
}

/// Streams a chat completion response from Ollama, handling tool calls.
async fn tool_calling_stream(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    tool_infos: Vec<ollama_rs::generation::tools::ToolInfo>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<StreamToolCallResult> {
    let history = Arc::new(Mutex::new(vec![]));
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages).tools(tool_infos);

    let mut resp = ollama
        .send_chat_messages_with_history_stream(history, request)
        .await?;

    let mut response_text: Option<String> = None;
    let mut tool_call_results: Option<Vec<ToolCall>> = None;

    let on_token = Arc::new(on_token);

    // The streaming implementation of ollama_rs returns chunks as they come, and then the complete response at the end.
    // This is annoying, but we can work around it by collecting the content from each chunk and only emitting if the content is new.
    while let Some(chunk) = resp.next().await {
        match chunk {
            Ok(part) => {
                let content = part.message.content;
                let is_last_chunk = process_token_response(
                    {
                        let on_token = Arc::clone(&on_token);
                        move |s: &str| on_token(s)
                    },
                    &mut response_text,
                    content,
                );

                // If it's the last chunk (e.g. the full response), we don't need to process tool calls again.
                if !part.message.tool_calls.is_empty() && !is_last_chunk {
                    for tool_call in part.message.tool_calls {
                        let tool_call_result = ToolCall {
                            id: generate_tool_id(),
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.to_string(),
                        };
                        if let Some(ref mut tool_calls) = tool_call_results {
                            tool_calls.push(tool_call_result);
                        } else {
                            tool_call_results = Some(vec![tool_call_result]);
                        }
                    }
                }
            }
            Err(_) => {
                bail!("Error during Ollama streaming response",);
            }
        }
    }

    Ok((tool_call_results, response_text))
}

fn stream_response_blocking(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<Option<String>> {
    let provider = provider.clone();
    let model = model.to_string();
    let mut messages: Vec<ollama_rs::generation::chat::ChatMessage> =
        vec![ollama_rs::generation::chat::ChatMessage::system(
            system_message.to_string(),
        )];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ollama_rs::generation::chat::ChatMessage::from),
    );

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(stream_response(&provider, messages, model, on_token))
    })
    .join()
    .unwrap()
}

/// Streams a chat completion response from Ollama
async fn stream_response(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<Option<String>> {
    let history = Arc::new(Mutex::new(vec![]));
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages);

    let mut resp = ollama
        .send_chat_messages_with_history_stream(history.clone(), request)
        .await?;

    let mut response_text: Option<String> = None;

    let on_token = Arc::new(on_token);

    // The streaming implementation of ollama_rs returns chunks as they come, and then the complete response at the end.
    // This is annoying, but we can work around it by collecting the content from each chunk and only emitting if the content is new.
    while let Some(chunk) = resp.next().await {
        match chunk {
            Ok(part) => {
                let content = part.message.content;
                process_token_response(
                    {
                        let on_token = Arc::clone(&on_token);
                        move |s: &str| on_token(s)
                    },
                    &mut response_text,
                    content,
                );
            }
            Err(_) => {
                bail!("Error during Ollama streaming response",);
            }
        }
    }

    Ok(response_text)
}

/// Processes a token response chunk, emitting it if it's new.
///
/// Returns true if the processed content was the determined to be the last one.
fn process_token_response(
    on_token: impl Fn(&str) + Send + Sync + 'static,
    response_text: &mut Option<String>,
    content: String,
) -> bool {
    let last_chunk = Some(content.clone()) == *response_text;
    // Don't emit empty strings or the last chunk containing the full response again.
    // We can tell that it's the last chunk because it will be equal to the accumulated response_text.
    if !content.is_empty() && !last_chunk {
        on_token(&content);
        if let Some(ref mut text) = *response_text {
            text.push_str(&content);
        } else {
            *response_text = Some(content);
        }
    }

    last_chunk
}

fn response_blocking(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<String>> {
    let provider = provider.clone();
    let model = model.to_string();
    let mut messages: Vec<ollama_rs::generation::chat::ChatMessage> =
        vec![ollama_rs::generation::chat::ChatMessage::system(
            system_message.to_string(),
        )];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ollama_rs::generation::chat::ChatMessage::from),
    );

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(response(&provider, messages, model))
    })
    .join()
    .unwrap()
}

async fn response(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    model: String,
) -> anyhow::Result<Option<String>> {
    let mut history = vec![];
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages);

    let response = ollama
        .send_chat_messages_with_history(&mut history, request)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    if response.message.content.is_empty() {
        Ok(None)
    } else {
        Ok(Some(response.message.content))
    }
}

fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let provider = provider.clone();
    let model = model.to_string();
    let mut messages: Vec<ollama_rs::generation::chat::ChatMessage> =
        vec![ollama_rs::generation::chat::ChatMessage::system(
            system_message.to_string(),
        )];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ollama_rs::generation::chat::ChatMessage::from),
    );

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(structured_output::<T>(&provider, messages, model))
    })
    .join()
    .unwrap()
}

async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    model: String,
) -> anyhow::Result<Option<T>> {
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<T>()));

    let mut history = vec![];
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages).format(format);

    let response = ollama
        .send_chat_messages_with_history(&mut history, request)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    match serde_json::from_str(&response.message.content) {
        Ok(result) => Ok(Some(result)),
        Err(_) => Ok(None),
    }
}

fn tool_calling_loop(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
) -> Result<String> {
    let mut messages: Vec<ollama_rs::generation::chat::ChatMessage> =
        vec![ollama_rs::generation::chat::ChatMessage::system(
            system_message.to_string(),
        )];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ollama_rs::generation::chat::ChatMessage::from),
    );

    let ollama_tool_infos = tool_set
        .list()
        .into_iter()
        .map(|t| t.deref().try_into())
        .collect::<Result<Vec<ollama_rs::generation::tools::ToolInfo>, _>>()?;

    let mut response =
        tool_calling_blocking(provider, messages.clone(), ollama_tool_infos.clone(), model)?;

    let mut text_response_buffer = vec![];
    if !response.message.content.is_empty() {
        let text_response = response.message.content.clone();
        text_response_buffer.push(text_response.clone());
        messages.push(ollama_rs::generation::chat::ChatMessage::assistant(
            text_response,
        ));
    }

    while !response.message.tool_calls.is_empty() {
        for call in response.message.tool_calls.into_iter() {
            let function_name = call.function.name;
            let function_args = call.function.arguments.to_string();
            let tool_response = tool_set.call_tool(&function_name, &function_args);
            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            messages.push(ollama_rs::generation::chat::ChatMessage {
                role: ollama_rs::generation::chat::MessageRole::Assistant,
                content: "".to_string(),
                tool_calls: vec![ollama_rs::generation::tools::ToolCall {
                    function: ollama_rs::generation::tools::ToolCallFunction {
                        name: function_name,
                        arguments: serde_json::from_str(&function_args).unwrap_or(Value::Null),
                    },
                }],
                images: None,
                thinking: None,
            });

            messages.push(ollama_rs::generation::chat::ChatMessage::tool(
                tool_response_str,
            ));
        }

        response =
            tool_calling_blocking(provider, messages.clone(), ollama_tool_infos.clone(), model)?;

        if !response.message.content.is_empty() {
            let text_response = response.message.content.clone();
            text_response_buffer.push(text_response.clone());
            messages.push(ollama_rs::generation::chat::ChatMessage::assistant(
                text_response,
            ));
        }
    }

    let text_response = text_response_buffer
        .into_iter()
        .filter(|s: &String| !s.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n");

    Ok(text_response)
}

fn tool_calling_blocking(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    tool_infos: Vec<ollama_rs::generation::tools::ToolInfo>,
    model: &str,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse> {
    let provider = provider.clone();
    let model = model.to_string();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling(&provider, messages, tool_infos, model))
    })
    .join()
    .unwrap()
}

async fn tool_calling(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    tool_infos: Vec<ollama_rs::generation::tools::ToolInfo>,
    model: String,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse> {
    let mut history = vec![];
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages).tools(tool_infos);

    ollama
        .send_chat_messages_with_history(&mut history, request)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

fn generate_tool_id() -> String {
    Uuid::new_v4().to_string()
}

impl From<ChatMessage> for ollama_rs::generation::chat::ChatMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => ollama_rs::generation::chat::ChatMessage::user(content),
            ChatMessage::Assistant(content) => {
                ollama_rs::generation::chat::ChatMessage::assistant(content)
            }
            ChatMessage::ToolCall(tool_call) => ollama_rs::generation::chat::ChatMessage {
                role: ollama_rs::generation::chat::MessageRole::Tool,
                content: "".to_string(),
                tool_calls: vec![ollama_rs::generation::tools::ToolCall {
                    function: ollama_rs::generation::tools::ToolCallFunction {
                        name: tool_call.name,
                        arguments: serde_json::from_str(&tool_call.arguments)
                            .unwrap_or(Value::Null),
                    },
                }],
                images: None,
                thinking: None,
            },
            ChatMessage::ToolResponse(tool_response) => {
                ollama_rs::generation::chat::ChatMessage::tool(tool_response.result)
            }
        }
    }
}

/// Converts Ollama chat messages to our ChatMessage format, including tool calls.
///
/// TODO: The tool call IDs are meaningless UUIDs here. We might want to improve this later.
fn from_ollama_chat_messages(
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
) -> Vec<ChatMessage> {
    let mut chat_messages = vec![];
    for msg in messages.into_iter() {
        match msg.role {
            ollama_rs::generation::chat::MessageRole::System => continue,
            ollama_rs::generation::chat::MessageRole::User => {
                chat_messages.push(ChatMessage::User(msg.content))
            }
            ollama_rs::generation::chat::MessageRole::Assistant => {
                chat_messages.push(ChatMessage::Assistant(msg.content));
                if !msg.tool_calls.is_empty() {
                    for call in msg.tool_calls {
                        let tool_response = ToolCallContent {
                            id: generate_tool_id(),
                            name: call.function.name,
                            arguments: call.function.arguments.to_string(),
                        };
                        chat_messages.push(ChatMessage::ToolCall(tool_response));
                    }
                }
            }
            ollama_rs::generation::chat::MessageRole::Tool => {
                if !msg.content.is_empty() {
                    chat_messages.push(ChatMessage::Assistant(msg.content));
                }
                if !msg.tool_calls.is_empty() {
                    for call in msg.tool_calls {
                        let tool_call = ToolCallContent {
                            id: generate_tool_id(),
                            name: call.function.name,
                            arguments: call.function.arguments.to_string(),
                        };
                        chat_messages.push(ChatMessage::ToolCall(tool_call));
                    }
                }
            }
        }
    }

    chat_messages
}
