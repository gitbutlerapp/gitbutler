use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use but_tools::tool::Toolset;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    AI_LMSTUDIO_ENDPOINT_KEY, AI_LMSTUDIO_MODEL_NAME_KEY,
    chat::ChatMessage,
    client::{GitConfigReader, LLMClient, http_client_builder},
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking,
        structured_output_blocking, tool_calling_loop, tool_calling_loop_stream,
    },
};

const LMSTUDIO_API_BASE_DEFAULT: &str = "http://localhost:1234/v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LMStudioConfig {
    pub api_base: String,
}

impl Default for LMStudioConfig {
    fn default() -> Self {
        Self {
            api_base: LMSTUDIO_API_BASE_DEFAULT.to_string(),
        }
    }
}

impl LMStudioConfig {
    fn from_git_config(config: &impl GitConfigReader) -> Self {
        let api_base = config
            .string_value(AI_LMSTUDIO_ENDPOINT_KEY)
            .unwrap_or_else(|| LMSTUDIO_API_BASE_DEFAULT.to_string());

        Self { api_base }
    }
}

#[derive(Debug, Clone)]
pub struct LMStudioProvider {
    model: Option<String>,
    config: LMStudioConfig,
}

impl LMStudioProvider {
    pub fn with(config: Option<LMStudioConfig>, model: Option<String>) -> Option<Self> {
        let config = config.unwrap_or_default();
        Some(Self { config, model })
    }

    pub fn config(&self) -> &LMStudioConfig {
        &self.config
    }
}

impl OpenAIClientProvider for LMStudioProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>> {
        let open_ai_config = OpenAIConfig::new()
            .with_api_base(self.config.api_base.clone())
            .with_api_key("lm-studio");

        Ok(Client::with_config(open_ai_config).with_http_client(http_client_builder().build()?))
    }
}

impl LLMClient for LMStudioProvider {
    fn from_git_config(config: &impl GitConfigReader) -> Option<Self>
    where
        Self: Sized,
    {
        let lmstudio_config = LMStudioConfig::from_git_config(config);
        let model = config.string_value(AI_LMSTUDIO_MODEL_NAME_KEY);
        Some(Self {
            config: lmstudio_config,
            model,
        })
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
