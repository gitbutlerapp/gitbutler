use std::ops::Deref;

use anyhow::{Context, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};

use but_tools::tool::Toolset;
use gitbutler_secret::{Sensitive, secret};
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum ChatMessage {
    User(String),
    Assistant(String),
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
    tool_set: &mut Toolset,
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

            let tool_response = tool_set
                .call_tool(&function_name, &function_args)
                .with_context(|| {
                    format!(
                        "Failed to call tool {} with arguments {}",
                        function_name, function_args
                    )
                })?;

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
