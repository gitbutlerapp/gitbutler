use but_tools::tool::Toolset;
use std::ops::Deref;
use std::sync::Arc;
use std::vec;
use uuid::Uuid;

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use serde_json::Value;

use crate::client::LLMClient;
use crate::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent};

#[derive(Debug, Clone, Default)]
pub struct OllamaProvider {
    client: Ollama,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaHostConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaConfig {
    pub host_config: Option<OllamaHostConfig>,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Self {
        let client = if let Some(host_config) = config.host_config {
            Ollama::new(host_config.host, host_config.port)
        } else {
            Ollama::default()
        };

        Self { client }
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
    fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl Toolset,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
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
}

pub fn tool_calling_loop_stream(
    provider: &OllamaProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: String,
    on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
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

    let on_token_cb = {
        let on_token = on_token.clone();
        Box::new(move |token: &str| on_token(token)) as Box<dyn Fn(&str) + Send + Sync + 'static>
    };

    let mut response = tool_calling_stream_blocking(
        provider,
        messages.clone(),
        ollama_tool_infos.clone(),
        model.clone(),
        on_token_cb,
    )?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = response.1.clone() {
        text_response_buffer.push(text_response.clone());
        messages.push(ollama_rs::generation::chat::ChatMessage::assistant(
            text_response,
        ));
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

        let on_token_cb = {
            let on_token = on_token.clone();
            Box::new(move |token: &str| on_token(token))
                as Box<dyn Fn(&str) + Send + Sync + 'static>
        };
        response = tool_calling_stream_blocking(
            provider,
            messages.clone(),
            ollama_tool_infos.clone(),
            model.clone(),
            on_token_cb,
        )?;

        if let Some(text_response) = response.1.clone() {
            text_response_buffer.push(text_response.clone());
            messages.push(ollama_rs::generation::chat::ChatMessage::assistant(
                text_response,
            ));
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

fn tool_calling_stream_blocking(
    provider: &OllamaProvider,
    messages: Vec<ollama_rs::generation::chat::ChatMessage>,
    tool_infos: Vec<ollama_rs::generation::tools::ToolInfo>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> Result<StreamToolCallResult> {
    let provider = provider.clone();

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
    use std::sync::{Arc, Mutex};

    let history = Arc::new(Mutex::new(vec![]));
    let ollama = provider.client();
    let request = ChatMessageRequest::new(model, messages).tools(tool_infos);

    let mut resp = ollama
        .send_chat_messages_with_history_stream(history.clone(), request)
        .await?;

    let mut response_text: Option<String> = None;
    let mut tool_call_results: Option<Vec<ToolCall>> = None;

    while let Some(chunk) = resp.next().await {
        match chunk {
            Ok(part) => {
                let content = part.message.content;
                on_token(&content);
                if let Some(ref mut text) = response_text {
                    text.push_str(&content);
                } else {
                    response_text = Some(content);
                }

                if !part.message.tool_calls.is_empty() {
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
