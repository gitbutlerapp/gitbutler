use std::{collections::HashMap, fmt::Display, ops::Deref, sync::Arc};

use anyhow::{Context, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};

use but_tools::tool::Toolset;
use futures::StreamExt;
use gitbutler_secret::{Sensitive, secret};
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

#[allow(unused)]
#[derive(Debug, Clone, serde::Serialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarOpenAiKey,
    OwnOpenAiKey,
    GitButlerProxied,
}

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    credentials: (CredentialsKind, Sensitive<String>),
}

impl OpenAiProvider {
    pub fn with(preferred_creds: Option<CredentialsKind>) -> Option<Self> {
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
            Ok(credentials) => Some(Self { credentials }),
            Err(e) => {
                tracing::error!("Failed to retrieve OpenAI credentials: {}", e);
                None
            }
        }
    }

    pub fn client(&self) -> Result<Client<OpenAIConfig>> {
        match &self.credentials {
            (CredentialsKind::EnvVarOpenAiKey, _) => Ok(Client::with_config(OpenAIConfig::new())),
            (CredentialsKind::OwnOpenAiKey, key) => Ok(Client::with_config(
                OpenAIConfig::new().with_api_key(key.0.clone()),
            )),
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

#[allow(dead_code)]
pub fn structured_output_blocking<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    openai: &OpenAiProvider,
    messages: Vec<ChatCompletionRequestMessage>,
) -> anyhow::Result<Option<T>> {
    let client = openai.client()?;
    let messages_owned = messages.clone();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(structured_output::<T>(&client, messages_owned))
    })
    .join()
    .unwrap()
}

#[allow(dead_code)]
pub async fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema>(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
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
        .model("gpt-4.1-mini")
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

#[allow(dead_code)]
pub fn tool_calling_blocking(
    client: &OpenAiProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::ChatCompletionTool>,
    model: Option<String>,
) -> anyhow::Result<async_openai::types::CreateChatCompletionResponse> {
    let client = client.client()?;
    let messages_owned = messages.clone();
    let tools_owned = tools.clone();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool_calling(&client, messages_owned, tools_owned, model))
    })
    .join()
    .unwrap()
}

#[allow(dead_code)]
pub async fn tool_calling(
    client: &Client<OpenAIConfig>,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::ChatCompletionTool>,
    model: Option<String>,
) -> anyhow::Result<async_openai::types::CreateChatCompletionResponse> {
    let model = model.unwrap_or_else(|| "gpt-4.1-mini".to_string());
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages.clone())
        .tools(tools)
        .build()?;

    let response = client.chat().create(request).await?;

    Ok(response)
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

type StreamToolCallResult = (Option<Vec<ToolCall>>, Option<String>);

pub fn tool_calling_stream_blocking(
    client: &OpenAiProvider,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<async_openai::types::ChatCompletionTool>,
    model: Option<String>,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let client = client.client()?;
    let messages_owned = messages.clone();
    let tools_owned = tools.clone();

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
    tools: Vec<async_openai::types::ChatCompletionTool>,
    model: Option<String>,
    on_token: impl Fn(&str) + Send + Sync + 'static,
) -> anyhow::Result<StreamToolCallResult> {
    let model = model.unwrap_or_else(|| "gpt-4.1-mini".to_string());
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
            if let Some(finish_reason) = &chat_choice.finish_reason {
                if matches!(finish_reason, async_openai::types::FinishReason::ToolCalls) {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallContent {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponseContent {
    pub id: String,
    pub result: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    ToolCall(ToolCallContent),
    ToolResponse(ToolResponseContent),
}

impl From<ChatMessage> for ChatCompletionRequestMessage {
    fn from(msg: ChatMessage) -> Self {
        match msg {
            ChatMessage::User(content) => ChatCompletionRequestMessage::User(content.into()),
            ChatMessage::Assistant(content) => ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    content: Some(content.into()),
                    ..Default::default()
                },
            ),
            ChatMessage::ToolCall(content) => ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    content: None,
                    tool_calls: Some(vec![async_openai::types::ChatCompletionMessageToolCall {
                        id: content.id,
                        r#type: async_openai::types::ChatCompletionToolType::Function,
                        function: async_openai::types::FunctionCall {
                            name: content.name,
                            arguments: content.arguments,
                        },
                    }]),
                    ..Default::default()
                },
            ),
            ChatMessage::ToolResponse(content) => ChatCompletionRequestMessage::Tool(
                async_openai::types::ChatCompletionRequestToolMessage {
                    tool_call_id: content.id,
                    content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                        content.result,
                    ),
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
                        chat_messages.push(ChatMessage::ToolCall(ToolCallContent {
                            id: tool_call.id.clone(),
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.clone(),
                        }));
                    }
                }

                if let Some(ChatCompletionRequestAssistantMessageContent::Text(text)) =
                    assistant_msg.content
                {
                    chat_messages.push(ChatMessage::Assistant(text));
                }
            }
            ChatCompletionRequestMessage::Tool(tool_msg) => {
                if let async_openai::types::ChatCompletionRequestToolMessageContent::Text(text) =
                    tool_msg.content
                {
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

fn clamp_result_content(result: &ToolResponseContent) -> String {
    if result.result.len() > 500 {
        "Result too big to be displayed".to_string()
    } else {
        result.result.to_owned()
    }
}

impl Display for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMessage::User(content) => write!(f, "<user_message>\n{}\n</user_message>", content),
            ChatMessage::Assistant(content) => write!(f, "<but-bot\n{}\n</but-bot>", content),
            ChatMessage::ToolCall(content) => write!(
                f,
                "
<but-bot-tool-call>
    <id>{}</id>
    <name>{}</name>
    <arguments>{}</arguments>
</but-bot-tool-call>",
                content.id, content.name, content.arguments
            ),
            ChatMessage::ToolResponse(content) => write!(
                f,
                "
<but-bot-tool-response>
    <id>{}</id>
    <result>{}</result>
</but-bot-tool-response>",
                content.id,
                clamp_result_content(content)
            ),
        }
    }
}

impl From<&str> for ChatMessage {
    fn from(msg: &str) -> Self {
        ChatMessage::User(msg.to_string())
    }
}

impl From<String> for ChatMessage {
    fn from(msg: String) -> Self {
        ChatMessage::User(msg)
    }
}

pub fn tool_calling_loop(
    provider: &OpenAiProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: Option<String>,
) -> anyhow::Result<async_openai::types::CreateChatCompletionResponse> {
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
        .collect::<Result<Vec<async_openai::types::ChatCompletionTool>, _>>()?;

    let mut response = crate::openai::tool_calling_blocking(
        provider,
        messages.clone(),
        open_ai_tools.clone(),
        model.clone(),
    )?;

    while let Some(tool_calls) = response
        .choices
        .first()
        .and_then(|choice| choice.message.tool_calls.as_ref())
    {
        let mut tool_calls_messages: Vec<async_openai::types::ChatCompletionMessageToolCall> =
            vec![];
        let mut tool_response_messages: Vec<async_openai::types::ChatCompletionRequestMessage> =
            vec![];

        for call in tool_calls {
            let function_name = call.function.name.clone();
            let function_args = call.function.arguments.clone();

            let tool_response = tool_set.call_tool(&function_name, &function_args);

            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            tool_calls_messages.push(async_openai::types::ChatCompletionMessageToolCall {
                id: call.id.clone(),
                r#type: async_openai::types::ChatCompletionToolType::Function,
                function: async_openai::types::FunctionCall {
                    name: function_name,
                    arguments: function_args,
                },
            });

            tool_response_messages.push(async_openai::types::ChatCompletionRequestMessage::Tool(
                async_openai::types::ChatCompletionRequestToolMessage {
                    tool_call_id: call.id.clone(),
                    content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                        tool_response_str,
                    ),
                },
            ));
        }

        messages.push(
            async_openai::types::ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    tool_calls: Some(tool_calls_messages),
                    ..Default::default()
                },
            ),
        );

        messages.extend(tool_response_messages);

        response = crate::openai::tool_calling_blocking(
            provider,
            messages.clone(),
            open_ai_tools.clone(),
            model.clone(),
        )?;
    }

    Ok(response)
}

pub fn tool_calling_loop_stream(
    provider: &OpenAiProvider,
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
    tool_set: &mut impl Toolset,
    model: Option<String>,
    on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
) -> anyhow::Result<(String, Vec<ChatMessage>)> {
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
        .collect::<Result<Vec<async_openai::types::ChatCompletionTool>, _>>()?;

    let on_token_cb = {
        let on_token = on_token.clone();
        Box::new(move |token: &str| on_token(token)) as Box<dyn Fn(&str) + Send + Sync + 'static>
    };

    let mut response = tool_calling_stream_blocking(
        provider,
        messages.clone(),
        open_ai_tools.clone(),
        model.clone(),
        on_token_cb,
    )?;

    let mut text_response_buffer = vec![];
    if let Some(text_response) = response.1.clone() {
        text_response_buffer.push(text_response.clone());
        messages.push(ChatCompletionRequestMessage::Assistant(
            async_openai::types::ChatCompletionRequestAssistantMessage {
                content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                    text_response,
                )),
                ..Default::default()
            },
        ));
    }

    while let Some(tool_calls) = response.0 {
        let mut tool_calls_messages: Vec<async_openai::types::ChatCompletionMessageToolCall> =
            vec![];
        let mut tool_response_messages: Vec<async_openai::types::ChatCompletionRequestMessage> =
            vec![];

        for call in tool_calls {
            let ToolCall {
                id,
                name: function_name,
                arguments: function_args,
            } = call;

            let tool_response = tool_set.call_tool(&function_name, &function_args);

            let tool_response_str = serde_json::to_string(&tool_response)
                .context("Failed to serialize tool response")?;

            tool_calls_messages.push(async_openai::types::ChatCompletionMessageToolCall {
                id: id.clone(),
                r#type: async_openai::types::ChatCompletionToolType::Function,
                function: async_openai::types::FunctionCall {
                    name: function_name,
                    arguments: function_args,
                },
            });

            tool_response_messages.push(async_openai::types::ChatCompletionRequestMessage::Tool(
                async_openai::types::ChatCompletionRequestToolMessage {
                    tool_call_id: id.clone(),
                    content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                        tool_response_str,
                    ),
                },
            ));
        }

        messages.push(
            async_openai::types::ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    tool_calls: Some(tool_calls_messages),
                    ..Default::default()
                },
            ),
        );

        messages.extend(tool_response_messages);

        let on_token_cb = {
            let on_token = on_token.clone();
            Box::new(move |token: &str| on_token(token))
                as Box<dyn Fn(&str) + Send + Sync + 'static>
        };
        response = tool_calling_stream_blocking(
            provider,
            messages.clone(),
            open_ai_tools.clone(),
            model.clone(),
            on_token_cb,
        )?;

        if let Some(text_response) = response.1.clone() {
            text_response_buffer.push(text_response.clone());
            messages.push(ChatCompletionRequestMessage::Assistant(
                async_openai::types::ChatCompletionRequestAssistantMessage {
                    content: Some(ChatCompletionRequestAssistantMessageContent::Text(
                        text_response,
                    )),
                    ..Default::default()
                },
            ));
        }
    }

    let chat_messages = from_openai_chat_messages(messages);
    let text_response = text_response_buffer
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n");
    Ok((text_response, chat_messages))
}
