mod chat;
mod client;
mod ollama;
mod openai;

use std::sync::Arc;

pub use chat::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent};

pub use openai::CredentialsKind;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::client::LLMClient;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LLMProviderConfig {
    OpenAi(Option<CredentialsKind>),
    Ollama(Option<ollama::OllamaConfig>),
}

#[derive(Debug, Clone)]
pub enum LLMClientType {
    OpenAi(Arc<openai::OpenAiProvider>),
    Ollama(Arc<ollama::OllamaProvider>),
}

#[derive(Debug, Clone)]
pub struct LLMProvider {
    pub kind: LLMProviderConfig,
    client: LLMClientType,
}

/// The top-level LLM provider that wraps specific implementations.
///
/// This struct provides a unified interface to interact with different LLM providers
impl LLMProvider {
    pub fn new(kind: LLMProviderConfig) -> Option<Self> {
        let client = match &kind {
            LLMProviderConfig::OpenAi(creds) => openai::OpenAiProvider::with(creds.clone())
                .map(|p| LLMClientType::OpenAi(Arc::new(p)))?,
            LLMProviderConfig::Ollama(config) => {
                let config = config
                    .clone()
                    .unwrap_or(ollama::OllamaConfig { host_config: None });
                LLMClientType::Ollama(Arc::new(ollama::OllamaProvider::new(config)))
            }
        };
        Some(Self { kind, client })
    }
    pub fn default_openai() -> Option<Self> {
        Self::new(LLMProviderConfig::OpenAi(None))
    }

    /// Streams a tool-calling loop using the selected LLM provider.
    pub fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> anyhow::Result<(String, Vec<ChatMessage>)> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.tool_calling_loop_stream(
                system_message,
                chat_messages,
                tool_set,
                model,
                on_token,
            ),
            LLMClientType::Ollama(client) => client.tool_calling_loop_stream(
                system_message,
                chat_messages,
                tool_set,
                model,
                on_token,
            ),
        }
    }

    /// Executes a tool-calling loop using the selected LLM provider.
    pub fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: String,
    ) -> anyhow::Result<String> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::Ollama(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
        }
    }

    /// Streams a response using the selected LLM provider.
    pub fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
        on_token: Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
            LLMClientType::Ollama(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
        }
    }

    /// Gets a response using the selected LLM provider in the provided format.
    pub fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
    ) -> anyhow::Result<Option<T>> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
            LLMClientType::Ollama(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
        }
    }

    /// Gets a response using the selected LLM provider.
    pub fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: String,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.response(system_message, chat_messages, model),
            LLMClientType::Ollama(client) => client.response(system_message, chat_messages, model),
        }
    }
}
