mod anthropic;
mod chat;
mod client;
mod key;
mod lmstudio;
mod ollama;
mod openai;
mod openai_utils;

use std::sync::Arc;

pub use chat::{ChatMessage, StreamToolCallResult, ToolCall, ToolCallContent, ToolResponseContent};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::client::LLMClient;

const MODEL_PROVIDER: &str = "gitbutler.aiModelProvider";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
    LMStudio,
}

impl LLMProviderKind {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(LLMProviderKind::OpenAi),
            "anthropic" => Some(LLMProviderKind::Anthropic),
            "ollama" => Some(LLMProviderKind::Ollama),
            "lmstudio" => Some(LLMProviderKind::LMStudio),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LLMProviderConfig {
    OpenAi(Option<openai::CredentialsKind>),
    Anthropic(Option<anthropic::CredentialsKind>),
    Ollama(Option<ollama::OllamaConfig>),
    LMStudio(Option<lmstudio::LMStudioConfig>),
}

#[derive(Debug, Clone)]
pub enum LLMClientType {
    OpenAi(Arc<openai::OpenAiProvider>),
    Anthropic(Arc<anthropic::AnthropicProvider>),
    Ollama(Arc<ollama::OllamaProvider>),
    LMStudio(Arc<lmstudio::LMStudioProvider>),
}

#[derive(Debug, Clone)]
pub struct LLMProvider {
    client: LLMClientType,
}

/// The top-level LLM provider that wraps specific implementations.
///
/// This struct provides a unified interface to interact with different LLM providers
/// (currently OpenAI and Ollama). It abstracts away the differences between providers,
/// allowing callers to work with a consistent API regardless of which underlying
/// LLM service is being used. The provider handles authentication, request formatting,
/// response parsing, and tool calling orchestration for all supported backends.
impl LLMProvider {
    /// Creates a new LLM provider based on the specified configuration.
    ///
    /// This method initializes the appropriate underlying client (OpenAI or Ollama)
    /// based on the provided configuration. For OpenAI, it will attempt to use
    /// the provided credentials or fall back to environment variables. For Ollama,
    /// it will use the provided host configuration or default to localhost.
    ///
    /// # Arguments
    ///
    /// * `kind` - The provider configuration specifying which LLM backend to use
    ///   and any associated credentials or settings.
    ///
    /// # Returns
    ///
    /// Returns `Some(LLMProvider)` if the provider was successfully initialized,
    /// or `None` if initialization failed (e.g., missing required credentials).
    pub fn new(kind: LLMProviderConfig) -> Option<Self> {
        let client = match kind {
            LLMProviderConfig::OpenAi(creds) => {
                openai::OpenAiProvider::with(creds, None, None).map(|p| LLMClientType::OpenAi(Arc::new(p)))?
            }
            LLMProviderConfig::Anthropic(creds) => {
                anthropic::AnthropicProvider::with(creds, None).map(|p| LLMClientType::Anthropic(Arc::new(p)))?
            }
            LLMProviderConfig::Ollama(config) => {
                let config = config.unwrap_or_default();
                LLMClientType::Ollama(Arc::new(ollama::OllamaProvider::new(config, None)))
            }
            LLMProviderConfig::LMStudio(config) => {
                lmstudio::LMStudioProvider::with(config, None).map(|p| LLMClientType::LMStudio(Arc::new(p)))?
            }
        };
        Some(Self { client })
    }

    /// Creates a new LLM provider based on configuration stored in the global Git config.
    ///
    /// This method reads the LLM provider settings from Git's configuration system,
    /// specifically looking for the `gitbutler.aiModelProvider` setting to determine
    /// which provider to instantiate. It then delegates to the appropriate provider's
    /// `from_git_config` method to read provider-specific settings like API keys,
    /// model names, and endpoints.
    ///
    /// This is the recommended way to initialize an LLM provider in a Git repository
    /// context, as it respects user-configured settings and credentials stored in
    /// their Git configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to a Git configuration object, typically the global
    ///   or repository-level config, containing the LLM provider settings.
    ///
    /// # Returns
    ///
    /// Returns `Some(LLMProvider)` if a valid provider is configured and successfully
    /// initialized, or `None` if:
    /// - The `gitbutler.aiModelProvider` setting is not present
    /// - The configured provider is not supported
    /// - Provider-specific initialization fails (e.g., missing credentials)
    pub fn from_git_config(config: &gix::config::File<'static>) -> Option<Self> {
        let provider_str = config.string(MODEL_PROVIDER).map(|v| v.to_string())?;
        let provider = LLMProviderKind::from_str(&provider_str);
        match provider {
            Some(LLMProviderKind::OpenAi) => {
                let client = openai::OpenAiProvider::from_git_config(config)?;
                Some(Self {
                    client: LLMClientType::OpenAi(Arc::new(client)),
                })
            }
            Some(LLMProviderKind::Anthropic) => {
                let client = anthropic::AnthropicProvider::from_git_config(config)?;
                Some(Self {
                    client: LLMClientType::Anthropic(Arc::new(client)),
                })
            }
            Some(LLMProviderKind::Ollama) => {
                let client = ollama::OllamaProvider::from_git_config(config)?;
                Some(Self {
                    client: LLMClientType::Ollama(Arc::new(client)),
                })
            }
            Some(LLMProviderKind::LMStudio) => {
                let client = lmstudio::LMStudioProvider::from_git_config(config)?;
                Some(Self {
                    client: LLMClientType::LMStudio(Arc::new(client)),
                })
            }
            None => None,
        }
    }

    /// Returns the model identifier configured for this LLM provider.
    ///
    /// This method retrieves the model name that was configured for the provider
    /// at the time of instantiation, typically read from the Git global configuration.
    /// The model identifier (e.g., "gpt-4", "claude-3-opus", "llama2") determines
    /// which specific model variant will be used for LLM requests.
    ///
    /// The model value is read once during provider initialization and remains
    /// constant for the lifetime of the provider instance.
    ///
    /// # Returns
    ///
    /// Returns `Some(String)` containing the model identifier if one was configured,
    /// or `None` if no model was specified in the configuration.
    pub fn model(&self) -> Option<String> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.model(),
            LLMClientType::Anthropic(client) => client.model(),
            LLMClientType::Ollama(client) => client.model(),
            LLMClientType::LMStudio(client) => client.model(),
        }
    }

    /// Creates a default OpenAI LLM provider using environment-based credentials.
    ///
    /// This is a convenience method that attempts to create an OpenAI provider
    /// by reading credentials from environment variables (typically GB credentials or `OPENAI_API_KEY`).
    /// It's the recommended way to create an OpenAI provider when you want to use
    /// the default configuration without explicitly passing credentials.
    ///
    /// # Returns
    ///
    /// Returns `Some(LLMProvider)` if OpenAI credentials are found in the environment
    /// and the provider was successfully initialized, or `None` if credentials are
    /// missing or initialization failed.
    pub fn default_openai() -> Option<Self> {
        Self::new(LLMProviderConfig::OpenAi(None))
    }

    /// Creates a default Anthropic LLM provider using environment-based credentials.
    ///
    /// This is a convenience method that attempts to create an Anthropic provider
    /// by reading credentials from environment variables (typically GB credentials or `ANTHROPIC_API_KEY`).
    /// It's the recommended way to create an Anthropic provider when you want to use
    /// the default configuration without explicitly passing credentials.
    ///
    /// # Returns
    ///
    /// Returns `Some(LLMProvider)` if Anthropic credentials are found in the environment
    /// and the provider was successfully initialized, or `None` if credentials are
    /// missing or initialization failed.
    pub fn default_anthropic() -> Option<Self> {
        Self::new(LLMProviderConfig::Anthropic(None))
    }

    /// Creates a default LM Studio LLM provider using environment-based configuration.
    ///
    /// This is a convenience method that attempts to create an LM Studio provider
    /// by reading configuration from environment variables (LM_STUDIO_API_BASE and LM_API_TOKEN).
    /// It's the recommended way to create an LM Studio provider when you want to use
    /// the default configuration without explicitly passing settings.
    ///
    /// # Returns
    ///
    /// Returns `Some(LLMProvider)` if the provider was successfully initialized,
    /// or `None` if initialization failed.
    pub fn default_lmstudio() -> Option<Self> {
        Self::new(LLMProviderConfig::LMStudio(None))
    }

    /// Executes an interactive tool-calling loop with streaming output.
    ///
    /// This method orchestrates a conversation with the LLM where the model can call
    /// tools (functions) to gather information or perform actions. The LLM's text output
    /// is streamed token-by-token via the provided callback, enabling real-time display
    /// of the assistant's responses. The loop continues until the LLM produces a final
    /// response without tool calls.
    ///
    /// The tool-calling loop works as follows:
    /// 1. Send messages to the LLM with available tools
    /// 2. If the LLM requests tool calls, execute them and add results to the conversation
    /// 3. Continue until the LLM provides a final text response
    /// 4. Stream all text output through the `on_token` callback as it's generated
    ///
    /// # Arguments
    ///
    /// * `system_message` - The system prompt that defines the assistant's behavior and context
    /// * `chat_messages` - The conversation history including user and assistant messages
    /// * `tool_set` - A mutable reference to the toolset containing available functions the LLM can call
    /// * `model` - The model identifier (e.g., "gpt-4", "llama2") to use for generation
    /// * `on_token` - A callback function that receives each token as it's generated, enabling streaming display
    ///
    /// # Returns
    ///
    /// Returns `Ok((final_response, updated_messages))` where:
    /// - `final_response` is the complete text response from the LLM
    /// - `updated_messages` is the full conversation history including all tool calls and responses
    ///
    /// Returns `Err` if the LLM request fails or tool execution encounters an error.
    pub fn tool_calling_loop_stream(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> anyhow::Result<(String, Vec<ChatMessage>)> {
        match &self.client {
            LLMClientType::OpenAi(client) => {
                client.tool_calling_loop_stream(system_message, chat_messages, tool_set, model, on_token)
            }
            LLMClientType::Anthropic(client) => {
                client.tool_calling_loop_stream(system_message, chat_messages, tool_set, model, on_token)
            }
            LLMClientType::Ollama(client) => {
                client.tool_calling_loop_stream(system_message, chat_messages, tool_set, model, on_token)
            }
            LLMClientType::LMStudio(client) => {
                client.tool_calling_loop_stream(system_message, chat_messages, tool_set, model, on_token)
            }
        }
    }

    /// Executes an interactive tool-calling loop without streaming.
    ///
    /// Similar to `tool_calling_loop_stream`, but returns the complete final response
    /// only after the entire conversation is finished, without streaming intermediate tokens.
    /// This is useful when you don't need real-time output and prefer to wait for the
    /// complete response before processing it.
    ///
    /// The tool-calling loop works as follows:
    /// 1. Send messages to the LLM with available tools
    /// 2. If the LLM requests tool calls, execute them and add results to the conversation
    /// 3. Continue until the LLM provides a final text response
    /// 4. Return only the final complete response
    ///
    /// # Arguments
    ///
    /// * `system_message` - The system prompt that defines the assistant's behavior and context
    /// * `chat_messages` - The conversation history including user and assistant messages
    /// * `tool_set` - A mutable reference to the toolset containing available functions the LLM can call
    /// * `model` - The model identifier (e.g., "gpt-4", "llama2") to use for generation
    ///
    /// # Returns
    ///
    /// Returns `Ok(final_response)` containing the complete text response from the LLM
    /// after all tool calls have been resolved, or `Err` if the request or tool execution fails.
    pub fn tool_calling_loop(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        tool_set: &mut impl but_tools::tool::Toolset,
        model: &str,
    ) -> anyhow::Result<String> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.tool_calling_loop(system_message, chat_messages, tool_set, model),
            LLMClientType::Anthropic(client) => {
                client.tool_calling_loop(system_message, chat_messages, tool_set, model)
            }
            LLMClientType::Ollama(client) => client.tool_calling_loop(system_message, chat_messages, tool_set, model),
            LLMClientType::LMStudio(client) => client.tool_calling_loop(system_message, chat_messages, tool_set, model),
        }
    }

    /// Generates a streaming text response without tool calling capability.
    ///
    /// This method sends a conversation to the LLM and streams the response token-by-token
    /// through the provided callback. Unlike the tool-calling methods, this is a simple
    /// request-response interaction without the ability for the model to call functions.
    /// Use this when you want a straightforward chat completion with streaming output.
    ///
    /// # Arguments
    ///
    /// * `system_message` - The system prompt that defines the assistant's behavior and context
    /// * `chat_messages` - The conversation history including user and assistant messages
    /// * `model` - The model identifier (e.g., "gpt-4", "llama2") to use for generation
    /// * `on_token` - A callback function that receives each token as it's generated, enabling streaming display
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(response))` containing the complete text response if successful,
    /// `Ok(None)` if the model produced no output, or `Err` if the request failed.
    pub fn stream_response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
        on_token: impl Fn(&str) + Send + Sync + 'static,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.stream_response(system_message, chat_messages, model, on_token),
            LLMClientType::Anthropic(client) => client.stream_response(system_message, chat_messages, model, on_token),
            LLMClientType::Ollama(client) => client.stream_response(system_message, chat_messages, model, on_token),
            LLMClientType::LMStudio(client) => client.stream_response(system_message, chat_messages, model, on_token),
        }
    }

    /// Generates a structured response conforming to a specific schema.
    ///
    /// This method requests the LLM to produce output in a specific structured format
    /// defined by a Rust type. The response is automatically validated against the
    /// JSON schema derived from the type and deserialized into the requested structure.
    /// This is useful for extracting structured data (like configuration, JSON objects,
    /// or specific data models) from LLM responses with type safety.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type that defines the expected structure. Must implement
    ///   `Serialize`, `Deserialize`, and `JsonSchema` to enable schema generation
    ///   and validation.
    ///
    /// # Arguments
    ///
    /// * `system_message` - The system prompt that defines the assistant's behavior and context
    /// * `chat_messages` - The conversation history including user and assistant messages
    /// * `model` - The model identifier (e.g., "gpt-4", "llama2") to use for generation
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(value))` containing the parsed structured response if successful,
    /// `Ok(None)` if the model produced no output, or `Err` if the request failed or
    /// the response didn't conform to the expected schema.
    pub fn structured_output<T: serde::Serialize + DeserializeOwned + JsonSchema + std::marker::Send + 'static>(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> anyhow::Result<Option<T>> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.structured_output::<T>(system_message, chat_messages, model),
            LLMClientType::Anthropic(client) => client.structured_output::<T>(system_message, chat_messages, model),
            LLMClientType::Ollama(client) => client.structured_output::<T>(system_message, chat_messages, model),
            LLMClientType::LMStudio(client) => client.structured_output::<T>(system_message, chat_messages, model),
        }
    }

    /// Generates a simple text response without streaming or tool calling.
    ///
    /// This is the most basic interaction method that sends a conversation to the LLM
    /// and waits for the complete response. The entire response is returned at once
    /// without streaming. Use this for simple request-response scenarios where you
    /// don't need real-time output or function calling capabilities.
    ///
    /// # Arguments
    ///
    /// * `system_message` - The system prompt that defines the assistant's behavior and context
    /// * `chat_messages` - The conversation history including user and assistant messages
    /// * `model` - The model identifier (e.g., "gpt-4", "llama2") to use for generation
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(response))` containing the complete text response if successful,
    /// `Ok(None)` if the model produced no output, or `Err` if the request failed.
    pub fn response(
        &self,
        system_message: &str,
        chat_messages: Vec<ChatMessage>,
        model: &str,
    ) -> anyhow::Result<Option<String>> {
        match &self.client {
            LLMClientType::OpenAi(client) => client.response(system_message, chat_messages, model),
            LLMClientType::Anthropic(client) => client.response(system_message, chat_messages, model),
            LLMClientType::Ollama(client) => client.response(system_message, chat_messages, model),
            LLMClientType::LMStudio(client) => client.response(system_message, chat_messages, model),
        }
    }
}
