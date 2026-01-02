//! Gitea API client and data types.
#![allow(missing_docs)]

use anyhow::{Context, Result};
use but_secret::Sensitive;
use serde::Deserialize;
use serde::Serialize;

/// Gitea API client for interacting with Gitea instances.
pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
    token: Sensitive<String>,
}

impl GiteaClient {
    /// Create a new Gitea client.
    pub fn new(host: &str, access_token: &Sensitive<String>) -> Result<Self> {
        let client = reqwest::Client::new();
        // Ensure host has protocol
        let base_url = if host.starts_with("http") {
            format!("{}/api/v1", host.trim_end_matches('/'))
        } else {
            format!("https://{}/api/v1", host.trim_end_matches('/'))
        };

        Ok(Self {
            client,
            base_url,
            token: access_token.clone(),
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{}", self.base_url, path))
            .header("Authorization", format!("token {}", self.token.0))
            .header("Accept", "application/json")
    }

    /// Get the currently authenticated user.
    pub async fn get_authenticated(&self) -> Result<GiteaUser> {
        let resp = self
            .request(reqwest::Method::GET, "/user")
            .send()
            .await
            .context("Failed to send request")?;

        if !resp.status().is_success() {
            anyhow::bail!("Request failed with status: {}", resp.status());
        }

        resp.json::<GiteaUser>()
            .await
            .context("Failed to parse response")
    }

    /// List open pull requests for a repository.
    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let path = format!(
            "/repos/{}/{}/pulls?state=open&sort=recentupdate",
            owner, repo
        );
        let resp = self
            .request(reqwest::Method::GET, &path)
            .send()
            .await
            .context("Failed to list PRs")?;

        if !resp.status().is_success() {
            anyhow::bail!("Request failed with status: {}", resp.status());
        }

        let prs: Vec<GiteaPullRequest> = resp.json().await.context("Failed to parse PR list")?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    /// Get a specific pull request.
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<PullRequest> {
        let path = format!("/repos/{}/{}/pulls/{}", owner, repo, number);
        let resp = self
            .request(reqwest::Method::GET, &path)
            .send()
            .await
            .context("Failed to get PR")?;

        if !resp.status().is_success() {
            anyhow::bail!("Request failed with status: {}", resp.status());
        }

        let pr: GiteaPullRequest = resp.json().await.context("Failed to parse PR")?;
        Ok(pr.into())
    }

    /// Create a new pull request.
    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        let path = format!("/repos/{}/{}/pulls", params.owner, params.repo);

        let body = CreatePrBody {
            title: params.title,
            body: params.body,
            head: params.head,
            base: params.base,
        };

        let resp = self
            .request(reqwest::Method::POST, &path)
            .json(&body)
            .send()
            .await
            .context("Failed to create PR")?;

        if !resp.status().is_success() {
            anyhow::bail!("Request failed with status: {}", resp.status());
        }

        let pr: GiteaPullRequest = resp.json().await.context("Failed to parse PR response")?;
        Ok(pr.into())
    }
}

/// Body for creating a pull request (internal use).
#[derive(Serialize)]
struct CreatePrBody<'a> {
    title: &'a str,
    body: &'a str,
    head: &'a str,
    base: &'a str,
}

/// Parameters for creating a pull request.
pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
}

/// Gitea user information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaUser {
    pub id: i64,
    pub login: String,
    pub full_name: Option<String>,
    pub email: String,
    pub avatar_url: String,
    #[serde(default)]
    pub is_bot: bool,
}

/// Gitea pull request as returned by the API.
#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaPullRequest {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub user: GiteaUser,
    pub html_url: String,
    pub state: String,
    #[serde(default)]
    pub draft: bool,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merge_base: Option<String>,
    pub head: GiteaBranchInfo,
    pub base: GiteaBranchInfo,
    #[serde(default)]
    pub labels: Vec<GiteaLabel>,
    #[serde(default)]
    pub requested_reviewers: Vec<GiteaUser>,
}

/// Gitea label information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteaLabel {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// Gitea branch information.
#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaBranchInfo {
    pub label: String,
    pub r#ref: String,
    pub sha: String,
    pub repo: Option<GiteaRepo>,
}

/// Gitea repository information.
#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaRepo {
    pub id: i64,
    pub owner: GiteaUser,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub empty: bool,
    pub private: bool,
    pub fork: bool,
    pub template: bool,
    pub parent: Option<Box<GiteaRepo>>,
    pub mirror: bool,
    pub size: i64,
    pub html_url: String,
    pub ssh_url: String,
    pub clone_url: String,
    pub website: Option<String>,
    pub stars_count: i64,
    pub forks_count: i64,
    pub watchers_count: i64,
    pub open_issues_count: i64,
    pub default_branch: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Normalized pull request representation.
#[derive(Debug, Clone, Serialize)]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: GiteaUser,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: String,
    pub modified_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub labels: Vec<GiteaLabel>,
    pub requested_reviewers: Vec<GiteaUser>,
}

impl From<GiteaPullRequest> for PullRequest {
    fn from(pr: GiteaPullRequest) -> Self {
        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author: pr.user,
            draft: pr.draft,
            source_branch: pr.head.r#ref,
            target_branch: pr.base.r#ref,
            sha: pr.head.sha,
            created_at: pr.created_at,
            modified_at: Some(pr.updated_at),
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: pr.base.repo.as_ref().map(|r| r.ssh_url.clone()),
            repository_https_url: pr.base.repo.as_ref().map(|r| r.clone_url.clone()),
            repo_owner: pr.base.repo.as_ref().map(|r| r.owner.login.clone()),
            labels: pr.labels,
            requested_reviewers: pr.requested_reviewers,
        }
    }
}
