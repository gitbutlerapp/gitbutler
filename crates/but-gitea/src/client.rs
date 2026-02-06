use anyhow::{Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
}

impl GiteaClient {
    /// Create a new Gitea client for the given host.
    /// Gitea is always self-hosted, so a host URL is required.
    /// The host should be like "https://gitea.example.com" or "https://codeberg.org".
    pub fn new(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitea-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        // Gitea uses "token" scheme for API tokens
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("token {}", access_token.0))?,
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        let base_url = if host.ends_with("/api/v1") {
            host.to_string()
        } else if host.ends_with('/') {
            format!("{}api/v1", host)
        } else {
            format!("{}/api/v1", host)
        };

        Ok(Self { client, base_url })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&crate::GiteaAccountIdentifier>,
    ) -> anyhow::Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_gitea_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No Gitea access token found for account '{}'.\nPlease, try to re-authenticate with this account.",
                account_id
            ))
        }
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
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
            username: user.login,
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
            .query(&[
                ("state", "open"),
                ("sort", "newest"),
                ("limit", "50"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list open pull requests: {}", response.status());
        }

        let prs: Vec<GiteaPullRequest> = response.json().await?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    pub async fn list_pulls_for_branch(&self, owner: &str, repo: &str, base: &str) -> Result<Vec<PullRequest>> {
        // Gitea doesn't have a direct filter for base branch in the list endpoint,
        // so we fetch all and filter. For larger repos, pagination would be needed.
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "all"),
                ("sort", "newest"),
                ("limit", "100"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list pull requests for branch: {}", response.status());
        }

        let prs: Vec<GiteaPullRequest> = response.json().await?;
        Ok(prs
            .into_iter()
            .filter(|pr| pr.base.as_ref().is_some_and(|b| b.ref_field == base))
            .map(Into::into)
            .collect())
    }

    pub async fn create_pull_request(&self, params: &CreatePullRequestParams<'_>) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct CreatePullRequestBody<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            base: &'a str,
        }

        let url = format!("{}/repos/{}/{}/pulls", self.base_url, params.owner, params.repo);

        let body = CreatePullRequestBody {
            title: params.title,
            body: params.body,
            head: params.head,
            base: params.base,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to create pull request: {} - {}", status, error_text);
        }

        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: i64) -> Result<PullRequest> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, owner, repo, pr_number
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get pull request: {}", response.status());
        }

        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn update_pull_request(&self, params: &UpdatePullRequestParams<'_>) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct UpdatePullRequestBody<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            title: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            body: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            base: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            state: Option<&'a str>,
        }

        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, params.owner, params.repo, params.pr_number
        );

        let body = UpdatePullRequestBody {
            title: params.title,
            body: params.body,
            base: params.base,
            state: params.state,
        };

        let response = self.client.patch(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to update pull request: {} - {}", status, error_text);
        }

        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }
}

pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
}

pub struct UpdatePullRequestParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub pr_number: i64,
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
    pub base: Option<&'a str>,
    pub state: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatedUser {
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GiteaUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Deserialize)]
struct GiteaApiUser {
    id: i64,
    login: String,
    full_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    avatar_url: Option<String>,
}

impl From<GiteaApiUser> for GiteaUser {
    fn from(user: GiteaApiUser) -> Self {
        GiteaUser {
            id: user.id,
            login: user.login,
            name: user.full_name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GiteaLabel {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
}

#[derive(Debug, Serialize)]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<GiteaUser>,
    pub labels: Vec<GiteaLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub requested_reviewers: Vec<GiteaUser>,
}

// Internal deserialization types matching the Gitea API response format

#[derive(Debug, Deserialize)]
struct GiteaApiLabel {
    id: i64,
    name: String,
    #[serde(default)]
    description: Option<String>,
    color: String,
}

impl From<GiteaApiLabel> for GiteaLabel {
    fn from(label: GiteaApiLabel) -> Self {
        GiteaLabel {
            id: label.id,
            name: label.name,
            description: label.description,
            color: label.color,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GiteaPrRef {
    #[serde(rename = "ref")]
    ref_field: String,
    sha: String,
    repo: Option<GiteaPrRepo>,
}

#[derive(Debug, Deserialize)]
struct GiteaPrRepo {
    ssh_url: Option<String>,
    clone_url: Option<String>,
    owner: Option<GiteaPrRepoOwner>,
}

#[derive(Debug, Deserialize)]
struct GiteaPrRepoOwner {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GiteaPullRequest {
    html_url: String,
    number: i64,
    title: String,
    body: Option<String>,
    user: Option<GiteaApiUser>,
    #[serde(default)]
    labels: Vec<GiteaApiLabel>,
    #[serde(default)]
    draft: bool,
    head: Option<GiteaPrRef>,
    base: Option<GiteaPrRef>,
    #[serde(default)]
    #[allow(dead_code)]
    merge_base: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    #[serde(default)]
    requested_reviewers: Vec<GiteaApiUser>,
}

impl From<GiteaPullRequest> for PullRequest {
    fn from(pr: GiteaPullRequest) -> Self {
        let author = pr.user.map(Into::into);
        let labels = pr.labels.into_iter().map(Into::into).collect();

        let source_branch = pr
            .head
            .as_ref()
            .map(|h| h.ref_field.clone())
            .unwrap_or_default();
        let target_branch = pr
            .base
            .as_ref()
            .map(|b| b.ref_field.clone())
            .unwrap_or_default();
        let sha = pr
            .head
            .as_ref()
            .map(|h| h.sha.clone())
            .unwrap_or_default();

        let repository_ssh_url = pr
            .head
            .as_ref()
            .and_then(|h| h.repo.as_ref())
            .and_then(|r| r.ssh_url.clone());
        let repository_https_url = pr
            .head
            .as_ref()
            .and_then(|h| h.repo.as_ref())
            .and_then(|r| r.clone_url.clone());
        let repo_owner = pr
            .head
            .as_ref()
            .and_then(|h| h.repo.as_ref())
            .and_then(|r| r.owner.as_ref())
            .map(|o| o.login.clone());

        let requested_reviewers = pr.requested_reviewers.into_iter().map(Into::into).collect();

        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author,
            labels,
            draft: pr.draft,
            source_branch,
            target_branch,
            sha,
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url,
            repository_https_url,
            repo_owner,
            requested_reviewers,
        }
    }
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::GiteaAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_gitea_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!("No authenticated Gitea users found. Please authenticate with Gitea first.");
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred Gitea account '{}' has not authenticated yet. Please choose another account or authenticate with the desired account first.",
                account
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
