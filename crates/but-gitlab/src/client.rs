use anyhow::{Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::GitLabProjectId;

const GITLAB_API_BASE_URL: &str = "https://gitlab.com/api/v4";

pub struct GitLabClient {
    client: reqwest::Client,
    base_url: String,
}

impl GitLabClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitlab-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        Ok(Self {
            client,
            base_url: GITLAB_API_BASE_URL.to_string(),
        })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&crate::GitlabAccountIdentifier>,
    ) -> anyhow::Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_gl_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No GitLab access token found for account '{account_id}'.\nRun 'but config forge auth' to re-authenticate."
            ))
        }
    }

    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitlab-integration"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder().default_headers(headers).build()?;

        let base_url = if host.ends_with("/api/v4") {
            host.to_string()
        } else if host.ends_with('/') {
            format!("{host}api/v4")
        } else {
            format!("{host}/api/v4")
        };

        Ok(Self { client, base_url })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        #[derive(Deserialize)]
        struct User {
            username: String,
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
            username: user.username,
            avatar_url: user.avatar_url,
            name: user.name,
            email: user.email,
        })
    }

    pub async fn list_open_mrs(&self, project_id: GitLabProjectId) -> Result<Vec<MergeRequest>> {
        let url = format!("{}/projects/{}/merge_requests", self.base_url, project_id);

        let response = self
            .client
            .get(&url)
            .query(&[("state", "opened"), ("sort", "created_at")])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list open merge requests: {}", response.status());
        }

        let mrs: Vec<GitLabMergeRequest> = response.json().await?;
        Ok(mrs.into_iter().map(Into::into).collect())
    }

    pub async fn list_mrs_for_target(
        &self,
        project_id: GitLabProjectId,
        target_branch: &str,
    ) -> Result<Vec<MergeRequest>> {
        let url = format!("{}/projects/{}/merge_requests", self.base_url, project_id);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "all"),
                ("target_branch", target_branch),
                ("order_by", "updated_at"),
                ("sort", "desc"),
                ("per_page", "100"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list merge requests for target branch: {}", response.status());
        }

        let mrs: Vec<GitLabMergeRequest> = response.json().await?;
        Ok(mrs.into_iter().map(Into::into).collect())
    }

    pub async fn create_merge_request(&self, params: &CreateMergeRequestParams<'_>) -> Result<MergeRequest> {
        #[derive(Serialize)]
        struct CreateMergeRequestBody<'a> {
            title: &'a str,
            description: &'a str,
            source_branch: &'a str,
            target_branch: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            target_project_id: Option<i64>,
        }

        let url = format!("{}/projects/{}/merge_requests", self.base_url, params.project_id);

        let body = CreateMergeRequestBody {
            title: params.title,
            description: params.body,
            source_branch: params.source_branch,
            target_branch: params.target_branch,
            target_project_id: None,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to create merge request: {status} - {error_text}");
        }

        let mr: GitLabMergeRequest = response.json().await?;
        Ok(mr.into())
    }

    pub async fn get_merge_request(&self, project_id: GitLabProjectId, mr_iid: i64) -> Result<MergeRequest> {
        let url = format!("{}/projects/{}/merge_requests/{}", self.base_url, project_id, mr_iid);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get merge request: {}", response.status());
        }

        let mr: GitLabMergeRequest = response.json().await?;
        Ok(mr.into())
    }

    pub async fn merge_merge_request(&self, params: &MergeMergeRequestParams) -> Result<()> {
        #[derive(Serialize)]
        struct MergeMergeRequestBody {
            #[serde(skip_serializing_if = "Option::is_none")]
            squash: Option<bool>,
        }

        let url = format!(
            "{}/projects/{}/merge_requests/{}/merge",
            self.base_url, params.project_id, params.mr_iid
        );

        let body = MergeMergeRequestBody { squash: params.squash };

        let response = self.client.put(&url).json(&body).send().await?;

        if !response.status().is_success() {
            bail!("Failed to merge merge request: {}", response.status());
        }

        Ok(())
    }
}

pub struct CreateMergeRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source_branch: &'a str,
    pub target_branch: &'a str,
    pub project_id: GitLabProjectId,
}
pub struct MergeMergeRequestParams {
    pub project_id: GitLabProjectId,
    pub mr_iid: i64,
    pub squash: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AuthenticatedUser {
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitLabUser {
    pub id: i64,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Deserialize)]
struct GitLabApiUser {
    id: i64,
    username: String,
    name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    avatar_url: Option<String>,
    #[serde(default)]
    bot: bool,
}

impl From<GitLabApiUser> for GitLabUser {
    fn from(user: GitLabApiUser) -> Self {
        GitLabUser {
            id: user.id,
            username: user.username,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.bot,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GitLabLabel {
    pub name: String,
}

impl From<String> for GitLabLabel {
    fn from(name: String) -> Self {
        GitLabLabel { name }
    }
}

#[derive(Debug, Serialize)]
pub struct MergeRequest {
    pub web_url: String,
    pub iid: i64,
    pub title: String,
    pub description: Option<String>,
    pub author: Option<GitLabUser>,
    pub labels: Vec<GitLabLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub sha: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub project_id: i64,
    pub assignees: Vec<GitLabUser>,
    pub reviewers: Vec<GitLabUser>,
}

#[derive(Debug, Deserialize)]
struct GitLabMergeRequest {
    web_url: String,
    iid: i64,
    title: String,
    description: Option<String>,
    author: Option<GitLabApiUser>,
    labels: Vec<String>,
    draft: bool,
    source_branch: String,
    target_branch: String,
    sha: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    project_id: i64,
    #[serde(default)]
    assignees: Vec<GitLabApiUser>,
    #[serde(default)]
    reviewers: Vec<GitLabApiUser>,
}

impl From<GitLabMergeRequest> for MergeRequest {
    fn from(mr: GitLabMergeRequest) -> Self {
        let author = mr.author.map(Into::into);

        let assignees = mr.assignees.into_iter().map(Into::into).collect();
        let reviewers = mr.reviewers.into_iter().map(Into::into).collect();

        MergeRequest {
            web_url: mr.web_url,
            iid: mr.iid,
            title: mr.title,
            description: mr.description,
            author,
            labels: mr.labels.into_iter().map(Into::into).collect(),
            draft: mr.draft,
            source_branch: mr.source_branch,
            target_branch: mr.target_branch,
            sha: mr.sha,
            created_at: mr.created_at,
            updated_at: mr.updated_at,
            merged_at: mr.merged_at,
            closed_at: mr.closed_at,
            project_id: mr.project_id,
            assignees,
            reviewers,
        }
    }
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::GitlabAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_gitlab_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!("No authenticated GitLab users found.\nRun 'but config forge auth' to authenticate with GitLab.");
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred GitLab account '{account}' has not authenticated yet.\nRun 'but config forge auth' to authenticate, or choose another account."
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
