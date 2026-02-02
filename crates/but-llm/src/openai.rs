use anyhow::{Context as _, Result};
use async_openai::{Client, config::OpenAIConfig};
use but_secret::{Sensitive, secret};
use but_tools::tool::Toolset;
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    chat::ChatMessage,
    client::LLMClient,
    key::CredentialsKeyOption,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking, structured_output_blocking,
        tool_calling_loop, tool_calling_loop_stream,
    },
};

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

const OPEN_AI_KEY_OPTION: &str = "gitbutler.aiOpenAIKeyOption";
const OPEN_AI_MODEL_NAME: &str = "gitbutler.aiOpenAIModelName";
const OPEN_AI_CUSTOM_ENDPOINT: &str = "gitbutler.aiOpenAICustomEndpoint";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarOpenAiKey,
    OwnOpenAiKey,
    GitButlerProxied,
}

impl CredentialsKind {
    fn from_git_config(config: &gix::config::File<'static>) -> Option<Self> {
        let key_option_str = config.string(OPEN_AI_KEY_OPTION).map(|v| v.to_string())?;
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

    pub fn credentials_kind(&self) -> CredentialsKind {
        self.credentials.0.clone()
    }

    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?.ok_or(
            anyhow::anyhow!("No GitButler token available. Log-in to use the GitButler OpenAI provider"),
        )?;
        Ok((CredentialsKind::GitButlerProxied, creds))
    }

    fn openai_own_key_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("aiOpenAIKey", secret::Namespace::Global)?.ok_or(anyhow::anyhow!(
            "No OpenAI own key configured. Add this through the GitButler settings"
        ))?;
        Ok((CredentialsKind::OwnOpenAiKey, creds))
    }

    fn openai_env_var_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = Sensitive(
            std::env::var_os("OPENAI_API_KEY")
                .ok_or(anyhow::anyhow!("Environment variable OPENAI_API_KEY is not set"))?
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
                let config = self.configure_custom_endpoint(OpenAIConfig::new().with_api_key(key.0.clone()));
                Ok(Client::with_config(config))
            }

            (CredentialsKind::GitButlerProxied, key) => {
                let config = OpenAIConfig::new().with_api_base(GB_OPENAI_API_BASE);
                let mut headers = HeaderMap::new();
                headers.insert(
                    reqwest::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
                headers.insert("X-Auth-Token", key.0.parse().unwrap_or(HeaderValue::from_static("")));
                let http_client = reqwest::Client::builder().default_headers(headers).build()?;
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
        let model = config.string(OPEN_AI_MODEL_NAME).map(|v| v.to_string());
        let custom_endpoint = config.string(OPEN_AI_CUSTOM_ENDPOINT).map(|v| v.to_string());

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
        structured_output_blocking::<T>(self, system_message, chat_messages, model)
    }

    fn response(&self, system_message: &str, chat_messages: Vec<ChatMessage>, model: &str) -> Result<Option<String>> {
        response_blocking(self, system_message, chat_messages, model)
    }
}
