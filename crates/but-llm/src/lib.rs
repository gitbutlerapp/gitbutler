mod anthropic;
mod chat;
mod client;
mod config;
mod key;
mod lmstudio;
mod ollama;
mod openai;
mod openai_utils;
mod openrouter;

use std::sync::Arc;

pub use chat::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent};
pub use config::*;
pub use key::CredentialsKeyOption;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::client::LLMClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    LMStudio,
    OpenRouter,
}

impl LLMProviderKind {
    pub fn from_git_config_value(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(LLMProviderKind::OpenAi),
            "anthropic" => Some(LLMProviderKind::Anthropic),
            "ollama" => Some(LLMProviderKind::Ollama),
            "lmstudio" => Some(LLMProviderKind::LMStudio),
            "openrouter" => Some(LLMProviderKind::OpenRouter),
            _ => None,
        }
    }

    pub fn as_git_config_value(self) -> &'static str {
        match self {
            LLMProviderKind::OpenAi => "openai",
            LLMProviderKind::Anthropic => "anthropic",
            LLMProviderKind::Ollama => "ollama",
            LLMProviderKind::LMStudio => "lmstudio",
            LLMProviderKind::OpenRouter => "openrouter",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            LLMProviderKind::OpenAi => "OpenAI",
            LLMProviderKind::Anthropic => "Anthropic",
            LLMProviderKind::Ollama => "Ollama",
            LLMProviderKind::LMStudio => "LM Studio",
            LLMProviderKind::OpenRouter => "OpenRouter",
        }
    }
}

impl From<LLMProviderKind> for String {
    fn from(value: LLMProviderKind) -> Self {
        value.display_name().to_string()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LLMProviderConfig {
    OpenAi(Option<openai::CredentialsKind>),
    Anthropic(Option<anthropic::CredentialsKind>),
    Ollama(Option<ollama::OllamaConfig>),
    LMStudio(Option<lmstudio::LMStudioConfig>),
    OpenRouter(Option<openrouter::OpenRouterConfig>),
}

#[derive(Debug, Clone)]
pub enum LLMClientType {
    OpenAi(Arc<openai::OpenAiProvider>),
    Anthropic(Arc<anthropic::AnthropicProvider>),
    Ollama(Arc<ollama::OllamaProvider>),
    LMStudio(Arc<lmstudio::LMStudioProvider>),
    OpenRouter(Arc<openrouter::OpenRouterProvider>),
}

#[derive(Debug, Clone)]
pub struct LLMProvider {
    client: LLMClientType,
}

impl LLMProvider {
    /// Returns `None` if the provider can't be initialized, e.g. for missing credentials.
    pub fn new(kind: LLMProviderConfig) -> Option<Self> {
        let client = match kind {
            LLMProviderConfig::OpenAi(creds) => openai::OpenAiProvider::with(creds, None, None)
                .map(|p| LLMClientType::OpenAi(Arc::new(p)))?,
            LLMProviderConfig::Anthropic(creds) => anthropic::AnthropicProvider::with(creds, None)
                .map(|p| LLMClientType::Anthropic(Arc::new(p)))?,
            LLMProviderConfig::Ollama(config) => {
                let config = config.unwrap_or_default();
                LLMClientType::Ollama(Arc::new(ollama::OllamaProvider::new(config, None)))
            }
            LLMProviderConfig::LMStudio(config) => lmstudio::LMStudioProvider::with(config, None)
                .map(|p| LLMClientType::LMStudio(Arc::new(p)))?,
            LLMProviderConfig::OpenRouter(config) => {
                openrouter::OpenRouterProvider::with(config, None)
                    .map(|p| LLMClientType::OpenRouter(Arc::new(p)))?
            }
        };
        Some(Self { client })
    }

    /// Returns `None` if no provider is configured, its settings don't validate, or it can't be
    /// initialized, e.g. for missing credentials.
    pub fn from_git_config(config: &gix::config::File) -> Option<Self> {
        let configuration = AiConfiguration::from_git_config(config).ok()?;
        configuration.validate_active().ok()?;
        match configuration.provider {
            LLMProviderKind::OpenAi => {
                let credentials = match configuration.openai.key_option {
                    CredentialsKeyOption::BringYourOwn => openai::CredentialsKind::OwnOpenAiKey,
                    CredentialsKeyOption::ButlerApi => openai::CredentialsKind::GitButlerProxied,
                };
                let client = openai::OpenAiProvider::with(
                    Some(credentials),
                    Some(configuration.openai.model),
                    configuration.openai.custom_endpoint,
                )?;
                Some(Self {
                    client: LLMClientType::OpenAi(Arc::new(client)),
                })
            }
            LLMProviderKind::Anthropic => {
                let credentials = match configuration.anthropic.key_option {
                    CredentialsKeyOption::BringYourOwn => {
                        anthropic::CredentialsKind::OwnAnthropicKey
                    }
                    CredentialsKeyOption::ButlerApi => anthropic::CredentialsKind::GitButlerProxied,
                };
                let client = anthropic::AnthropicProvider::with(
                    Some(credentials),
                    Some(configuration.anthropic.model),
                )?;
                Some(Self {
                    client: LLMClientType::Anthropic(Arc::new(client)),
                })
            }
            LLMProviderKind::Ollama => {
                let host_config = ollama::OllamaHostConfig::from(configuration.ollama.endpoint);
                let client = ollama::OllamaProvider::new(
                    ollama::OllamaConfig {
                        host_config: Some(host_config),
                    },
                    Some(configuration.ollama.model),
                );
                Some(Self {
                    client: LLMClientType::Ollama(Arc::new(client)),
                })
            }
            LLMProviderKind::LMStudio => {
                let client = lmstudio::LMStudioProvider::with(
                    Some(lmstudio::LMStudioConfig {
                        api_base: configuration.lmstudio.endpoint,
                    }),
                    Some(configuration.lmstudio.model),
                )?;
                Some(Self {
                    client: LLMClientType::LMStudio(Arc::new(client)),
                })
            }
            LLMProviderKind::OpenRouter => {
                let provider_config = config.string(AI_OPENROUTER_ENDPOINT_KEY).map(|api_base| {
                    openrouter::OpenRouterConfig {
                        api_base: api_base.to_string(),
                    }
                });
                let model = config
                    .string(AI_OPENROUTER_MODEL_NAME_KEY)
                    .map(|model| model.to_string());
                let client = openrouter::OpenRouterProvider::with(provider_config, model)?;
                Some(Self {
                    client: LLMClientType::OpenRouter(Arc::new(client)),
                })
            }
        }
    }

    pub fn model(&self) -> Option<String> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.model(),
            LLMClientType::Anthropic(client) => client.model(),
            LLMClientType::Ollama(client) => client.model(),
            LLMClientType::LMStudio(client) => client.model(),
            LLMClientType::OpenRouter(client) => client.model(),
        }
    }

    /// Returns the configured model, or a per-provider default when none is set.
    ///
    /// Callers that need a concrete model name (e.g. structured output) must not
    /// hardcode a provider-specific model, as it would 404 against other providers.
    pub fn model_or_default(&self) -> String {
        if let Some(model) = self.model() {
            return model;
        }
        match &self.client {
            LLMClientType::OpenAi(_) => DEFAULT_OPENAI_MODEL,
            LLMClientType::Anthropic(_) => DEFAULT_ANTHROPIC_MODEL,
            LLMClientType::Ollama(_) => DEFAULT_OLLAMA_MODEL,
            LLMClientType::LMStudio(_) => DEFAULT_LMSTUDIO_MODEL,
            LLMClientType::OpenRouter(_) => DEFAULT_OPENROUTER_MODEL,
        }
        .to_string()
    }

    /// Uses credentials from the environment, typically GB credentials or `OPENAI_API_KEY`.
    pub fn default_openai() -> Option<Self> {
        Self::new(LLMProviderConfig::OpenAi(None))
    }

    /// Uses credentials from the environment, typically GB credentials or `ANTHROPIC_API_KEY`.
    pub fn default_anthropic() -> Option<Self> {
        Self::new(LLMProviderConfig::Anthropic(None))
    }

    /// Reads `LM_STUDIO_API_BASE` and `LM_API_TOKEN` from the environment.
    pub fn default_lmstudio() -> Option<Self> {
        Self::new(LLMProviderConfig::LMStudio(None))
    }

    /// Lets the model call tools from `tool_set` until it answers without tool calls, streaming
    /// all text it produces through `on_token`.
    ///
    /// Returns the final response along with the full conversation, including all tool calls and
    /// their results.
    pub fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> anyhow::Result<(String, Vec<ChatMessage>)> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.tool_calling_loop_stream(
                system_message,
                chat_messages,
                tool_set,
                model,
                on_token,
            ),
            LLMClientType::Anthropic(client) => client.tool_calling_loop_stream(
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
            LLMClientType::LMStudio(client) => client.tool_calling_loop_stream(
                system_message,
                chat_messages,
                tool_set,
                model,
                on_token,
            ),
            LLMClientType::OpenRouter(client) => client.tool_calling_loop_stream(
                system_message,
                chat_messages,
                tool_set,
                model,
                on_token,
            ),
        }
    }

    /// Like [`Self::tool_calling_loop_stream()`], but returns only the final response, once all
    /// tool calls have been resolved.
    pub fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: &str,
    ) -> anyhow::Result<String> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::Anthropic(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::Ollama(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::LMStudio(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::OpenRouter(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
        }
    }

    /// Streams the response through `on_token`, without tool calling.
    ///
    /// Returns `None` if the model produced no output.
    pub fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
            LLMClientType::Anthropic(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
            LLMClientType::Ollama(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
            LLMClientType::LMStudio(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
            LLMClientType::OpenRouter(client) => {
                client.stream_response(system_message, chat_messages, model, on_token)
            }
        }
    }

    /// The response is validated against the JSON schema derived from `T` and deserialized into
    /// it.
    ///
    /// Returns `None` if the model produced no output.
    pub fn structured_output<
        T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static,
    >(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> anyhow::Result<Option<T>> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
            LLMClientType::Anthropic(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
            LLMClientType::Ollama(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
            LLMClientType::LMStudio(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
            LLMClientType::OpenRouter(client) => {
                client.structured_output::<T>(system_message, chat_messages, model)
            }
        }
    }

    /// The complete response, without streaming or tool calling, or `None` if the model produced
    /// no output.
    pub fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.response(system_message, chat_messages, model),
            LLMClientType::Anthropic(client) => {
                client.response(system_message, chat_messages, model)
            }
            LLMClientType::Ollama(client) => client.response(system_message, chat_messages, model),
            LLMClientType::LMStudio(client) => {
                client.response(system_message, chat_messages, model)
            }
            LLMClientType::OpenRouter(client) => {
                client.response(system_message, chat_messages, model)
            }
        }
    }
}
