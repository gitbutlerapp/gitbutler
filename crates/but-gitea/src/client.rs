use anyhow::{Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::GiteaAccountIdentifier;

pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
}

impl GiteaClient {
    pub fn new(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-gitea-integration"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("token {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        // Ensure host has a scheme, default to https if not.
        let base_url = if host.contains("://") {
            host.to_string()
        } else {
            format!("https://{}", host)
        };

        // Gitea API is usually at /api/v1
        let base_url = if base_url.ends_with("/api/v1") {
            base_url
        } else if base_url.ends_with('/') {
            format!("{}api/v1", base_url)
        } else {
            format!("{}/api/v1", base_url)
        };

        Ok(Self {
            client,
            base_url,
        })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&GiteaAccountIdentifier>,
    ) -> anyhow::Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_gt_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No Gitea access token found for account '{account_id}'."
            ))
        }
    }

    pub async fn get_authenticated(&self, access_token: &Sensitive<String>) -> Result<AuthenticatedUser> {
        #[derive(Deserialize)]
        struct User {
            login: String,
            full_name: Option<String>,
            email: Option<String>,
            avatar_url: Option<String>,
        }

        let url = format!("{}/user", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get authenticated user: {}", response.status());
        }

        let user: User = response.json().await?;

        Ok(AuthenticatedUser {
            access_token: access_token.clone(),
            login: user.login,
            avatar_url: user.avatar_url,
            name: user.full_name,
            email: user.email,
        })
    }

    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);
        let response = self
            .client
            .get(&url)
            .query(&[("state", "open")])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list open pull requests: {}", response.status());
        }

        let pulls: Vec<GiteaPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_index: i64) -> Result<PullRequest> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_index);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get pull request: {}", response.status());
        }

        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }
}

fn resolve_account(
    preferred_account: Option<&GiteaAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<GiteaAccountIdentifier> {
    if let Some(account) = preferred_account {
        return Ok(account.clone());
    }

    let accounts = crate::token::list_known_gitea_accounts(storage)?;
    if accounts.len() == 1 {
        return Ok(accounts[0].clone());
    }

    if accounts.is_empty() {
        bail!("No Gitea accounts found.");
    }

    bail!("Multiple Gitea accounts found, please specify one.");
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: but_secret::Sensitive<String>,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/client.ts"))]
pub struct GiteaUser {
    pub id: i64,
    pub login: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/client.ts"))]
pub struct GiteaPrLabel {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/client.ts"))]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<GiteaUser>,
    pub labels: Vec<GiteaPrLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GiteaUserApi {
    id: i64,
    login: String,
    full_name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
}

impl From<GiteaUserApi> for GiteaUser {
    fn from(user: GiteaUserApi) -> Self {
        GiteaUser {
            id: user.id,
            login: user.login,
            full_name: user.full_name,
            email: user.email,
            avatar_url: user.avatar_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GiteaPrLabelApi {
    id: i64,
    name: String,
    description: Option<String>,
    color: Option<String>,
}

impl From<GiteaPrLabelApi> for GiteaPrLabel {
    fn from(label: GiteaPrLabelApi) -> Self {
        GiteaPrLabel {
            id: label.id,
            name: label.name,
            description: label.description,
            color: Some(label.color),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GiteaBranch {
    #[serde(rename = "ref")]
    ref_: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GiteaPullRequest {
    html_url: String,
    number: i64,
    title: String,
    body: Option<String>,
    user: Option<GiteaUserApi>,
    labels: Option<Vec<GiteaPrLabelApi>>,
    draft: bool,
    head: GiteaBranch,
    base: GiteaBranch,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
}

impl From<GiteaPullRequest> for PullRequest {
    fn from(pr: GiteaPullRequest) -> Self {
        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author: pr.user.map(Into::into),
            labels: pr.labels.unwrap_or_default().into_iter().map(Into::into).collect(),
            draft: pr.draft,
            source_branch: pr.head.ref_,
            target_branch: pr.base.ref_,
            sha: pr.head.sha,
            created_at: pr.created_at,
            modified_at: pr.updated_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
        }
    }
}
