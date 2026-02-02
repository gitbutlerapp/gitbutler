use anyhow::Result;
use async_openai::config::OpenAIConfig;
use but_tools::tool::Toolset;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    ChatMessage,
    client::LLMClient,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking, structured_output_blocking,
        tool_calling_loop, tool_calling_loop_stream,
    },
};

const OLLAMA_API_BASE_DEFAULT: &str = "http://localhost:11434/v1/";
const OLLAMA_ENDPOINT: &str = "gitbutler.aiOllamaEndpoint";
const OLLAMA_MODEL_NAME: &str = "gitbutler.aiOllamaModelName";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OllamaHostConfig {
    pub host: String,
    pub port: u16,
}

impl From<String> for OllamaHostConfig {
    fn from(endpoint: String) -> Self {
        let parts: Vec<&str> = endpoint.split(':').collect();
        let host = parts.first().cloned().unwrap_or("localhost").to_string();
        let port = parts.get(1).and_then(|p| p.parse::<u16>().ok()).unwrap_or(11434);
        OllamaHostConfig { host, port }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct OllamaConfig {
    pub host_config: Option<OllamaHostConfig>,
}

impl OllamaConfig {
    fn api_base(&self) -> String {
        if let Some(host_config) = &self.host_config {
            format!("http://{}:{}/v1/", host_config.host, host_config.port)
        } else {
            OLLAMA_API_BASE_DEFAULT.to_string()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OllamaProvider {
    pub config: OllamaConfig,
    model: Option<String>,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig, model: Option<String>) -> Self {
        Self { config, model }
    }
}

impl OpenAIClientProvider for OllamaProvider {
    fn client(&self) -> Result<async_openai::Client<async_openai::config::OpenAIConfig>> {
        let open_ai_config = OpenAIConfig::new()
            .with_api_base(self.config.api_base())
            .with_api_key("ollama");
        Ok(async_openai::Client::with_config(open_ai_config))
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
        let ollama_config = OllamaConfig { host_config: endpoint };
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
        let result = tool_calling_loop_stream(self, system_message, chat_messages, tool_set, model, on_token)?;
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

    fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static>(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<Option<T>> {
        structured_output_blocking(self, system_message, chat_messages, model)
    }

    fn response(&self, system_message: &str, chat_messages: Vec<ChatMessage>, model: &str) -> Result<Option<String>> {
        response_blocking(self, system_message, chat_messages, model)
    }
}
