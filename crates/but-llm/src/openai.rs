use anyhow::Result;
use async_openai::{Client, config::OpenAIConfig};
use but_secret::{Sensitive, secret};
use reqwest::header::{HeaderMap, HeaderValue};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    AI_OPENAI_SECRET_HANDLE, GITBUTLER_ACCESS_TOKEN_HANDLE,
    chat::ChatMessage,
    client::LLMClient,
    openai_utils::{
        OpenAIClientProvider, response_blocking, stream_response_blocking,
        structured_output_blocking,
    },
};

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

#[derive(Debug, Clone)]
pub enum CredentialsKind {
    OwnOpenAiKey,
    GitButlerProxied,
}

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    /// As configured in the git global config.
    custom_endpoint: Option<String>,
    /// As configured in the git global config.
    model: Option<String>,
    credentials: (CredentialsKind, Sensitive<String>),
}

impl OpenAiProvider {
    pub fn with(
        kind: CredentialsKind,
        model: Option<String>,
        custom_endpoint: Option<String>,
    ) -> Option<Self> {
        let credentials = match kind {
            CredentialsKind::OwnOpenAiKey => OpenAiProvider::openai_own_key_creds(),
            CredentialsKind::GitButlerProxied => OpenAiProvider::gitbutler_proxied_creds(),
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

    fn configure_custom_endpoint(&self, config: OpenAIConfig) -> OpenAIConfig {
        if let Some(custom_endpont) = &self.custom_endpoint {
            config.with_api_base(custom_endpont)
        } else {
            config
        }
    }

    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve(GITBUTLER_ACCESS_TOKEN_HANDLE, secret::Namespace::BuildKind)?
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
}

impl OpenAIClientProvider for OpenAiProvider {
    fn client(&self) -> Result<Client<OpenAIConfig>> {
        let http_builder = but_http::client_builder()?;
        match &self.credentials {
            (CredentialsKind::OwnOpenAiKey, key) => {
                let config =
                    self.configure_custom_endpoint(OpenAIConfig::new().with_api_key(key.0.clone()));
                Ok(Client::build(http_builder.build()?, config))
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
                let http_client = http_builder.default_headers(headers).build()?;
                Ok(Client::build(http_client, config))
            }
        }
    }
}

impl LLMClient for OpenAiProvider {
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
