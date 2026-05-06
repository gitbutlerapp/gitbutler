use anyhow::{Context as _, Result};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage},
};
use but_secret::{Sensitive, secret};
use but_tools::tool::Toolset;
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::{
    AI_OPENAI_CUSTOM_ENDPOINT_KEY, AI_OPENAI_KEY_OPTION_KEY, AI_OPENAI_MODEL_NAME_KEY,
    AI_OPENAI_SECRET_HANDLE,
    chat::ChatMessage,
    client::LLMClient,
    key::CredentialsKeyOption,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking,
        structured_output_blocking, tool_calling_loop, tool_calling_loop_stream,
    },
};

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";
const KIMI_API_BASE: &str = "https://api.moonshot.ai/v1";
const DEFAULT_KIMI_MAX_COMPLETION_TOKENS: u32 = 1024;

/// OpenAI-compatible API variants that need different request shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenAiApiFlavor {
    /// The official OpenAI API or compatible APIs that accept OpenAI's structured output shape.
    Standard,
    /// Kimi/Moonshot's OpenAI-compatible API, which uses JSON mode and a thinking extension.
    Kimi,
}

impl OpenAiApiFlavor {
    /// Detects the API variant from the configured model and endpoint.
    fn from_model_and_endpoint(model: &str, custom_endpoint: Option<&str>) -> Self {
        if custom_endpoint.is_some_and(|endpoint| endpoint.contains("moonshot.ai"))
            || model.starts_with("kimi-")
        {
            OpenAiApiFlavor::Kimi
        } else {
            OpenAiApiFlavor::Standard
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarOpenAiKey,
    OwnOpenAiKey,
    GitButlerProxied,
}

impl CredentialsKind {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self> {
        let key_option_str = config
            .string(AI_OPENAI_KEY_OPTION_KEY)
            .map(|v| v.to_string())?;
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

    /// Configure a custom endpoint if set in the provider, if any.
    fn configure_custom_endpoint(&self, config: OpenAIConfig) -> OpenAIConfig {
        if let Some(custom_endpont) = &self.custom_endpoint {
            config.with_api_base(custom_endpont)
        } else {
            config
        }
    }

    /// Returns the OpenAI-compatible API variant for the selected model.
    fn api_flavor(&self, model: &str) -> OpenAiApiFlavor {
        OpenAiApiFlavor::from_model_and_endpoint(model, self.custom_endpoint.as_deref())
    }

    /// Returns the direct API base used for a Kimi request.
    fn kimi_api_base(&self) -> String {
        self.custom_endpoint
            .as_deref()
            .filter(|endpoint| !endpoint.trim().is_empty())
            .unwrap_or(KIMI_API_BASE)
            .trim_end_matches('/')
            .to_string()
    }

    /// Returns credentials suitable for a direct OpenAI-compatible request.
    fn direct_openai_key(&self) -> Result<&Sensitive<String>> {
        match &self.credentials {
            (CredentialsKind::EnvVarOpenAiKey | CredentialsKind::OwnOpenAiKey, key) => Ok(key),
            (CredentialsKind::GitButlerProxied, _) => {
                anyhow::bail!("Kimi models require a direct OpenAI-compatible API key")
            }
        }
    }

    /// Generates a plain text response through Kimi's OpenAI-compatible HTTP endpoint.
    fn response_kimi_blocking(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<String>> {
        let api_base = self.kimi_api_base();
        let api_key = self.direct_openai_key()?.0.clone();
        let messages = openai_messages(system_message, chat_messages);
        let model = model.to_string();

        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(response_kimi(api_base, api_key, messages, model))
        })
        .join()
        .unwrap()
    }

    /// Generates structured output through Kimi's OpenAI-compatible HTTP endpoint.
    fn structured_output_kimi_blocking<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>> {
        let api_base = self.kimi_api_base();
        let api_key = self.direct_openai_key()?.0.clone();
        let messages = openai_messages(system_message, chat_messages);
        let model = model.to_string();

        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(structured_output_kimi::<T>(
                    api_base, api_key, messages, model,
                ))
        })
        .join()
        .unwrap()
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
        let creds = secret::retrieve(AI_OPENAI_SECRET_HANDLE, secret::Namespace::Global)?.ok_or(
            anyhow::anyhow!(
                "No OpenAI own key configured. Add this through the GitButler settings"
            ),
        )?;
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

impl OpenAIClientProvider for OpenAiProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>> {
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
}

impl LLMClient for OpenAiProvider {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self>
    where
        Self: Sized,
    {
        let credentials_kind = CredentialsKind::from_git_config(config)?;
        let model = config
            .string(AI_OPENAI_MODEL_NAME_KEY)
            .map(|v| v.to_string());
        let custom_endpoint = config
            .string(AI_OPENAI_CUSTOM_ENDPOINT_KEY)
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
        if self.api_flavor(model) == OpenAiApiFlavor::Kimi {
            let response = self.response_kimi_blocking(system_message, chat_messages, model)?;
            if let Some(response) = &response {
                on_token(response);
            }
            return Ok(response);
        }

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
        if self.api_flavor(model) == OpenAiApiFlavor::Kimi {
            return self.structured_output_kimi_blocking::<T>(system_message, chat_messages, model);
        }

        structured_output_blocking::<T>(self, system_message, chat_messages, model)
    }

    fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<String>> {
        if self.api_flavor(model) == OpenAiApiFlavor::Kimi {
            return self.response_kimi_blocking(system_message, chat_messages, model);
        }

        response_blocking(self, system_message, chat_messages, model)
    }
}

/// Minimal response shape needed from Kimi chat completions.
#[derive(Debug, serde::Deserialize)]
struct KimiChatCompletionResponse {
    /// Completion choices returned by the API.
    choices: Vec<KimiChatCompletionChoice>,
}

/// A single Kimi chat completion choice.
#[derive(Debug, serde::Deserialize)]
struct KimiChatCompletionChoice {
    /// The assistant message for this choice.
    message: KimiChatCompletionMessage,
}

/// Message content returned by a Kimi chat completion.
#[derive(Debug, serde::Deserialize)]
struct KimiChatCompletionMessage {
    /// Serialized JSON content when JSON mode is enabled.
    content: Option<String>,
}

/// Builds the base request body for Kimi chat completions.
fn build_kimi_chat_completion_request(
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> Result<serde_json::Value> {
    Ok(json!({
        "model": model,
        "messages": serde_json::to_value(messages)?,
        "stream": false,
        "max_tokens": DEFAULT_KIMI_MAX_COMPLETION_TOKENS,
        "thinking": {
            "type": "disabled",
        },
    }))
}

/// Builds a Kimi request body for structured JSON output.
fn build_kimi_structured_output_request<T: JsonSchema>(
    mut messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> Result<serde_json::Value> {
    let schema = schema_for!(T);
    let schema_value = serde_json::to_value(&schema)?;
    let schema_prompt = format!(
        "Return only a JSON object matching this JSON Schema:\n```json\n{}\n```",
        serde_json::to_string_pretty(&schema_value)?
    );
    messages.insert(
        1,
        ChatCompletionRequestSystemMessage::from(schema_prompt).into(),
    );

    let mut request = build_kimi_chat_completion_request(messages, model)?;
    request["response_format"] = json!({ "type": "json_object" });
    Ok(request)
}

/// Builds OpenAI-compatible chat messages from a system prompt and conversation.
fn openai_messages(
    system_message: &str,
    chat_messages: Vec<ChatMessage>,
) -> Vec<ChatCompletionRequestMessage> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessage::from(system_message).into()];

    messages.extend(
        chat_messages
            .into_iter()
            .map(ChatCompletionRequestMessage::from)
            .collect::<Vec<_>>(),
    );
    messages
}

/// Sends a Kimi chat completion request and returns the first assistant message content.
async fn response_kimi(
    api_base: String,
    api_key: String,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> Result<Option<String>> {
    let request = build_kimi_chat_completion_request(messages, model)?;
    send_kimi_chat_completion(api_base, api_key, request).await
}

/// Sends a Kimi request and parses the structured response.
async fn structured_output_kimi<
    T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
>(
    api_base: String,
    api_key: String,
    messages: Vec<ChatCompletionRequestMessage>,
    model: String,
) -> Result<Option<T>> {
    let request = build_kimi_structured_output_request::<T>(messages, model)?;
    let content = send_kimi_chat_completion(api_base, api_key, request).await?;
    if let Some(content) = content {
        return Ok(Some(serde_json::from_str::<T>(&content).with_context(
            || "Failed to parse structured content from Kimi API response",
        )?));
    }

    Ok(None)
}

/// Sends a Kimi request body and extracts the first assistant message content.
async fn send_kimi_chat_completion(
    api_base: String,
    api_key: String,
    request: serde_json::Value,
) -> Result<Option<String>> {
    let response = http_client_builder()
        .build()?
        .post(format!("{api_base}/chat/completions"))
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await
        .context("Failed to send Kimi API request")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|err| format!("failed to read error body: {err}"));
        let error_preview = error_text.chars().take(500).collect::<String>();
        anyhow::bail!("Kimi API request failed with HTTP {status}: {error_preview}");
    }

    let response = response
        .json::<KimiChatCompletionResponse>()
        .await
        .context("Failed to parse Kimi API response")?;
    for choice in response.choices {
        if let Some(content) = choice.message.content {
            return Ok(Some(content));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
    #[serde(rename_all = "camelCase")]
    struct TestStructuredOutput {
        commit_message: String,
    }

    /// Creates an OpenAI provider with direct credentials for API flavor tests.
    fn provider(model: Option<&str>, custom_endpoint: Option<&str>) -> OpenAiProvider {
        OpenAiProvider {
            custom_endpoint: custom_endpoint.map(str::to_string),
            model: model.map(str::to_string),
            credentials: (
                CredentialsKind::OwnOpenAiKey,
                Sensitive("test-key".to_string()),
            ),
        }
    }

    #[test]
    fn detects_kimi_by_endpoint() {
        let provider = provider(None, Some("https://api.moonshot.ai/v1"));

        assert_eq!(provider.api_flavor("gpt-4o"), OpenAiApiFlavor::Kimi);
    }

    #[test]
    fn detects_kimi_by_model() {
        let provider = provider(None, None);

        assert_eq!(provider.api_flavor("kimi-k2.6"), OpenAiApiFlavor::Kimi);
    }

    #[test]
    fn uses_standard_flavor_for_openai_endpoint() {
        let provider = provider(None, Some("https://api.openai.com/v1"));

        assert_eq!(provider.api_flavor("gpt-5.4"), OpenAiApiFlavor::Standard);
    }

    #[test]
    fn builds_kimi_request_without_thinking() {
        let messages = vec![
            ChatCompletionRequestSystemMessage::from("system").into(),
            ChatMessage::User("Generate a commit message.".to_string()).into(),
        ];

        let request = build_kimi_chat_completion_request(messages, "kimi-k2.6".to_string())
            .expect("request builds");

        assert_eq!(request["model"], "kimi-k2.6");
        assert_eq!(request["thinking"], json!({ "type": "disabled" }));
        assert_eq!(request["stream"], false);
        assert_eq!(request["max_tokens"], DEFAULT_KIMI_MAX_COMPLETION_TOKENS);
        assert!(request.get("max_completion_tokens").is_none());
        assert!(request.get("response_format").is_none());
    }

    #[test]
    fn builds_kimi_request_for_json_mode_with_schema_prompt() {
        let messages = vec![
            ChatCompletionRequestSystemMessage::from("system").into(),
            ChatMessage::User("Generate a commit message.".to_string()).into(),
        ];

        let request = build_kimi_structured_output_request::<TestStructuredOutput>(
            messages,
            "kimi-k2.6".to_string(),
        )
        .expect("request builds");

        assert_eq!(request["model"], "kimi-k2.6");
        assert_eq!(request["response_format"], json!({ "type": "json_object" }));
        assert_eq!(request["thinking"], json!({ "type": "disabled" }));
        assert_eq!(request["stream"], false);
        assert_eq!(request["max_tokens"], DEFAULT_KIMI_MAX_COMPLETION_TOKENS);
        assert!(request.get("max_completion_tokens").is_none());

        let messages = request["messages"]
            .as_array()
            .expect("messages are an array");
        let schema_message = messages[1]["content"]
            .as_str()
            .expect("schema message has content");
        assert!(schema_message.contains("commitMessage"));
    }
}
