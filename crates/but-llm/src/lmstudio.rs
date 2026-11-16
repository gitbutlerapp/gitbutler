use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    chat::ChatMessage,
    client::LLMClient,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking,
        structured_output_blocking,
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

#[derive(Debug, Clone)]
pub struct LMStudioProvider {
    model: Option<String>,
    config: LMStudioConfig,
}

impl LMStudioProvider {
    pub fn with(config: Option<LMStudioConfig>, model: Option<String>) -> Option<Self> {
        let mut config = config.unwrap_or_default();
        config.api_base = normalize_api_base(&config.api_base);
        Some(Self { config, model })
    }

    pub fn config(&self) -> &LMStudioConfig {
        &self.config
    }
}

fn normalize_api_base(endpoint: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    if endpoint.ends_with("/v1") {
        endpoint.into()
    } else {
        format!("{endpoint}/v1")
    }
}

impl OpenAIClientProvider for LMStudioProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>> {
        let open_ai_config = OpenAIConfig::new()
            .with_api_base(self.config.api_base.clone())
            .with_api_key("lm-studio");

        Ok(Client::with_config(open_ai_config))
    }
}

impl LLMClient for LMStudioProvider {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_api_base() {
        assert_eq!(
            normalize_api_base("http://127.0.0.1:1234"),
            "http://127.0.0.1:1234/v1"
        );
        assert_eq!(
            normalize_api_base("http://127.0.0.1:1234/v1/"),
            "http://127.0.0.1:1234/v1"
        );
    }
}
