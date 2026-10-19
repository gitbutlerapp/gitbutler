use anyhow::{Context, Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITEE_API_BASE_URL: &str = "https://gitee.com/api/v5";
const GITEE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PR_PAGES: usize = 10;

/// An HTTP error with a status code, returned when the API responds with a non-success status.
///
/// This can be downcasted from `anyhow::Error` to distinguish auth failures (401/403) from other errors.
#[derive(Debug, thiserror::Error)]
#[error("HTTP {status}")]
pub struct HttpStatusError {
    pub status: reqwest::StatusCode,
}

/// Client for the Gitee API v5 (https://gitee.com/api/v5/swagger).
///
/// Gitee's API is GitHub-v3-flavoured; personal access tokens are passed as
/// query parameter or `Authorization: Bearer` header.
pub struct GiteeClient {
    client: reqwest::Client,
    base_url: String,
    access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeRepo {
    pub id: i64,
    pub full_name: String,
    pub human_name: Option<String>,
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    pub default_branch: Option<String>,
    #[serde(default)]
    pub licensee: Option<serde_json::Value>,
    #[serde(default)]
    pub permissions: Option<GiteeRepoPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GiteeRepoPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub pull: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteePullRequest {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    /// `open`, `closed`, `rejected` (declined) or `merged`.
    #[serde(default)]
    pub merged_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub diff_url: Option<String>,
    pub user: Option<GiteeUser>,
    pub head: Option<GiteePullRequestBranch>,
    pub base: Option<GiteePullRequestBranch>,
    #[serde(default)]
    pub labels: Option<Vec<GiteeLabel>>,
    #[serde(default)]
    pub draft: Option<bool>,
}

impl GiteePullRequest {
    pub fn is_open(&self) -> bool {
        self.state == "open"
    }
    /// True when the PR ended up merged (as opposed to rejected).
    pub fn is_merged(&self) -> bool {
        self.merged_at.is_some() || self.state == "merged"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteePullRequestBranch {
    pub label: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
    pub repo: Option<GiteeRepo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeLabel {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GiteeCommitStatus {
    pub state: String,
    pub description: Option<String>,
    pub target_url: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeBranch {
    pub name: String,
    pub commit: GiteeBranchCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeBranchCommit {
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePullRequestParams {
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: Option<String>,
}

impl GiteeClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        Self::with_base_url(access_token, GITEE_API_BASE_URL)
    }

    pub fn with_base_url(access_token: &Sensitive<String>, base_url: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitee-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(GITEE_REQUEST_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token: access_token.0.clone(),
        })
    }

    fn url(&self, path: &str, extra_query: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}{}?access_token={}",
            self.base_url,
            path,
            urlencoding::encode(&self.access_token)
        );
        for (k, v) in extra_query {
            url.push_str(&format!("&{k}={}", urlencoding::encode(v)));
        }
        url
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send request to Gitee")?;
        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::Error::new(HttpStatusError { status })
                .context(format!("Gitee API request failed: {url}")));
        }
        response
            .json::<T>()
            .await
            .context("Failed to parse Gitee API response")
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .context("Failed to send request to Gitee")?;
        let status = response.status();
        if !status.is_success() {
            bail!("Gitee API request failed with status {status}: {url}");
        }
        response
            .json::<T>()
            .await
            .context("Failed to parse Gitee API response")
    }

    async fn patch_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let response = self
            .client
            .patch(url)
            .json(body)
            .send()
            .await
            .context("Failed to send request to Gitee")?;
        let status = response.status();
        if !status.is_success() {
            bail!("Gitee API request failed with status {status}: {url}");
        }
        response
            .json::<T>()
            .await
            .context("Failed to parse Gitee API response")
    }

    /// GET an endpoint that may return a JSON array or a bare `null`/object-error body.
    async fn get_json_or_empty<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<Vec<T>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send request to Gitee")?;
        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::Error::new(HttpStatusError { status })
                .context(format!("Gitee API request failed: {url}")));
        }
        let text = response
            .text()
            .await
            .context("Failed to read Gitee API response")?;
        if text.trim().is_empty() || text.trim() == "null" {
            return Ok(vec![]);
        }
        serde_json::from_str::<Vec<T>>(&text)
            .map_err(|e| anyhow::Error::new(e).context("Failed to parse Gitee API response"))
    }

    pub async fn get_authenticated(&self) -> Result<GiteeUser> {
        self.get_json(&self.url("/user", &[])).await
    }

    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<GiteeRepo> {
        let path = format!("/repos/{owner}/{repo}");
        self.get_json(&self.url(&path, &[])).await
    }

    pub async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<GiteeBranch>> {
        let path = format!("/repos/{owner}/{repo}/branches");
        self.get_json_or_empty(&self.url(&path, &[("per_page", "100")]))
            .await
    }

    /// List pull requests for a repo. `state`: `open`, `closed`, `all`.
    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
    ) -> Result<Vec<GiteePullRequest>> {
        let path = format!("/repos/{owner}/{repo}/pulls");
        let mut all = Vec::new();
        for page in 1..=MAX_PR_PAGES {
            let page_str = page.to_string();
            let page_prs: Vec<GiteePullRequest> = self
                .get_json_or_empty(&self.url(
                    &path,
                    &[
                        ("state", state),
                        ("per_page", "50"),
                        ("page", page_str.as_str()),
                    ],
                ))
                .await?;
            let exhausted = page_prs.len() < 50;
            all.extend(page_prs);
            if exhausted {
                break;
            }
        }
        Ok(all)
    }

    /// List PRs whose head branch matches `branch_name`.
    pub async fn list_pull_requests_for_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
    ) -> Result<Vec<GiteePullRequest>> {
        let all = self.list_pull_requests(owner, repo, "all").await?;
        Ok(all
            .into_iter()
            .filter(|pr| {
                pr.head
                    .as_ref()
                    .map(|head| head.ref_name == branch_name)
                    .unwrap_or(false)
            })
            .collect())
    }

    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<GiteePullRequest> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}");
        self.get_json(&self.url(&path, &[])).await
    }

    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: &CreatePullRequestParams,
    ) -> Result<GiteePullRequest> {
        let path = format!("/repos/{owner}/{repo}/pulls");
        let body = serde_json::json!({
            "title": params.title,
            "head": params.head,
            "base": params.base,
            "body": params.body,
        });
        self.post_json(&self.url(&path, &[]), &body).await
    }

    pub async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        title: Option<&str>,
        body: Option<&str>,
        state: Option<&str>,
    ) -> Result<GiteePullRequest> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}");
        let mut payload = serde_json::Map::new();
        if let Some(title) = title {
            payload.insert("title".into(), serde_json::Value::String(title.into()));
        }
        if let Some(body) = body {
            payload.insert("body".into(), serde_json::Value::String(body.into()));
        }
        if let Some(state) = state {
            payload.insert("state".into(), serde_json::Value::String(state.into()));
        }
        self.patch_json(&self.url(&path, &[]), &serde_json::Value::Object(payload))
            .await
    }

    /// Combined commit status (from Gitee Go / external CI integrations).
    /// Gitee v5 mirrors GitHub's `/statuses` and `/status` endpoints.
    pub async fn get_combined_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<GiteeCommitStatus> {
        let path = format!("/repos/{owner}/{repo}/commits/{sha}/status");
        #[derive(Deserialize)]
        struct Combined {
            state: Option<String>,
            #[serde(default)]
            statuses: Option<Vec<GiteeCommitStatus>>,
        }
        let combined: Combined = self.get_json(&self.url(&path, &[])).await?;
        Ok(GiteeCommitStatus {
            state: combined.state.unwrap_or_else(|| "unknown".to_string()),
            description: combined
                .statuses
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.description.clone()),
            target_url: combined
                .statuses
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.target_url.clone()),
            context: None,
        })
    }
}

/// The default Gitee.com API base URL.
pub fn api_base_url() -> &'static str {
    GITEE_API_BASE_URL
}
