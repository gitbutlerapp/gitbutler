mod chat;
mod client;
mod ollama;
mod openai;

use std::sync::Arc;

pub use chat::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent};

pub use openai::{
    CredentialsKind, OpenAiProvider, stream_response_blocking, structured_output_blocking,
    tool_calling_loop, tool_calling_loop_stream,
};

use crate::client::LLMClient;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LLMProviderConfig {
    OpenAi(Option<CredentialsKind>),
    Ollama(Option<ollama::OllamaConfig>),
}

#[derive(Debug, Clone)]
pub enum LLMClientType {
    OpenAi(Arc<OpenAiProvider>),
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
            LLMProviderConfig::OpenAi(creds) => {
                OpenAiProvider::with(creds.clone()).map(|p| LLMClientType::OpenAi(Arc::new(p)))?
            }
            LLMProviderConfig::Ollama(config) => {
                let config = config
                    .clone()
                    .unwrap_or(ollama::OllamaConfig { host_config: None });
                LLMClientType::Ollama(Arc::new(ollama::OllamaProvider::new(config)))
            }
        };
        Some(Self { kind, client })
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
}
