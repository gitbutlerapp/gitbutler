use anyhow::Result;
use async_openai::config::OpenAIConfig;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    ChatMessage,
    client::LLMClient,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking,
        structured_output_blocking,
    },
};

#[derive(Debug, Clone)]
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

impl OllamaHostConfig {
    fn api_base(&self) -> String {
        format!("http://{}:{}/v1/", self.host, self.port)
    }
}

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    host_config: OllamaHostConfig,
    model: Option<String>,
}

impl OllamaProvider {
    pub fn new(host_config: OllamaHostConfig, model: Option<String>) -> Self {
        Self { host_config, model }
    }
}

impl OpenAIClientProvider for OllamaProvider {
    fn client(&self) -> Result<async_openai::Client<async_openai::config::OpenAIConfig>> {
        let open_ai_config = OpenAIConfig::new()
            .with_api_base(self.host_config.api_base())
            .with_api_key("ollama");
        Ok(async_openai::Client::build(
            but_http::client_builder()?.build()?,
            open_ai_config,
        ))
    }
}

impl LLMClient for OllamaProvider {
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
