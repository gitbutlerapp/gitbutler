use anyhow::{Context, Result};
use async_openai::{Client, config::OpenAIConfig};
use gitbutler_secret::{Sensitive, secret};
use reqwest::header::{HeaderMap, HeaderValue};

#[allow(unused)]
#[derive(Debug, Clone, serde::Serialize, strum::Display)]
pub enum CredentialsKind {
    EnvVarOpenAiKey,
    OwnOpenAiKey,
    GitButlerProxied,
}

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    credentials: (CredentialsKind, Sensitive<String>),
}

impl OpenAiProvider {
    pub fn with(preferred_creds: Option<CredentialsKind>) -> Option<Self> {
        let credentials = if let Some(kind) = preferred_creds {
            match kind {
                CredentialsKind::EnvVarOpenAiKey => OpenAiProvider::openai_env_var_creds(),
                CredentialsKind::OwnOpenAiKey => OpenAiProvider::openai_own_key_creds(),
                CredentialsKind::GitButlerProxied => OpenAiProvider::gitbutler_proxied_creds(),
            }
        } else {
            OpenAiProvider::openai_env_var_creds()
                .or_else(|_| OpenAiProvider::openai_own_key_creds())
                .or_else(|_| OpenAiProvider::gitbutler_proxied_creds())
                .context("No OpenAI credentials found. This can be configured in the app or read from a OPENAI_API_KEY environment variable")
        };

        match credentials {
            Ok(credentials) => Some(Self { credentials }),
            Err(e) => {
                tracing::error!("Failed to retrieve OpenAI credentials: {}", e);
                None
            }
        }
    }

    pub fn client(&self) -> Result<Client<OpenAIConfig>> {
        match &self.credentials {
            (CredentialsKind::EnvVarOpenAiKey, _) => Ok(Client::with_config(OpenAIConfig::new())),
            (CredentialsKind::OwnOpenAiKey, key) => Ok(Client::with_config(
                OpenAIConfig::new().with_api_key(key.0.clone()),
            )),
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
                let http_client = reqwest::Client::builder()
                    .default_headers(headers)
                    .build()?;
                Ok(Client::with_config(config).with_http_client(http_client))
            }
        }
    }

    pub fn credentials_kind(&self) -> CredentialsKind {
        self.credentials.0.clone()
    }

    fn gitbutler_proxied_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?
            .ok_or(anyhow::anyhow!(
                "No GitButler token available. Log-in to use the GitButler OpenAI provider"
            ))?;
        Ok((CredentialsKind::GitButlerProxied, creds))
    }

    fn openai_own_key_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds =
            secret::retrieve("aiOpenAIKey", secret::Namespace::Global)?.ok_or(anyhow::anyhow!(
                "No OpenAI own key configured. Add this through the GitButler settings"
            ))?;
        Ok((CredentialsKind::OwnOpenAiKey, creds))
    }

    fn openai_env_var_creds() -> Result<(CredentialsKind, Sensitive<String>)> {
        let creds = Sensitive(
            std::env::var_os("OPENAI_API_KEY")
                .ok_or(anyhow::anyhow!(
                    "Environment variable OPENAI_API_KEY is not set"
                ))?
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in OPENAI_API_KEY"))?,
        );
        Ok((CredentialsKind::EnvVarOpenAiKey, creds))
    }
}
