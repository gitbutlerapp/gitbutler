use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::{Context, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageContent,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};
use but_tools::tool::Toolset;
use futures::StreamExt;
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::{
    ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent,
    chat::ConversationResult,
};

pub trait OpenAIClientProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>>;
}

impl From<ChatMessage> for ChatCompletionRequestMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => ChatCompletionRequestMessage::User(content.into()),
            ChatMessage::Assistant(content) => ChatCompletionRequestMessage::Assistant(
                async_openai::types::chat::ChatCompletionRequestAssistantMessage {
                    content: Some(content.into()),
                    ..Default::default()
                },
            ),
            ChatMessage::ToolCall(content) => ChatCompletionRequestMessage::Assistant(
                async_openai::types::chat::ChatCompletionRequestAssistantMessage {
                    content: None,
                    tool_calls: Some(vec![ChatCompletionMessageToolCalls::Function(
                        async_openai::types::chat::ChatCompletionMessageToolCall {
                            id: content.id,
                            function: async_openai::types::chat::FunctionCall {
                                name: content.name,
                                arguments: content.arguments,
                            },
                        },
                    )]),
                    ..Default::default()
                },
            ),
            ChatMessage::ToolResponse(content) => ChatCompletionRequestMessage::Tool(
                async_openai::types::chat::ChatCompletionRequestToolMessage {
                    tool_call_id: content.id,
                    content: ChatCompletionRequestToolMessageContent::Text(content.result),
                },
            ),
        }
    }
}

pub fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let client = provider.client()?;
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];
    let model = model.to_string();

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(structured_output::<T>(&client, messages, model))
    })
    .join()
    .unwrap()
}

pub fn response_blocking(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<String>> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let client = provider.client()?;
    let messages_owned = messages.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(response(&client, messages_owned, model))
    })
    .join()
    .unwrap()
}

pub fn tool_calling_blocking(
    provider: &impl OpenAIClientProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: &str,
) -> anyhow::Result<async_openai::types::chat::CreateChatCompletionResponse> {
    let client = provider.client()?;
    let messages_owned = messages.clone();
    let tools_owned = tools.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling(&client, messages_owned, tools_owned, model))
    })
    .join()
    .unwrap()
}

pub fn stream_response_blocking(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let client = provider.client()?;
    let messages_owned = messages.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(stream_response(&client, messages_owned, model, on_token))
    })
    .join()
    .unwrap()
}

pub fn tool_calling_stream_blocking(
    provider: &impl OpenAIClientProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let client = provider.client()?;
    let messages_owned = messages.clone();
    let tools_owned = tools.clone();
    let model = model.to_string();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling_stream(
                &client,
                messages_owned,
                tools_owned,
                model,
                on_token,
            ))
    })
    .join()
    .unwrap()
}
pub fn tool_calling_loop(
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
) -> Result<String> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let open_ai_tools = tool_set
        .list()
        .iter()
        .map(|t| t.deref().try_into())
        .collect::<Result<Vec<async_openai::types::chat::ChatCompletionTools>, _>>()?;

    let mut response =
        tool_calling_blocking(provider, messages.clone(), open_ai_tools.clone(), model)?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
    {
        handle_text_response(text_response, &mut text_response_buffer, &mut messages);
    }

    while let Some(tool_calls) = response
        .choices
        .first()
        .and_then(|choice| choice.message.tool_calls.as_ref())
    {
        let mut tool_calls_messages: Vec<ChatCompletionMessageToolCalls> = vec![];
        let mut tool_response_messages: Vec<ChatCompletionRequestMessage> = vec![];

        for call in tool_calls {
            // Extract function call from the enum
            let (id, function_name, function_args) = match call {
                ChatCompletionMessageToolCalls::Function(func_call) => (
                    func_call.id.clone(),
                    func_call.function.name.clone(),
                    func_call.function.arguments.clone(),
                ),
                ChatCompletionMessageToolCalls::Custom(custom) => {
                    tracing::warn!(?custom, "Encountered unexpected custom tool call");
                    continue;
                }
            };

            let tool_response = tool_set.call_tool(&function_name, &function_args);
            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            tool_calls_messages.push(ChatCompletionMessageToolCalls::Function(
                async_openai::types::chat::ChatCompletionMessageToolCall {
                    id: id.clone(),
                    function: async_openai::types::chat::FunctionCall {
                        name: function_name,
                        arguments: function_args,
                    },
                },
            ));

            tool_response_messages.push(ChatCompletionRequestMessage::Tool(
                async_openai::types::chat::ChatCompletionRequestToolMessage {
                    tool_call_id: id.clone(),
                    content: ChatCompletionRequestToolMessageContent::Text(tool_response_str),
                },
            ));
        }

        messages.push(ChatCompletionRequestMessage::Assistant(
            async_openai::types::chat::ChatCompletionRequestAssistantMessage {
                tool_calls: Some(tool_calls_messages),
                ..Default::default()
            },
        ));

        messages.extend(tool_response_messages);

        response = tool_calling_blocking(provider, messages.clone(), open_ai_tools.clone(), model)?;

        if let Some(text_response) = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
        {
            handle_text_response(text_response, &mut text_response_buffer, &mut messages);
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
    provider: &impl OpenAIClientProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<ConversationResult> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );

    let open_ai_tools = tool_set
        .list()
        .iter()
        .map(|t| t.deref().try_into())
        .collect::<Result<Vec<async_openai::types::chat::ChatCompletionTools>, _>>()?;

    let on_token = Arc::new(on_token);

    let mut response =
        tool_calling_stream_blocking(provider, messages.clone(), open_ai_tools.clone(), model, {
            let on_token = Arc::clone(&on_token);
            move |s: &str| on_token(s)
        })?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = response.1 {
        handle_text_response(text_response, &mut text_response_buffer, &mut messages);
    }

    while let Some(tool_calls) = response.0 {
        let mut tool_calls_messages: Vec<
            async_openai::types::chat::ChatCompletionMessageToolCalls,
        > = vec![];
        let mut tool_response_messages: Vec<
            async_openai::types::chat::ChatCompletionRequestMessage,
        > = vec![];

        for call in tool_calls {
            let ToolCall {
                id,
                name: function_name,
                arguments: function_args,
            } = call;

            let tool_response = tool_set.call_tool(&function_name, &function_args);

            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            tool_calls_messages.push(
                async_openai::types::chat::ChatCompletionMessageToolCalls::Function(
                    async_openai::types::chat::ChatCompletionMessageToolCall {
                        id: id.clone(),
                        function: async_openai::types::chat::FunctionCall {
                            name: function_name,
                            arguments: function_args,
                        },
                    },
                ),
            );

            tool_response_messages.push(
                async_openai::types::chat::ChatCompletionRequestMessage::Tool(
                    async_openai::types::chat::ChatCompletionRequestToolMessage {
                        tool_call_id: id.clone(),
                        content: ChatCompletionRequestToolMessageContent::Text(tool_response_str),
                    },
                ),
            );
        }

        messages.push(
            async_openai::types::chat::ChatCompletionRequestMessage::Assistant(
                async_openai::types::chat::ChatCompletionRequestAssistantMessage {
                    tool_calls: Some(tool_calls_messages),
                    ..Default::default()
                },
            ),
        );

        messages.extend(tool_response_messages);

        response = tool_calling_stream_blocking(
            provider,
            messages.clone(),
            open_ai_tools.clone(),
            model,
            {
                let on_token = Arc::clone(&on_token);
                move |s: &str| on_token(s)
            },
        )?;

        if let Some(text_response) = response.1 {
            handle_text_response(text_response, &mut text_response_buffer, &mut messages);
        }
    }

    let chat_messages = from_openai_chat_messages(messages);
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

fn from_openai_chat_messages(messages: Vec<ChatCompletionRequestMessage>) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();

    for m in messages.into_iter() {
        match m {
            ChatCompletionRequestMessage::User(content) => {
                if let ChatCompletionRequestUserMessageContent::Text(text) = content.content {
                    chat_messages.push(ChatMessage::User(text));
                }
            }
            ChatCompletionRequestMessage::Assistant(assistant_msg) => {
                if let Some(tool_calls) = &assistant_msg.tool_calls {
                    for tool_call in tool_calls {
                        if let ChatCompletionMessageToolCalls::Function(func_call) = tool_call {
                            chat_messages.push(ChatMessage::ToolCall(ToolCallContent {
                                id: func_call.id.clone(),
                                name: func_call.function.name.clone(),
                                arguments: func_call.function.arguments.clone(),
                            }));
                        } else {
                            tracing::warn!(
                                ?tool_call,
                                "Encountered unexpected non-function tool call"
                            );
                        }
                    }
                }

                if let Some(ChatCompletionRequestAssistantMessageContent::Text(text)) =
                    assistant_msg.content
                {
                    chat_messages.push(ChatMessage::Assistant(text));
                }
            }
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                if let ChatCompletionRequestToolMessageContent::Text(text) = tool_msg.content {
                    chat_messages.push(ChatMessage::ToolResponse(ToolResponseContent {
                        id: tool_msg.tool_call_id.clone(),
                        result: text,
                    }));
                }
            }
            _ => (),
        }
    }

    chat_messages
}

/// Helper function to handle text response from streaming
fn handle_text_response(
    text_response: String,
    text_response_buffer: &mut Vec<String>,
    messages: &mut Vec<ChatCompletionRequestMessage>,
) {
    text_response_buffer.push(text_response.clone());
    messages.push(ChatCompletionRequestMessage::Assistant(
        async_openai::types::chat::ChatCompletionRequestAssistantMessage {
            content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                text_response,
            )),
            ..Default::default()
        },
    ));
}

async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> anyhow::Result<Option<T>> {
    let schema = schema_for!(T);
    let schema_value = serde_json::to_value(&schema)?;
    let response_format = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: None,
            name: "structured_response".into(),
            schema: Some(schema_value),
            strict: Some(false),
        },
    };

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .response_format(response_format)
        .build()?;

    let response = client.chat().create(request).await?;

    for choice in response.choices {
        if let Some(content) = choice.message.content {
            return Ok(Some(serde_json::from_str::<T>(&content)?));
        }
    }

    Ok(None)
}

async fn response(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> anyhow::Result<Option<String>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .build()?;

    let response = client.chat().create(request).await?;

    for choice in response.choices {
        if let Some(content) = choice.message.content {
            return Ok(Some(content));
        }
    }

    Ok(None)
}

async fn tool_calling(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: String,
) -> anyhow::Result<async_openai::types::chat::CreateChatCompletionResponse> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.clone())
        .tools(tools)
        .build()?;

    let response = client.chat().create(request).await?;

    Ok(response)
}

async fn stream_response(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<Option<String>> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.clone())
        .build()?;

    let mut stream = client.chat().create_stream(request).await?;

    let mut response_text: Option<String> = None;

    while let Some(result) = stream.next().await {
        let response = result.context("Failed to receive response from OpenAI stream")?;
        if let Some(chat_choice) = response.choices.first() {
            // If there is any text content in the response, call the on_token callback
            if let Some(content) = &chat_choice.delta.content {
                if response_text.is_none() {
                    response_text = Some(String::new());
                }

                if let Some(text) = response_text.as_mut() {
                    text.push_str(content);
                }

                let content_str = content.as_str();
                on_token(content_str);
            }
        }
    }

    Ok(response_text)
}

async fn tool_calling_stream(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: String,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.clone())
        .tools(tools)
        .build()?;

    let mut stream = client.chat().create_stream(request).await?;

    let tool_call_states: Arc<Mutex<HashMap<(u32, u32), ToolCall>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut response_text: Option<String> = None;

    while let Some(result) = stream.next().await {
        let response = result.context("Failed to receive response from OpenAI stream")?;
        if let Some(chat_choice) = response.choices.first() {
            // Keep track of tool call states
            if let Some(tool_calls) = &chat_choice.delta.tool_calls {
                for tool_call_chunk in tool_calls.iter() {
                    let key = (chat_choice.index, tool_call_chunk.index);
                    let states = tool_call_states.clone();
                    let tool_call_data = tool_call_chunk.clone();

                    let mut states_lock = states.lock().await;
                    let state = states_lock.entry(key).or_insert_with(|| ToolCall {
                        id: tool_call_data.id.clone().unwrap_or_default(),
                        name: tool_call_data
                            .function
                            .as_ref()
                            .and_then(|f| f.name.clone())
                            .unwrap_or_default(),
                        arguments: tool_call_data
                            .function
                            .as_ref()
                            .and_then(|f| f.arguments.clone())
                            .unwrap_or_default(),
                    });

                    if let Some(arguments) = tool_call_chunk
                        .function
                        .as_ref()
                        .and_then(|f| f.arguments.as_ref())
                    {
                        state.arguments.push_str(arguments);
                    }
                }
            }

            // If finished streaming the tool calls, return them.
            if let Some(finish_reason) = &chat_choice.finish_reason
                && matches!(
                    finish_reason,
                    async_openai::types::chat::FinishReason::ToolCalls
                )
            {
                let tool_call_states_clone = tool_call_states.clone();

                let tool_calls_to_process = {
                    let states_lock = tool_call_states_clone.lock().await;
                    states_lock
                        .values()
                        .map(|state| ToolCall {
                            id: state.id.clone(),
                            name: state.name.clone(),
                            arguments: state.arguments.clone(),
                        })
                        .collect::<Vec<ToolCall>>()
                };

                return Ok((Some(tool_calls_to_process), response_text));
            }

            // If there is any text content in the response, call the on_token callback
            if let Some(content) = &chat_choice.delta.content {
                if response_text.is_none() {
                    response_text = Some(String::new());
                }

                if let Some(text) = response_text.as_mut() {
                    text.push_str(content);
                }

                let content_str = content.as_str();
                on_token(content_str);
            }
        }
    }

    Ok((None, response_text))
}
