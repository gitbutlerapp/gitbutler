use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::{Context as _, Result};
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
use but_secret::{Sensitive, secret};
use but_tools::tool::Toolset;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::{
    StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent, chat::ChatMessage,
    client::LLMClient, key::CredentialsKeyOption,
};

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

const OPEN_AI_KEY_OPTION: &str = "gitbutler.aiOpenAIKeyOption";
const OPEN_AI_MODEL_NAME: &str = "gitbutler.aiOpenAIModelName";
const OPEN_AI_CUSTOM_ENDPOINT: &str = "gitbutler.aiOpenAICustomEndpoint";

/// Result of a tool calling loop with streaming
pub struct ConversationResult {
    pub final_response: String,
    pub message_history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarOpenAiKey,
    OwnOpenAiKey,
    GitButlerProxied,
}

impl CredentialsKind {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self> {
        let key_option_str = config.string(OPEN_AI_KEY_OPTION).map(|v| v.to_string())?;
        let key_option = CredentialsKeyOption::from_str(&key_option_str)?;
        match key_option {
            CredentialsKeyOption::BringYourOwn => Some(CredentialsKind::OwnOpenAiKey),
            CredentialsKeyOption::ButlerApi => Some(CredentialsKind::GitButlerProxied),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    /// The API endpoint to use, if any. As configured in the git global config
    custom_endpoint: Option<String>,
    /// The preferred model to use, as configured in the git global config
    model: Option<String>,
    credentials: (CredentialsKind, Sensitive<String>),
}

impl OpenAiProvider {
    pub fn with(
        preferred_creds: Option<CredentialsKind>,
        model: Option<String>,
        custom_endpoint: Option<String>,
    ) -> Option<Self> {
        let credentials = if let Some(kind) = preferred_creds {
            match kind {
                CredentialsKind::EnvVarOpenAiKey => OpenAiProvider::openai_env_var_creds(),
                CredentialsKind::OwnOpenAiKey => OpenAiProvider::openai_own_key_creds(),
                CredentialsKind::GitButlerProxied => OpenAiProvider::gitbutler_proxied_creds(),
            }
        } else {
            OpenAiProvider::gitbutler_proxied_creds()
                .or_else(|_| OpenAiProvider::openai_own_key_creds())
                .or_else(|_| OpenAiProvider::openai_env_var_creds())
                .context("No OpenAI credentials found. This can be configured in the app or read from a OPENAI_API_KEY environment variable")
        };

        match credentials {
            Ok(credentials) => Some(Self {
                credentials,
                model,
                custom_endpoint,
            }),
            Err(e) => {
                tracing::error!("Failed to retrieve OpenAI credentials: {}", e);
                None
            }
        }
    }

    pub fn client(&self) -> Result<Client<OpenAIConfig>> {
        match &self.credentials {
            (CredentialsKind::EnvVarOpenAiKey, _) => {
                let config = self.configure_custom_endpoint(OpenAIConfig::new());
                Ok(Client::with_config(config))
            }
            (CredentialsKind::OwnOpenAiKey, key) => {
                let config =
                    self.configure_custom_endpoint(OpenAIConfig::new().with_api_key(key.0.clone()));
                Ok(Client::with_config(config))
            }

            (CredentialsKind::GitButlerProxied, key) => {
                let config = OpenAIConfig::new().with_api_base(GB_OPENAI_API_BASE);
                let mut headers = HeaderMap::new();
                headers.insert(
                    reqwest::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                headers.insert(
                    "X-Auth-Token",
                    key.0.parse().unwrap_or(HeaderValue::from_static("")),
                );
                let http_client = reqwest::Client::builder()
                    .default_headers(headers)
                    .build()?;
                Ok(Client::with_config(config).with_http_client(http_client))
            }
        }
    }

    /// Configure a custom endpoint if set in the provider, if any.
    fn configure_custom_endpoint(&self, config: OpenAIConfig) -> OpenAIConfig {
        if let Some(custom_endpont) = &self.custom_endpoint {
            config.with_api_base(custom_endpont)
        } else {
            config
        }
    }

    pub fn credentials_kind(&self) -> CredentialsKind {
        self.credentials.0.clone()
    }

    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?
            .ok_or(anyhow::anyhow!(
                "No GitButler token available. Log-in to use the GitButler OpenAI provider"
            ))?;
        Ok((CredentialsKind::GitButlerProxied, creds))
    }

    fn openai_own_key_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds =
            secret::retrieve("aiOpenAIKey", secret::Namespace::Global)?.ok_or(anyhow::anyhow!(
                "No OpenAI own key configured. Add this through the GitButler settings"
            ))?;
        Ok((CredentialsKind::OwnOpenAiKey, creds))
    }

    fn openai_env_var_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = Sensitive(
            std::env::var_os("OPENAI_API_KEY")
                .ok_or(anyhow::anyhow!(
                    "Environment variable OPENAI_API_KEY is not set"
                ))?
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in OPENAI_API_KEY"))?,
        );
        Ok((CredentialsKind::EnvVarOpenAiKey, creds))
    }
}

impl LLMClient for OpenAiProvider {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self>
    where
        Self: Sized,
    {
        let credentials_kind = CredentialsKind::from_git_config(config)?;
        let model = config.string(OPEN_AI_MODEL_NAME).map(|v| v.to_string());
        let custom_endpoint = config
            .string(OPEN_AI_CUSTOM_ENDPOINT)
            .map(|v| v.to_string());

        OpenAiProvider::with(Some(credentials_kind), model, custom_endpoint)
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
        let result = tool_calling_loop_stream(
            self,
            system_message,
            chat_messages,
            tool_set,
            model,
            on_token,
        )?;
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

pub fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    openai: &OpenAiProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    model: &str,
) -> anyhow::Result<Option<T>> {
    let client = openai.client()?;
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

pub async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
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

fn response_blocking(
    client: &OpenAiProvider,
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

    let client = client.client()?;
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

pub fn tool_calling_blocking(
    client: &OpenAiProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: &str,
) -> anyhow::Result<async_openai::types::chat::CreateChatCompletionResponse> {
    let client = client.client()?;
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

pub async fn tool_calling(
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

pub async fn stream_response(
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

pub fn stream_response_blocking(
    client: &OpenAiProvider,
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

    let client = client.client()?;
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
    client: &OpenAiProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::chat::ChatCompletionTools>,
    model: &str,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let client = client.client()?;
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

pub async fn tool_calling_stream(
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

pub fn tool_calling_loop(
    provider: &OpenAiProvider,
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

        response = crate::openai::tool_calling_blocking(
            provider,
            messages.clone(),
            open_ai_tools.clone(),
            model,
        )?;

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
    provider: &OpenAiProvider,
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
