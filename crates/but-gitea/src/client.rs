//! Thin Gitea API client used by the authentication slice.

use anyhow::{Context, Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;

const GITEA_API_SUFFIX: &str = "/api/v1";

pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
}

impl GiteaClient {
    /// Create an authenticated client for an arbitrary Gitea-compatible host.
    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitea-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("token {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: normalize_api_base_url(host),
        })
    }

    /// Resolve the currently authenticated user from `/api/v1/user`.
    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        #[derive(Deserialize)]
        struct User {
            login: String,
            full_name: String,
            email: String,
            avatar_url: Option<String>,
        }

        let url = format!("{}/user", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get authenticated user: {}", response.status());
        }

        let user: User = response.json().await?;

        Ok(AuthenticatedUser {
            username: user.login,
            avatar_url: user.avatar_url,
            name: option_if_non_empty(user.full_name),
            email: option_if_non_empty(user.email),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

fn option_if_non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

/// Normalize either an instance root URL or API root to the `/api/v1` base path.
fn normalize_api_base_url(host: &str) -> String {
    let trimmed = host.trim_end_matches('/');
    if trimmed.ends_with(GITEA_API_SUFFIX) {
        trimmed.to_string()
    } else {
        format!("{trimmed}{GITEA_API_SUFFIX}")
    }
}

/// Recreate a client for a previously persisted account identifier.
pub fn client_for(
    account: &crate::GiteaAccountIdentifier,
    access_token: &Sensitive<String>,
) -> Result<GiteaClient> {
    GiteaClient::new_with_host_override(access_token, &account.host)
        .with_context(|| format!("Failed to create Gitea client for {}", account.host))
}

#[cfg(test)]
mod tests {
    use super::normalize_api_base_url;

    #[test]
    fn appends_api_suffix_once() {
        assert_eq!(
            normalize_api_base_url("https://codeberg.org"),
            "https://codeberg.org/api/v1"
        );
        assert_eq!(
            normalize_api_base_url("https://codeberg.org/"),
            "https://codeberg.org/api/v1"
        );
        assert_eq!(
            normalize_api_base_url("https://codeberg.org/api/v1"),
            "https://codeberg.org/api/v1"
        );
    }
}
