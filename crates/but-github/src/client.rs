use anyhow::{Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

const GITHUB_API_BASE_URL: &str = "https://api.github.com";

pub struct GitHubClient {
    client: reqwest::Client,
    base_url: String,
}

impl GitHubClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-github-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        Ok(Self {
            client,
            base_url: GITHUB_API_BASE_URL.to_string(),
        })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&crate::GithubAccountIdentifier>,
    ) -> anyhow::Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_gh_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No GitHub access token found for account '{account_id}'.\nRun 'but config forge auth' to re-authenticate."
            ))
        }
    }

    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-github-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        Ok(Self {
            client,
            base_url: host.to_string(),
        })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        #[derive(Deserialize)]
        struct User {
            login: String,
            name: Option<String>,
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
            login: user.login,
            avatar_url: user.avatar_url,
            name: user.name,
            email: user.email,
        })
    }

    pub async fn list_checks_for_ref(&self, owner: &str, repo: &str, reference: &str) -> Result<Vec<CheckRun>> {
        #[derive(Deserialize)]
        struct CheckRunsResponse {
            check_runs: Vec<CheckRun>,
        }

        let url = format!(
            "{}/repos/{}/{}/commits/{}/check-runs",
            self.base_url, owner, repo, reference
        );

        let response = self.client.get(&url).query(&[("filter", "latest")]).send().await?;

        if !response.status().is_success() {
            bail!("Failed to list checks for ref: {}", response.status());
        }

        let result: CheckRunsResponse = response.json().await?;
        Ok(result.check_runs)
    }

    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "open"),
                ("sort", "updated"),
                ("direction", "desc"),
                ("per_page", "20"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list open pulls: {}", response.status());
        }

        let pulls: Vec<GitHubPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    pub async fn list_pulls_for_base(&self, owner: &str, repo: &str, base: &str) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "all"),
                ("base", base),
                ("sort", "updated"),
                ("direction", "desc"),
                ("per_page", "100"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list pulls for base: {}", response.status());
        }

        let pulls: Vec<GitHubPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    pub async fn create_pull_request(&self, params: &CreatePullRequestParams<'_>) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct CreatePullRequestBody<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            base: &'a str,
            draft: bool,
        }

        let url = format!("{}/repos/{}/{}/pulls", self.base_url, params.owner, params.repo);

        let body = CreatePullRequestBody {
            title: params.title,
            body: params.body,
            head: params.head,
            base: params.base,
            draft: params.draft,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            bail!("Failed to create pull request: {}", response.status());
        }

        let pr: GitHubPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: i64) -> Result<PullRequest> {
        let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get pull request: {}", response.status());
        }

        let pr: GitHubPullRequest = response.json().await?;
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
            bail!("Failed to update pull request: {}", response.status());
        }

        let pr: GitHubPullRequest = response.json().await?;
        Ok(pr.into())
    }
}

pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub draft: bool,
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
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    pub id: i64,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub head_sha: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub details_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubApiUser {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
    #[serde(rename = "type")]
    user_type: Option<String>,
}

impl From<GitHubApiUser> for GitHubUser {
    fn from(user: GitHubApiUser) -> Self {
        GitHubUser {
            id: user.id,
            login: user.login,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.user_type.map(|user_type| user_type == "bot").unwrap_or(false),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GitHubPrLabel {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<GitHubUser>,
    pub labels: Vec<GitHubPrLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub requested_reviewers: Vec<GitHubUser>,
}

#[derive(Debug, Deserialize)]
struct GitHubPrLabelApi {
    id: i64,
    name: String,
    description: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubBranch {
    #[serde(rename = "ref")]
    ref_: String,
    sha: String,
    repo: Option<GitHubRepo>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepo {
    ssh_url: Option<String>,
    clone_url: Option<String>,
    owner: Option<GitHubApiUser>,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequest {
    html_url: String,
    number: i64,
    title: String,
    body: Option<String>,
    user: Option<GitHubApiUser>,
    labels: Vec<GitHubPrLabelApi>,
    draft: bool,
    head: GitHubBranch,
    base: GitHubBranch,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    requested_reviewers: Vec<GitHubApiUser>,
}

impl From<GitHubPullRequest> for PullRequest {
    fn from(pr: GitHubPullRequest) -> Self {
        let author = pr.user.map(Into::into);

        let labels = pr
            .labels
            .into_iter()
            .map(|label| GitHubPrLabel {
                id: label.id,
                name: label.name,
                description: label.description,
                color: label.color,
            })
            .collect();

        let requested_reviewers = pr.requested_reviewers.into_iter().map(Into::into).collect();

        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author,
            labels,
            draft: pr.draft,
            source_branch: pr.head.ref_,
            target_branch: pr.base.ref_,
            sha: pr.head.sha,
            created_at: pr.created_at,
            modified_at: pr.updated_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: pr.base.repo.as_ref().and_then(|r| r.ssh_url.clone()),
            repository_https_url: pr.head.repo.as_ref().and_then(|r| r.clone_url.clone()),
            repo_owner: pr.head.repo.and_then(|r| r.owner.map(|o| o.login)),
            requested_reviewers,
        }
    }
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::GithubAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_github_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!("No authenticated GitHub users found.\nRun 'but config forge auth' to authenticate with GitHub.");
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred GitHub account '{account}' has not authenticated yet.\nRun 'but config forge auth' to authenticate, or choose another account."
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
