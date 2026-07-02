use anyhow::{Context, Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

use crate::GitLabProjectId;

const GITLAB_API_BASE_URL: &str = "https://gitlab.com/api/v4";
const GITLAB_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PIPELINE_JOB_PAGES: usize = 25;

/// An HTTP error with a status code, returned when the API responds with a non-success status.
///
/// This can be downcasted from `anyhow::Error` to distinguish auth failures (401/403) from other errors.
#[derive(Debug, thiserror::Error)]
#[error("HTTP {status}")]
pub struct HttpStatusError {
    pub status: reqwest::StatusCode,
}

pub struct GitLabClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug)]
struct MergeRequestEnrichmentError {
    source: anyhow::Error,
    mr: MergeRequest,
}

impl MergeRequestEnrichmentError {
    fn into_inner(self) -> MergeRequest {
        self.mr
    }

    fn source(&self) -> &anyhow::Error {
        &self.source
    }
}

impl GitLabClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-gitlab-integration"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(GITLAB_REQUEST_TIMEOUT)
            .build()?;

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
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-gitlab-integration"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(GITLAB_REQUEST_TIMEOUT)
            .build()?;

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
            return Err(HttpStatusError {
                status: response.status(),
            }
            .into());
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
            .query(&[("state", "opened"), ("order_by", "created_at")])
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
            bail!(
                "Failed to list merge requests for target branch: {}",
                response.status()
            );
        }

        let mrs: Vec<GitLabMergeRequest> = response.json().await?;
        Ok(mrs.into_iter().map(Into::into).collect())
    }

    pub async fn list_mrs_for_commit(
        &self,
        project_id: GitLabProjectId,
        commit_sha: &str,
    ) -> Result<Vec<MergeRequest>> {
        let url = format!(
            "{}/projects/{}/repository/commits/{}/merge_requests",
            self.base_url, project_id, commit_sha
        );

        let response = self
            .client
            .get(&url)
            .query(&[("per_page", "100")])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!(
                "Failed to list merge requests for commit: {}",
                response.status()
            );
        }

        let mrs: Vec<GitLabMergeRequest> = response.json().await?;
        Ok(mrs.into_iter().map(Into::into).collect())
    }

    pub async fn create_merge_request(
        &self,
        params: &CreateMergeRequestParams<'_>,
    ) -> Result<MergeRequest> {
        #[derive(Serialize)]
        struct CreateMergeRequestBody<'a> {
            title: &'a str,
            description: &'a str,
            source_branch: &'a str,
            target_branch: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            target_project_id: Option<i64>,
        }

        // The host project id is the one where we create the merge request in.
        // If there's a source project defined, it means we're dealing with a fork,
        // and hence should create the review on it.
        let host_project_id = if let Some(source_project_id) = &params.source_project_id {
            source_project_id
        } else {
            &params.project_id
        };

        let url = format!(
            "{}/projects/{}/merge_requests",
            self.base_url, host_project_id
        );

        // If there's a source project defined, and it's different than the
        // 'main' project id, it means we're handling the creation of a merge request
        // from a fork.
        // Fetch the target project numeric ID and pass it to the params.
        let target_project_id = if let Some(source_project_id) = &params.source_project_id
            && &params.project_id != source_project_id
        {
            let target_project_id = self
                .fetch_project(params.project_id.clone())
                .await
                .map(|project| project.id)
                .context("Failed to fetch target project information.")?;
            Some(target_project_id)
        } else {
            None
        };

        let title = update_draft_state_in_title(params.title, params.draft);

        let body = CreateMergeRequestBody {
            title: &title,
            description: params.body,
            source_branch: params.source_branch,
            target_branch: params.target_branch,
            target_project_id,
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

    pub async fn get_merge_request(
        &self,
        project_id: GitLabProjectId,
        mr_iid: i64,
    ) -> Result<MergeRequest> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, mr_iid
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get merge request: {}", response.status());
        }

        let mr: GitLabMergeRequest = response.json().await?;
        let mr = mr.into();
        match self.enrich_merge_request_source_project(mr).await {
            Ok(mr) => Ok(mr),
            Err(err) => {
                tracing::warn!(
                    error = ?err.source(),
                    project_id = %project_id,
                    mr_iid,
                    "Failed to enrich GitLab merge request source project"
                );
                Ok(err.into_inner())
            }
        }
    }

    /// Focused fetch returning just GitLab's `merge_status` and the
    /// MR comment count. Mirrors the corresponding GitHub endpoint so
    /// the UI only subscribes to these fields where it displays them.
    pub async fn get_merge_request_merge_status(
        &self,
        project_id: GitLabProjectId,
        mr_iid: i64,
    ) -> Result<MergeRequestMergeStatus> {
        #[derive(Debug, Deserialize)]
        struct MrMergeStatusResponse {
            #[serde(default)]
            merge_status: Option<String>,
            #[serde(default)]
            user_notes_count: i64,
        }

        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, mr_iid
        );
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            bail!("Failed to get MR merge status: {}", response.status());
        }
        let body: MrMergeStatusResponse = response.json().await?;
        let is_mergeable = matches!(body.merge_status.as_deref(), Some("can_be_merged"));
        Ok(MergeRequestMergeStatus {
            mergeable_state: body.merge_status,
            comments_count: body.user_notes_count,
            is_mergeable,
        })
    }

    pub async fn update_merge_request(
        &self,
        params: &UpdateMergeRequestParams<'_>,
    ) -> Result<MergeRequest> {
        #[derive(Serialize)]
        struct UpdateMergeRequestBody<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            title: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            target_branch: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            state_event: Option<&'a str>,
        }

        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, params.project_id, params.mr_iid
        );

        let title = if let Some(title) = params.title {
            let mr = self
                .get_merge_request(params.project_id.clone(), params.mr_iid)
                .await?;
            Some(update_draft_state_in_title(title, mr.draft))
        } else {
            None
        };

        let body = UpdateMergeRequestBody {
            title: title.as_deref(),
            description: params.description,
            target_branch: params.target_branch,
            state_event: params.state_event,
        };

        let response = self.client.put(&url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to update merge request: {status} - {error_text}");
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

        let body = MergeMergeRequestBody {
            squash: params.squash,
        };

        let response = self.client.put(&url).json(&body).send().await?;

        if !response.status().is_success() {
            bail!("Failed to merge merge request: {}", response.status());
        }

        Ok(())
    }

    pub async fn set_merge_request_draft_state(
        &self,
        params: &SetMergeRequestDraftStateParams,
    ) -> Result<()> {
        let project_id = params.project_id.clone();
        let mr = self
            .get_merge_request(project_id.clone(), params.mr_iid)
            .await?;
        let next_title = update_draft_state_in_title(&mr.title, params.is_draft);

        if next_title == mr.title {
            return Ok(());
        }

        #[derive(Serialize)]
        struct UpdateMergeRequestBody<'a> {
            title: &'a str,
        }

        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, project_id, params.mr_iid
        );

        let response = self
            .client
            .put(&url)
            .json(&UpdateMergeRequestBody { title: &next_title })
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to set merge request draft state: {status} - {error_text}");
        }

        Ok(())
    }

    pub async fn set_merge_request_auto_merge(
        &self,
        params: &SetMergeRequestAutoMergeParams,
    ) -> Result<()> {
        let project_id = params.project_id.clone();
        if params.enabled {
            #[derive(Serialize)]
            struct EnableAutoMergeBody {
                auto_merge: bool,
            }

            let url = format!(
                "{}/projects/{}/merge_requests/{}/merge",
                self.base_url, project_id, params.mr_iid
            );

            let response = self
                .client
                .put(&url)
                .json(&EnableAutoMergeBody { auto_merge: true })
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                bail!("Failed to enable merge request auto-merge: {status} - {error_text}");
            }

            return Ok(());
        }

        let url = format!(
            "{}/projects/{}/merge_requests/{}/cancel_merge_when_pipeline_succeeds",
            self.base_url, project_id, params.mr_iid
        );

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to disable merge request auto-merge: {status} - {error_text}");
        }

        Ok(())
    }

    pub async fn fetch_project(&self, project_id: GitLabProjectId) -> Result<GitLabProject> {
        self.fetch_project_by_path(project_id.to_string()).await
    }

    pub async fn fetch_project_by_numeric_id(&self, project_id: i64) -> Result<GitLabProject> {
        self.fetch_project_by_path(project_id.to_string()).await
    }

    async fn fetch_project_by_path(&self, project_id: String) -> Result<GitLabProject> {
        #[derive(Deserialize)]
        struct GitLabApiProject {
            id: i64,
            path_with_namespace: String,
            ssh_url_to_repo: String,
            http_url_to_repo: String,
            default_branch: Option<String>,
            #[serde(default)]
            forked_from_project: Option<GitLabApiProjectRef>,
            #[serde(default)]
            remove_source_branch_after_merge: Option<bool>,
            #[serde(default)]
            permissions: Option<GitLabApiPermissions>,
        }

        #[derive(Deserialize)]
        struct GitLabApiProjectRef {
            id: i64,
        }

        #[derive(Deserialize)]
        struct GitLabApiPermissions {
            #[serde(default)]
            project_access: Option<GitLabApiAccess>,
            #[serde(default)]
            group_access: Option<GitLabApiAccess>,
        }

        #[derive(Deserialize)]
        struct GitLabApiAccess {
            access_level: i64,
        }

        let url = format!("{}/projects/{}", self.base_url, project_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to fetch project: {status} - {error_text}");
        }

        let project: GitLabApiProject = response.json().await?;

        let access_level = project.permissions.and_then(|p| {
            let project_level = p.project_access.map(|a| a.access_level);
            let group_level = p.group_access.map(|a| a.access_level);
            match (project_level, group_level) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            }
        });

        Ok(GitLabProject {
            id: project.id,
            path_with_namespace: project.path_with_namespace,
            ssh_url_to_repo: project.ssh_url_to_repo,
            http_url_to_repo: project.http_url_to_repo,
            default_branch: project.default_branch,
            forked_from_project_id: project.forked_from_project.map(|fork| fork.id),
            remove_source_branch_after_merge: project.remove_source_branch_after_merge,
            access_level,
        })
    }

    async fn enrich_merge_request_source_project(
        &self,
        mut mr: MergeRequest,
    ) -> std::result::Result<MergeRequest, MergeRequestEnrichmentError> {
        let source_project_id = mr.source_project_id.unwrap_or(mr.project_id);
        let source_project = match self.fetch_project_by_numeric_id(source_project_id).await {
            Ok(source_project) => source_project,
            Err(source) => return Err(MergeRequestEnrichmentError { source, mr }),
        };

        mr.repository_ssh_url = Some(source_project.ssh_url_to_repo);
        mr.repository_https_url = Some(source_project.http_url_to_repo);
        mr.repo_owner = repo_owner_from_path_with_namespace(&source_project.path_with_namespace);
        mr.source_project_is_fork = source_project_differs_from_target(
            mr.source_project_id,
            mr.target_project_id,
            mr.project_id,
        );
        Ok(mr)
    }

    /// Fetch pipeline jobs for the latest commit on a given branch reference.
    ///
    /// Returns an empty vec if GitLab has no pipeline for the latest commit on the ref.
    pub async fn list_pipeline_jobs_for_ref(
        &self,
        project_id: GitLabProjectId,
        reference: &str,
    ) -> Result<Vec<GitLabPipelineJob>> {
        #[derive(Deserialize)]
        struct GitLabPipelineResponse {
            id: i64,
            status: String,
            web_url: Option<String>,
        }

        let url = format!("{}/projects/{}/pipelines/latest", self.base_url, project_id);
        let response = self
            .client
            .get(&url)
            .query(&[("ref", reference)])
            .send()
            .await
            .with_context(|| {
                format!("Failed to get latest GitLab pipeline for ref '{reference}'")
            })?;

        if !response.status().is_success() {
            let status = response.status();
            if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND
            {
                return Ok(Vec::new());
            }
            bail!("Failed to get latest pipeline for ref: {status}");
        }

        let pipeline: GitLabPipelineResponse = response
            .json()
            .await
            .with_context(|| format!("Failed to parse GitLab pipeline for ref '{reference}'"))?;

        let pipeline_web_url = pipeline.web_url;
        let pipeline_status = Some(pipeline.status);

        let jobs_url = format!(
            "{}/projects/{}/pipelines/{}/jobs",
            self.base_url, project_id, pipeline.id
        );
        let mut jobs = Vec::new();
        let mut next_page = Some("1".to_string());
        let mut seen_pages = HashSet::new();
        let mut pages_iterated = 0;

        while let Some(page) = next_page.take() {
            if pages_iterated >= MAX_PIPELINE_JOB_PAGES || !seen_pages.insert(page.clone()) {
                bail!(
                    "Stopped listing GitLab jobs for pipeline {} after unsafe pagination state",
                    pipeline.id
                );
            }
            pages_iterated += 1;

            let response = self
                .client
                .get(&jobs_url)
                .query(&[("per_page", "100"), ("page", page.as_str())])
                .send()
                .await
                .with_context(|| {
                    format!("Failed to list GitLab jobs for pipeline {}", pipeline.id)
                })?;

            if !response.status().is_success() {
                bail!("Failed to list jobs for pipeline: {}", response.status());
            }

            next_page = next_page_from_headers(response.headers());
            let mut page_jobs: Vec<GitLabPipelineJob> =
                response.json().await.with_context(|| {
                    format!("Failed to parse GitLab jobs for pipeline {}", pipeline.id)
                })?;
            if page_jobs.is_empty() {
                break;
            }
            jobs.append(&mut page_jobs);
        }

        let jobs = normalize_pipeline_jobs(
            jobs,
            pipeline_web_url,
            pipeline_status,
            &self.base_url,
            project_id,
        );
        Ok(jobs)
    }
}

fn next_page_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-next-page")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn normalize_pipeline_jobs(
    jobs: Vec<GitLabPipelineJob>,
    pipeline_web_url: Option<String>,
    pipeline_status: Option<String>,
    base_url: &str,
    project_id: GitLabProjectId,
) -> Vec<GitLabPipelineJob> {
    let web_base = base_url
        .strip_suffix("/api/v4")
        .unwrap_or(base_url)
        .trim_end_matches('/');
    let username = project_id.username();
    let project_name = project_id.project_name();

    jobs.into_iter()
        .map(|mut job| {
            if job.web_url.is_none() {
                job.web_url = pipeline_web_url
                    .clone()
                    .or_else(|| job.pipeline.web_url.clone())
                    .or_else(|| {
                        Some(format!(
                            "{}/{}/{}/-/pipelines/{}",
                            web_base, username, project_name, job.pipeline.id
                        ))
                    });
            }
            if job.pipeline.web_url.is_none() {
                job.pipeline.web_url = pipeline_web_url.clone().or_else(|| job.web_url.clone());
            }
            if job.pipeline.status.is_none() {
                job.pipeline.status = pipeline_status.clone();
            }
            job
        })
        .collect()
}

pub struct CreateMergeRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub source_branch: &'a str,
    pub target_branch: &'a str,
    pub project_id: GitLabProjectId,
    pub source_project_id: Option<GitLabProjectId>,
    pub draft: bool,
}

pub struct UpdateMergeRequestParams<'a> {
    pub project_id: GitLabProjectId,
    pub mr_iid: i64,
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub target_branch: Option<&'a str>,
    pub state_event: Option<&'a str>,
}

pub struct MergeMergeRequestParams {
    pub project_id: GitLabProjectId,
    pub mr_iid: i64,
    pub squash: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SetMergeRequestDraftStateParams {
    pub project_id: GitLabProjectId,
    pub mr_iid: i64,
    pub is_draft: bool,
}

pub struct SetMergeRequestAutoMergeParams {
    pub project_id: GitLabProjectId,
    pub mr_iid: i64,
    pub enabled: bool,
}

fn update_draft_state_in_title(title: &str, is_draft: bool) -> String {
    if is_draft {
        if has_draft_prefix(title) {
            title.to_owned()
        } else {
            format!("Draft: {title}")
        }
    } else {
        remove_draft_prefix(title).to_owned()
    }
}

fn has_draft_prefix(title: &str) -> bool {
    split_draft_prefix(title).is_some()
}

fn remove_draft_prefix(title: &str) -> &str {
    split_draft_prefix(title).unwrap_or(title)
}

fn split_draft_prefix(title: &str) -> Option<&str> {
    let title = title.trim_start();

    if let Some((prefix, rest)) = title.split_once(':') {
        let prefix = prefix.trim();
        if prefix.eq_ignore_ascii_case("draft") || prefix.eq_ignore_ascii_case("wip") {
            return Some(rest.trim_start());
        }
    }

    if let Some(bracketed) = title.strip_prefix('[')
        && let Some((prefix, rest)) = bracketed.split_once(']')
    {
        let prefix = prefix.trim();
        if prefix.eq_ignore_ascii_case("draft") || prefix.eq_ignore_ascii_case("wip") {
            return Some(rest.trim_start());
        }
    }

    None
}

/// Focused subset of GitLab's MR-get response covering only the
/// runtime-computed merge status. Mirrors `PullRequestMergeStatus`
/// on the GitHub side so `but_forge` can expose a forge-agnostic
/// `MergeStatus`.
#[derive(Debug, Clone, Serialize)]
pub struct MergeRequestMergeStatus {
    pub mergeable_state: Option<String>,
    pub comments_count: i64,
    /// Whether GitLab considers the MR mergeable. True for
    /// `can_be_merged`.
    pub is_mergeable: bool,
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
pub struct GitLabProject {
    pub id: i64,
    pub path_with_namespace: String,
    pub ssh_url_to_repo: String,
    pub http_url_to_repo: String,
    pub default_branch: Option<String>,
    pub forked_from_project_id: Option<i64>,
    pub remove_source_branch_after_merge: Option<bool>,
    /// Higher of project-level and group-level access for the caller.
    /// GitLab levels: 10=Guest, 20=Reporter, 30=Developer,
    /// 40=Maintainer, 50=Owner.
    pub access_level: Option<i64>,
}

/// GitLab CI job with the pipeline fields needed to build GitButler check status.
#[derive(Debug, Clone, Deserialize)]
pub struct GitLabPipelineJob {
    pub id: i64,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub allow_failure: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub web_url: Option<String>,
    pub pipeline: GitLabPipelineRef,
}

/// Minimal pipeline reference embedded in GitLab job responses.
#[derive(Debug, Clone, Deserialize)]
pub struct GitLabPipelineRef {
    pub id: i64,
    pub web_url: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
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
    pub integration_commit_shas: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub project_id: i64,
    pub source_project_id: Option<i64>,
    pub target_project_id: Option<i64>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub source_project_is_fork: bool,
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
    merge_commit_sha: Option<String>,
    squash_commit_sha: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    project_id: i64,
    source_project_id: Option<i64>,
    target_project_id: Option<i64>,
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
        let integration_commit_shas = [mr.merge_commit_sha, mr.squash_commit_sha]
            .into_iter()
            .flatten()
            .collect();

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
            integration_commit_shas,
            created_at: mr.created_at,
            updated_at: mr.updated_at,
            merged_at: mr.merged_at,
            closed_at: mr.closed_at,
            project_id: mr.project_id,
            source_project_id: mr.source_project_id,
            target_project_id: mr.target_project_id,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            source_project_is_fork: source_project_differs_from_target(
                mr.source_project_id,
                mr.target_project_id,
                mr.project_id,
            ),
            assignees,
            reviewers,
        }
    }
}

fn repo_owner_from_path_with_namespace(path_with_namespace: &str) -> Option<String> {
    path_with_namespace
        .rsplit_once('/')
        .map(|(owner, _repo)| owner.to_string())
}

fn source_project_differs_from_target(
    source_project_id: Option<i64>,
    target_project_id: Option<i64>,
    project_id: i64,
) -> bool {
    let source_project_id = source_project_id.unwrap_or(project_id);
    let target_project_id = target_project_id.unwrap_or(project_id);
    source_project_id != target_project_id
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::GitlabAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::GitlabAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_gitlab_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!(
            "No authenticated GitLab users found.\nRun 'but config forge auth' to authenticate with GitLab."
        );
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

#[cfg(test)]
mod tests {
    use super::{
        GitLabMergeRequest, GitLabPipelineJob, GitLabPipelineRef, MergeRequest,
        next_page_from_headers, normalize_pipeline_jobs, repo_owner_from_path_with_namespace,
        update_draft_state_in_title,
    };
    use reqwest::header::{HeaderMap, HeaderValue};

    fn job(
        id: i64,
        pipeline_id: i64,
        web_url: Option<&str>,
        pipeline_web_url: Option<&str>,
    ) -> GitLabPipelineJob {
        GitLabPipelineJob {
            id,
            name: format!("job-{id}"),
            status: "success".into(),
            allow_failure: false,
            started_at: None,
            finished_at: None,
            web_url: web_url.map(str::to_owned),
            pipeline: GitLabPipelineRef {
                id: pipeline_id,
                web_url: pipeline_web_url.map(str::to_owned),
                status: None,
            },
        }
    }

    #[test]
    fn reads_next_page_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-next-page", HeaderValue::from_static("2"));
        assert_eq!(next_page_from_headers(&headers), Some("2".to_string()));

        headers.insert("x-next-page", HeaderValue::from_static(""));
        assert_eq!(next_page_from_headers(&headers), None);
    }

    #[test]
    fn makes_title_draft() {
        assert_eq!(
            update_draft_state_in_title("Add API validation", true),
            "Draft: Add API validation"
        );
    }

    #[test]
    fn draft_state_noops_when_already_draft() {
        assert_eq!(
            update_draft_state_in_title("Draft: Add API validation", true),
            "Draft: Add API validation"
        );
        assert_eq!(
            update_draft_state_in_title("draft: Add API validation", true),
            "draft: Add API validation"
        );
        assert_eq!(
            update_draft_state_in_title("wip: Add API validation", true),
            "wip: Add API validation"
        );
        assert_eq!(
            update_draft_state_in_title("Wip: Add API validation", true),
            "Wip: Add API validation"
        );
        assert_eq!(
            update_draft_state_in_title("[Draft] Add API validation", true),
            "[Draft] Add API validation"
        );
        assert_eq!(
            update_draft_state_in_title("[wip] Add API validation", true),
            "[wip] Add API validation"
        );
    }

    #[test]
    fn removes_draft_prefix_for_ready_state() {
        assert_eq!(
            update_draft_state_in_title("Draft: Add API validation", false),
            "Add API validation"
        );
    }

    #[test]
    fn removes_wip_prefix_for_ready_state() {
        assert_eq!(
            update_draft_state_in_title("WIP: Add API validation", false),
            "Add API validation"
        );
    }

    #[test]
    fn removes_bracketed_draft_prefix_for_ready_state() {
        assert_eq!(
            update_draft_state_in_title("[Draft] Add API validation", false),
            "Add API validation"
        );
    }

    #[test]
    fn removes_bracketed_wip_prefix_for_ready_state() {
        assert_eq!(
            update_draft_state_in_title("[WIP] Add API validation", false),
            "Add API validation"
        );
    }

    #[test]
    fn update_title_preserves_existing_draft_state() {
        assert_eq!(
            update_draft_state_in_title("Rename API validation", true),
            "Draft: Rename API validation"
        );
        assert_eq!(
            update_draft_state_in_title("Draft: Rename API validation", false),
            "Rename API validation"
        );
    }

    #[test]
    fn normalize_pipeline_jobs_prefers_pipeline_url_and_backfills_job_urls() {
        let jobs = normalize_pipeline_jobs(
            vec![job(1, 123, None, None), job(2, 123, None, None)],
            Some("https://gitlab.example/pipelines/123".into()),
            Some("success".into()),
            "https://gitlab.example/api/v4",
            crate::GitLabProjectId::new("group", "repo"),
        );

        assert_eq!(
            jobs[0].web_url.as_deref(),
            Some("https://gitlab.example/pipelines/123")
        );
        assert_eq!(
            jobs[1].web_url.as_deref(),
            Some("https://gitlab.example/pipelines/123")
        );
        assert_eq!(
            jobs[0].pipeline.web_url.as_deref(),
            Some("https://gitlab.example/pipelines/123")
        );
        assert_eq!(jobs[0].pipeline.status.as_deref(), Some("success"));
    }

    #[test]
    fn normalize_pipeline_jobs_synthesizes_pipeline_url_when_missing_everywhere() {
        let jobs = normalize_pipeline_jobs(
            vec![job(1, 123, None, None)],
            None,
            None,
            "https://gitlab.example/api/v4",
            crate::GitLabProjectId::new("group", "repo"),
        );

        assert_eq!(
            jobs[0].web_url.as_deref(),
            Some("https://gitlab.example/group/repo/-/pipelines/123")
        );
    }

    #[test]
    fn associated_commit_mrs_preserve_head_sha_and_merge_state() {
        let mr: MergeRequest = GitLabMergeRequest {
            web_url: "https://gitlab.example/group/repo/-/merge_requests/7".into(),
            iid: 7,
            title: "Integrate feature".into(),
            description: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            sha: "1234567890abcdef1234567890abcdef12345678".into(),
            merge_commit_sha: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
            squash_commit_sha: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into()),
            created_at: None,
            updated_at: None,
            merged_at: Some("2026-06-24T12:00:00Z".into()),
            closed_at: Some("2026-06-24T12:00:00Z".into()),
            project_id: 1,
            source_project_id: Some(1),
            target_project_id: Some(1),
            assignees: vec![],
            reviewers: vec![],
        }
        .into();

        assert_eq!(
            mr.sha, "1234567890abcdef1234567890abcdef12345678",
            "associated-commit lookup must preserve the review head SHA for integration hints"
        );
        assert_eq!(
            mr.integration_commit_shas,
            vec![
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ],
            "GitLab review payload should preserve landing commits for cache-only integration matching"
        );
        assert_eq!(
            mr.target_branch, "main",
            "associated-commit lookup must preserve the target branch for filtering"
        );
        assert_eq!(
            mr.merged_at.as_deref(),
            Some("2026-06-24T12:00:00Z"),
            "associated-commit lookup must preserve merge state for filtering"
        );
    }

    #[test]
    fn repo_owner_comes_from_project_namespace() {
        assert_eq!(
            repo_owner_from_path_with_namespace("group/subgroup/repo").as_deref(),
            Some("group/subgroup"),
            "GitLab remote names should be based on the full source namespace"
        );
    }

    #[test]
    fn source_project_is_not_a_fork_when_mr_targets_its_own_project() {
        let mr: MergeRequest = GitLabMergeRequest {
            web_url: "https://gitlab.example/group/repo/-/merge_requests/8".into(),
            iid: 8,
            title: "Refine branch".into(),
            description: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            sha: "1234567890abcdef1234567890abcdef12345678".into(),
            merge_commit_sha: None,
            squash_commit_sha: None,
            created_at: None,
            updated_at: None,
            merged_at: None,
            closed_at: None,
            project_id: 7,
            source_project_id: Some(7),
            target_project_id: Some(7),
            assignees: vec![],
            reviewers: vec![],
        }
        .into();

        assert!(
            !mr.source_project_is_fork,
            "MR fork handling should stay off when source and target projects match"
        );
    }

    #[test]
    fn source_project_falls_back_to_mr_project_when_target_project_is_missing() {
        let mr: MergeRequest = GitLabMergeRequest {
            web_url: "https://gitlab.example/group/repo/-/merge_requests/9".into(),
            iid: 9,
            title: "Refine branch".into(),
            description: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            sha: "1234567890abcdef1234567890abcdef12345678".into(),
            merge_commit_sha: None,
            squash_commit_sha: None,
            created_at: None,
            updated_at: None,
            merged_at: None,
            closed_at: None,
            project_id: 7,
            source_project_id: Some(11),
            target_project_id: None,
            assignees: vec![],
            reviewers: vec![],
        }
        .into();

        assert!(
            mr.source_project_is_fork,
            "Missing target project IDs should compare against the MR project for fork handling"
        );
    }
}
