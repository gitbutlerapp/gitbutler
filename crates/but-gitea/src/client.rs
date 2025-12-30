use anyhow::{Context, Result};
use but_secret::Sensitive;
use serde::Deserialize;
use serde::Serialize;

pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
    token: Sensitive<String>,
}

impl GiteaClient {
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
            .header("Authorization", format!("token {}", self.token.expose()))
            .header("Accept", "application/json")
    }

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

    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        let path = format!("/repos/{}/{}/pulls", params.owner, params.repo);

        #[derive(Serialize)]
        struct CreatePrBody<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            base: &'a str,
        }

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

pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaUser {
    pub id: i64,
    pub login: String,
    pub full_name: Option<String>,
    pub email: String,
    pub avatar_url: String,
    #[serde(default)]
    pub is_bot: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaPullRequest {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub user: GiteaUser,
    pub html_url: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merge_base: Option<String>,
    pub head: GiteaBranchInfo,
    pub base: GiteaBranchInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GiteaBranchInfo {
    pub label: String,
    pub r#ref: String,
    pub sha: String,
    pub repo: Option<GiteaRepo>,
}

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

#[derive(Debug, Serialize)]
pub struct PullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: GiteaUser,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: String,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
}

impl From<GiteaPullRequest> for PullRequest {
    fn from(pr: GiteaPullRequest) -> Self {
        PullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author: pr.user,
            source_branch: pr.head.r#ref,
            target_branch: pr.base.r#ref,
            sha: pr.head.sha,
            created_at: pr.created_at,
            repository_ssh_url: pr.base.repo.as_ref().map(|r| r.ssh_url.clone()),
            repository_https_url: pr.base.repo.as_ref().map(|r| r.clone_url.clone()),
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
