use anyhow::{Result, bail};
use async_openai::{
    Client,
    config::{Config, OpenAIConfig},
};
use gitbutler_secret::secret;
use reqwest::header::{HeaderMap, HeaderValue};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

pub struct OpenAiProvider {
    http_client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(http_client: reqwest::Client) -> Result<Self> {
        Ok(Self { http_client })
    }

    #[allow(dead_code)]
    pub fn open_ai(self) -> Result<Client<OpenAIConfig>> {
        let openai_api_key = secret::retrieve("aiOpenAIKey", secret::Namespace::Global)?;
        let config = if let Some(key) = openai_api_key {
            OpenAIConfig::new().with_api_key(key.0)
        } else {
            if std::env::var_os("OPENAI_API_KEY").is_none() {
                bail!(
                    "No OpenAI API key available. Either configure via GitButler or provide OPENAI_API_KEY env variable"
                )
            }
            OpenAIConfig::new()
        };
        Ok(Client::with_config(config).with_http_client(self.http_client))
    }

    pub fn gitbutler(self) -> Result<Client<GitButlerConfig>> {
        let gitbutler_token =
            secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?.ok_or(
                anyhow::anyhow!(
                    "No GitButler token available. Log-in to use the GitButler OpenAI provider"
                ),
            )?;
        let config = GitButlerConfig {
            token: SecretString::new(gitbutler_token.0.into()),
        };
        Ok(Client::with_config(config).with_http_client(self.http_client))
    }
}

pub const GB_OPENAI_API_BASE: &str = "https://app.gitbutler.com/api/proxy/openai";

#[derive(Clone, Debug, Deserialize)]
pub struct GitButlerConfig {
    token: SecretString,
}

impl Config for GitButlerConfig {
    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "X-Auth-Token",
            self.token
                .expose_secret()
                .parse()
                .unwrap_or(HeaderValue::from_static("")),
        );
        headers
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base(), path)
    }

    fn api_base(&self) -> &str {
        GB_OPENAI_API_BASE
    }

    fn api_key(&self) -> &SecretString {
        &self.token
    }

    fn query(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}
