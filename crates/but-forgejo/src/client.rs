use anyhow::{Context as _, Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;

const FORGEJO_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Forgejo caps page sizes at the instance's `MAX_RESPONSE_ITEMS` (50 by default).
const PAGE_LIMIT: usize = 50;
/// Safety cap on pagination so a misbehaving server can't loop forever.
const MAX_PAGES: usize = 25;
/// The title prefix Forgejo recognizes as "work in progress" out of the box.
const DRAFT_PREFIX: &str = "WIP: ";

/// An HTTP error with a status code, returned when the API responds with a non-success status.
///
/// This can be downcasted from `anyhow::Error` to distinguish auth failures (401/403) from other errors.
#[derive(Debug, thiserror::Error)]
#[error("HTTP {status}")]
pub struct HttpStatusError {
    pub status: reqwest::StatusCode,
}

/// A client for the Forgejo REST API (`/api/v1`), authenticated with a personal access token.
pub struct ForgejoClient {
    client: reqwest::Client,
    base_url: String,
}

impl ForgejoClient {
    /// Build a client for the instance at `host` (see [`crate::normalize_host`]).
    pub fn new(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-forgejo-integration"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let mut auth_value = HeaderValue::from_str(&format!("token {}", access_token.0))?;
        auth_value.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(FORGEJO_REQUEST_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            base_url: format!("{}/api/v1", crate::normalize_host(host)),
        })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    ) -> Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No Forgejo access token found for account '{account_id}'.\nRe-connect it under Settings → Integrations."
            )
            .context(NOT_AUTHENTICATED))
        }
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        let response = self
            .client
            .get(format!("{}/user", self.base_url))
            .send()
            .await?;
        let user: ForgejoApiUser = json_or_status_error(response).await?;
        Ok(AuthenticatedUser {
            username: user.login,
            name: non_empty(user.full_name),
            email: non_empty(user.email),
            avatar_url: non_empty(user.avatar_url),
        })
    }

    /// The instance's web origin, e.g. `https://git.example.com`.
    fn web_origin(&self) -> &str {
        self.base_url
            .strip_suffix("/api/v1")
            .unwrap_or(&self.base_url)
    }

    fn repo_url(&self, owner: &str, repo: &str) -> String {
        format!(
            "{}/repos/{}/{}",
            self.base_url,
            urlencoding::encode(owner),
            urlencoding::encode(repo)
        )
    }

    /// Fetch every item of a paginated collection. Stops at an empty page or once
    /// `X-Total-Count` items have arrived, and errors at the `MAX_PAGES` cap
    /// rather than silently truncating.
    async fn get_paginated<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let limit = PAGE_LIMIT.to_string();
        let mut items = Vec::new();
        for page in 1..=MAX_PAGES {
            let page = page.to_string();
            let response = self
                .client
                .get(url)
                .query(query)
                .query(&[("limit", limit.as_str()), ("page", page.as_str())])
                .send()
                .await?;
            let total = total_count(response.headers());
            let mut page_items: Vec<T> = json_or_status_error(response).await?;
            if page_items.is_empty() {
                return Ok(items);
            }
            items.append(&mut page_items);
            if total.is_some_and(|total| items.len() >= total) {
                return Ok(items);
            }
        }
        bail!("Forgejo pagination exceeded the {MAX_PAGES}-page safety cap")
    }

    /// Fetch the first page of a collection, for "most recent" listings.
    async fn get_first_page<T: DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let limit = PAGE_LIMIT.to_string();
        let response = self
            .client
            .get(url)
            .query(query)
            .query(&[("limit", limit.as_str()), ("page", "1")])
            .send()
            .await?;
        json_or_status_error(response).await
    }

    pub async fn list_open_prs(&self, owner: &str, repo: &str) -> Result<Vec<ForgejoPullRequest>> {
        let url = format!("{}/pulls", self.repo_url(owner, repo));
        let prs: Vec<ForgejoApiPullRequest> = self
            .get_paginated(&url, &[("state", "open"), ("sort", "recentupdate")])
            .await?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    /// The most recently updated pull requests in any state that target
    /// `target_branch`. Forgejo can't filter by base branch, so one page of
    /// all pull requests is filtered here.
    pub async fn list_prs_for_target(
        &self,
        owner: &str,
        repo: &str,
        target_branch: &str,
    ) -> Result<Vec<ForgejoPullRequest>> {
        let url = format!("{}/pulls", self.repo_url(owner, repo));
        let prs: Vec<ForgejoApiPullRequest> = self
            .get_first_page(&url, &[("state", "all"), ("sort", "recentupdate")])
            .await?;
        Ok(prs
            .into_iter()
            .map(ForgejoPullRequest::from)
            .filter(|pr| pr.target_branch == target_branch)
            .collect())
    }

    /// One page of the most recently updated closed (merged or declined) pull
    /// requests: the fate sweep for the review cache.
    pub async fn list_recently_closed_prs(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<ForgejoPullRequest>> {
        let url = format!("{}/pulls", self.repo_url(owner, repo));
        let prs: Vec<ForgejoApiPullRequest> = self
            .get_first_page(&url, &[("state", "closed"), ("sort", "recentupdate")])
            .await?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
    ) -> Result<ForgejoPullRequest> {
        let url = format!("{}/pulls/{number}", self.repo_url(owner, repo));
        let response = self.client.get(&url).send().await?;
        let pr: ForgejoApiPullRequest = json_or_status_error(response).await?;
        Ok(pr.into())
    }

    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<ForgejoPullRequest> {
        #[derive(Serialize)]
        struct Body<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            base: &'a str,
        }

        let url = format!("{}/pulls", self.repo_url(params.owner, params.repo));
        let title = update_draft_state_in_title(params.title, params.draft);
        let body = Body {
            title: &title,
            body: params.body,
            head: params.head,
            base: params.base,
        };
        let response = self.client.post(&url).json(&body).send().await?;
        let pr: ForgejoApiPullRequest =
            json_or_detailed_error(response, "create pull request").await?;
        Ok(pr.into())
    }

    pub async fn update_pull_request(
        &self,
        params: &UpdatePullRequestParams<'_>,
    ) -> Result<ForgejoPullRequest> {
        // Renaming must not silently drop or add the draft prefix.
        let title = match params.title {
            Some(title) => {
                let pr = self
                    .get_pull_request(params.owner, params.repo, params.number)
                    .await?;
                Some(update_draft_state_in_title(title, pr.draft))
            }
            None => None,
        };
        self.edit_pull_request(
            params.owner,
            params.repo,
            params.number,
            &EditBody {
                title: title.as_deref(),
                body: params.body,
                base: params.base,
                state: params.state,
            },
        )
        .await
    }

    pub async fn set_pull_request_draft_state(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        is_draft: bool,
    ) -> Result<()> {
        let pr = self.get_pull_request(owner, repo, number).await?;
        let title = update_draft_state_in_title(&pr.title, is_draft);
        if title == pr.title {
            return Ok(());
        }
        let body = EditBody {
            title: Some(&title),
            ..EditBody::default()
        };
        self.edit_pull_request(owner, repo, number, &body).await?;
        Ok(())
    }

    async fn edit_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &EditBody<'_>,
    ) -> Result<ForgejoPullRequest> {
        let url = format!("{}/pulls/{number}", self.repo_url(owner, repo));
        let response = self.client.patch(&url).json(body).send().await?;
        let pr: ForgejoApiPullRequest =
            json_or_detailed_error(response, "update pull request").await?;
        Ok(pr.into())
    }

    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        style: MergeStyle,
    ) -> Result<()> {
        self.post_merge(owner, repo, number, &MergeBody::now(style))
            .await
    }

    /// Schedule the pull request to merge once its checks pass, or cancel that.
    pub async fn set_pull_request_auto_merge(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        enabled: bool,
    ) -> Result<()> {
        if enabled {
            return self
                .post_merge(owner, repo, number, &MergeBody::when_checks_succeed())
                .await;
        }
        let url = format!("{}/pulls/{number}/merge", self.repo_url(owner, repo));
        let response = self.client.delete(&url).send().await?;
        ensure_success(response, "cancel auto-merge").await
    }

    async fn post_merge(
        &self,
        owner: &str,
        repo: &str,
        number: i64,
        body: &MergeBody,
    ) -> Result<()> {
        let url = format!("{}/pulls/{number}/merge", self.repo_url(owner, repo));
        let response = self.client.post(&url).json(body).send().await?;
        ensure_success(response, "merge pull request").await
    }

    /// The latest status of every CI context on `reference` (branch or SHA).
    ///
    /// Forgejo Actions reports each job as a commit status named
    /// `<workflow> / <job>`, so this covers Actions and external CI alike.
    /// `None` means the reference doesn't resolve, e.g. a deleted branch.
    pub async fn list_statuses_for_ref(
        &self,
        owner: &str,
        repo: &str,
        reference: &str,
    ) -> Result<Option<Vec<ForgejoCommitStatus>>> {
        #[derive(Deserialize)]
        struct CombinedStatus {
            #[serde(default)]
            sha: String,
            #[serde(default, deserialize_with = "null_as_default")]
            statuses: Vec<ForgejoApiCommitStatus>,
            #[serde(default)]
            total_count: usize,
        }

        let url = format!(
            "{}/commits/{}/status",
            self.repo_url(owner, repo),
            urlencoding::encode(reference)
        );
        let limit = PAGE_LIMIT.to_string();
        let mut statuses = Vec::new();
        for page in 1..=MAX_PAGES {
            let page = page.to_string();
            let response = self
                .client
                .get(&url)
                .query(&[("limit", limit.as_str()), ("page", page.as_str())])
                .send()
                .await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let combined: CombinedStatus = json_or_status_error(response).await?;
            // An unknown ref comes back as 200 with an empty SHA, not a 404.
            if combined.sha.is_empty() {
                return Ok(None);
            }
            let page_is_empty = combined.statuses.is_empty();
            statuses.extend(
                combined
                    .statuses
                    .into_iter()
                    .map(|status| status.into_status(&combined.sha, self.web_origin())),
            );
            if page_is_empty || statuses.len() >= combined.total_count {
                return Ok(Some(statuses));
            }
        }
        bail!("Forgejo pagination exceeded the {MAX_PAGES}-page safety cap")
    }

    pub async fn fetch_repo(&self, owner: &str, repo: &str) -> Result<ForgejoRepo> {
        #[derive(Deserialize)]
        struct ApiRepo {
            #[serde(default)]
            fork: bool,
            #[serde(default)]
            permissions: Option<ForgejoPermissions>,
            #[serde(default)]
            default_delete_branch_after_merge: Option<bool>,
            #[serde(default)]
            private: Option<bool>,
        }

        let response = self.client.get(self.repo_url(owner, repo)).send().await?;
        let repo: ApiRepo = json_or_detailed_error(response, "fetch repository").await?;
        Ok(ForgejoRepo {
            fork: repo.fork,
            permissions: repo.permissions,
            delete_branch_after_merge: repo.default_delete_branch_after_merge,
            private: repo.private,
        })
    }
}

async fn json_or_status_error<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        return Err(HttpStatusError { status }.into());
    }
    Ok(response.json().await?)
}

/// Like [`json_or_status_error`], but keeps Forgejo's error message: for
/// mutations it usually says what to fix (e.g. "pull request already exists").
async fn json_or_detailed_error<T: DeserializeOwned>(
    response: reqwest::Response,
    action: &str,
) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(anyhow::Error::new(HttpStatusError { status })
            .context(format!("Failed to {action}: {status} - {detail}")));
    }
    response
        .json()
        .await
        .with_context(|| format!("Failed to parse the response to {action}"))
}

async fn ensure_success(response: reqwest::Response, action: &str) -> Result<()> {
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(anyhow::Error::new(HttpStatusError { status })
            .context(format!("Failed to {action}: {status} - {detail}")));
    }
    Ok(())
}

fn total_count(headers: &HeaderMap) -> Option<usize> {
    headers
        .get("x-total-count")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok())
}

/// Forgejo sends `null` for some empty lists, which `#[serde(default)]` alone rejects.
fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Forgejo returns missing optional strings as `""` rather than omitting them.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// Marks credential lookups that came up empty, so consumers can tell "the
/// user is not authenticated" apart from a failing forge and e.g. keep
/// serving cached data instead of surfacing an error.
pub(crate) const NOT_AUTHENTICATED: but_error::Context = but_error::Context::new_static(
    but_error::Code::ForgeNotAuthenticated,
    "Not authenticated with Forgejo. Connect your account under Settings → Integrations.",
);

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::ForgejoAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::ForgejoAccountIdentifier> {
    let known_accounts = crate::token::list_known_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        return Err(anyhow::anyhow!(
            "No authenticated Forgejo users found.\nConnect an account under Settings → Integrations."
        )
        .context(NOT_AUTHENTICATED));
    };
    let account = match preferred_account {
        Some(account) if known_accounts.contains(account) => account,
        Some(account) => {
            return Err(anyhow::anyhow!(
                "Preferred Forgejo account '{account}' has not authenticated yet.\nConnect it under Settings → Integrations, or choose another account."
            )
            .context(NOT_AUTHENTICATED));
        }
        None => default_account,
    };
    Ok(account.to_owned())
}

fn update_draft_state_in_title(title: &str, is_draft: bool) -> String {
    match (is_draft, split_draft_prefix(title)) {
        (true, Some(_)) => title.to_owned(),
        (true, None) => format!("{DRAFT_PREFIX}{title}"),
        (false, Some(rest)) => rest.to_owned(),
        (false, None) => title.to_owned(),
    }
}

/// The title without its `WIP:`/`[WIP]` (or `Draft:`/`[Draft]`) prefix, if it has one.
/// Forgejo compares these prefixes case-insensitively.
fn split_draft_prefix(title: &str) -> Option<&str> {
    let title = title.trim_start();
    let is_draft_word = |word: &str| {
        let word = word.trim();
        word.eq_ignore_ascii_case("wip") || word.eq_ignore_ascii_case("draft")
    };
    if let Some((prefix, rest)) = title.split_once(':')
        && is_draft_word(prefix)
    {
        return Some(rest.trim_start());
    }
    if let Some((prefix, rest)) = title.strip_prefix('[').and_then(|t| t.split_once(']'))
        && is_draft_word(prefix)
    {
        return Some(rest.trim_start());
    }
    None
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ForgejoApiUser {
    id: i64,
    login: String,
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

/// A Forgejo user mapped to the shape `but_forge` expects for review participants.
#[derive(Debug, Clone)]
pub struct ForgejoUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<ForgejoApiUser> for ForgejoUser {
    fn from(user: ForgejoApiUser) -> Self {
        ForgejoUser {
            id: user.id,
            login: user.login,
            name: non_empty(user.full_name),
            email: non_empty(user.email),
            avatar_url: non_empty(user.avatar_url),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgejoLabel {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A Forgejo pull request, normalised to the fields `but_forge` needs.
#[derive(Debug, Clone)]
pub struct ForgejoPullRequest {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<ForgejoUser>,
    pub labels: Vec<ForgejoLabel>,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    /// The head commit. Empty when Forgejo reported none (e.g. a deleted head branch).
    pub sha: String,
    pub merge_commit_sha: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub merged_at: Option<String>,
    /// Only set for pull requests closed without merging.
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    /// Owner of the head repository; the fork owner for fork pull requests.
    pub repo_owner: Option<String>,
    pub head_repo_is_fork: bool,
    /// HTTPS clone URL of the repository the pull request targets.
    pub base_repository_https_url: Option<String>,
    pub requested_reviewers: Vec<ForgejoUser>,
    pub comments: i64,
    /// Whether Forgejo considers the pull request mergeable (no conflicts).
    pub mergeable: bool,
}

impl ForgejoPullRequest {
    pub fn is_open(&self) -> bool {
        self.merged_at.is_none() && self.closed_at.is_none()
    }
}

#[derive(Debug, Deserialize)]
struct ForgejoApiPullRequest {
    number: i64,
    #[serde(default)]
    html_url: String,
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    user: Option<ForgejoApiUser>,
    #[serde(default, deserialize_with = "null_as_default")]
    labels: Vec<ForgejoLabel>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    mergeable: bool,
    #[serde(default)]
    comments: i64,
    #[serde(default)]
    head: Option<ForgejoApiBranchInfo>,
    #[serde(default)]
    base: Option<ForgejoApiBranchInfo>,
    #[serde(default)]
    merge_commit_sha: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    merged_at: Option<String>,
    #[serde(default)]
    closed_at: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    requested_reviewers: Vec<ForgejoApiUser>,
}

#[derive(Debug, Deserialize)]
struct ForgejoApiBranchInfo {
    #[serde(default, rename = "ref")]
    reference: String,
    #[serde(default)]
    sha: String,
    #[serde(default)]
    repo_id: i64,
    #[serde(default)]
    repo: Option<ForgejoApiRepoRef>,
}

#[derive(Debug, Deserialize)]
struct ForgejoApiRepoRef {
    #[serde(default)]
    ssh_url: Option<String>,
    #[serde(default)]
    clone_url: Option<String>,
    #[serde(default)]
    owner: Option<ForgejoApiUser>,
}

impl From<ForgejoApiPullRequest> for ForgejoPullRequest {
    fn from(pr: ForgejoApiPullRequest) -> Self {
        let head_repo_id = pr.head.as_ref().map(|head| head.repo_id);
        let base_repo_id = pr.base.as_ref().map(|base| base.repo_id);
        let head_repo_is_fork = matches!(
            (head_repo_id, base_repo_id),
            (Some(head), Some(base)) if head != 0 && base != 0 && head != base
        );
        let base_repository_https_url = pr
            .base
            .as_ref()
            .and_then(|base| base.repo.as_ref())
            .and_then(|repo| non_empty(repo.clone_url.clone()));
        let (source_branch, sha, head_repo) = match pr.head {
            Some(head) => (head.reference, head.sha, head.repo),
            None => (String::new(), String::new(), None),
        };
        let (repository_ssh_url, repository_https_url, repo_owner) = match head_repo {
            Some(repo) => (
                non_empty(repo.ssh_url),
                non_empty(repo.clone_url),
                repo.owner.map(|owner| owner.login),
            ),
            None => (None, None, None),
        };
        // Forgejo closes the underlying issue on merge, so a merged pull request
        // also carries `closed_at`; only a declined one is "closed" here.
        let closed_at = if pr.merged { None } else { pr.closed_at };
        ForgejoPullRequest {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: non_empty(pr.body),
            author: pr.user.map(Into::into),
            labels: pr.labels,
            draft: pr.draft,
            source_branch,
            target_branch: pr.base.map(|base| base.reference).unwrap_or_default(),
            sha,
            merge_commit_sha: non_empty(pr.merge_commit_sha),
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            merged_at: pr.merged_at.filter(|_| pr.merged),
            closed_at,
            repository_ssh_url,
            repository_https_url,
            repo_owner,
            head_repo_is_fork,
            base_repository_https_url,
            requested_reviewers: pr.requested_reviewers.into_iter().map(Into::into).collect(),
            comments: pr.comments,
            mergeable: pr.mergeable,
        }
    }
}

/// One CI context's latest status on a commit.
#[derive(Debug, Clone)]
pub struct ForgejoCommitStatus {
    pub id: i64,
    /// The CI context, e.g. `CI / checks (push)` for a Forgejo Actions job.
    pub context: String,
    pub description: Option<String>,
    /// One of `pending`, `success`, `error`, `failure`, `warning`.
    pub state: String,
    pub target_url: Option<String>,
    pub sha: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForgejoApiCommitStatus {
    id: i64,
    #[serde(default)]
    context: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    target_url: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl ForgejoApiCommitStatus {
    /// Forgejo Actions reports instance-relative target URLs; `web_origin`
    /// makes them absolute so they open in a browser.
    fn into_status(self, sha: &str, web_origin: &str) -> ForgejoCommitStatus {
        let target_url = non_empty(self.target_url).map(|url| {
            if url.starts_with('/') {
                format!("{web_origin}{url}")
            } else {
                url
            }
        });
        ForgejoCommitStatus {
            id: self.id,
            context: self.context,
            description: non_empty(self.description),
            state: self.status,
            target_url,
            sha: sha.to_owned(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Repository metadata used to populate `but_forge`'s `RepoInfo`.
#[derive(Debug, Clone)]
pub struct ForgejoRepo {
    pub fork: bool,
    pub permissions: Option<ForgejoPermissions>,
    pub delete_branch_after_merge: Option<bool>,
    pub private: Option<bool>,
}

/// The authenticated user's permissions on a repository.
#[derive(Debug, Clone, Deserialize)]
pub struct ForgejoPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub pull: bool,
}

pub struct CreatePullRequestParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    /// The head branch; `fork-owner:branch` when opening from a fork.
    pub head: &'a str,
    pub base: &'a str,
    pub draft: bool,
}

pub struct UpdatePullRequestParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub number: i64,
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
    pub base: Option<&'a str>,
    /// `open` or `closed`.
    pub state: Option<&'a str>,
}

/// Forgejo merge styles, as accepted by the merge endpoint's `Do` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStyle {
    Merge,
    Squash,
    Rebase,
}

impl MergeStyle {
    fn as_str(self) -> &'static str {
        match self {
            MergeStyle::Merge => "merge",
            MergeStyle::Squash => "squash",
            MergeStyle::Rebase => "rebase",
        }
    }
}

#[derive(Serialize, Default)]
struct EditBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<&'a str>,
}

#[derive(Serialize)]
struct MergeBody {
    #[serde(rename = "Do")]
    style: &'static str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    merge_when_checks_succeed: bool,
}

impl MergeBody {
    fn now(style: MergeStyle) -> Self {
        MergeBody {
            style: style.as_str(),
            merge_when_checks_succeed: false,
        }
    }

    // ponytail: auto-merge always uses a merge commit; read the repo's
    // `default_merge_style` if someone needs squash/rebase auto-merges.
    fn when_checks_succeed() -> Self {
        MergeBody {
            style: MergeStyle::Merge.as_str(),
            merge_when_checks_succeed: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_titles_use_the_prefix_forgejo_recognizes() {
        assert_eq!(update_draft_state_in_title("Add API", true), "WIP: Add API");
        for draft in [
            "WIP: Add API",
            "wip: Add API",
            "[WIP] Add API",
            "Draft: Add API",
        ] {
            assert_eq!(
                update_draft_state_in_title(draft, true),
                draft,
                "{draft:?} is already a draft"
            );
            assert_eq!(
                update_draft_state_in_title(draft, false),
                "Add API",
                "{draft:?} loses its prefix when marked ready"
            );
        }
        assert_eq!(
            update_draft_state_in_title("Fix: typo", false),
            "Fix: typo",
            "a colon alone is not a draft prefix"
        );
    }

    #[test]
    fn merge_bodies_use_forgejo_field_names() {
        assert_eq!(
            serde_json::to_value(MergeBody::now(MergeStyle::Squash)).unwrap(),
            serde_json::json!({"Do": "squash"}),
            "Forgejo requires `Do`; other forges' `merge_method` is rejected"
        );
        assert_eq!(
            serde_json::to_value(MergeBody::when_checks_succeed()).unwrap(),
            serde_json::json!({"Do": "merge", "merge_when_checks_succeed": true})
        );
    }

    fn pr_json(merged: bool, closed_at: Option<&str>, head_repo_id: i64) -> String {
        serde_json::json!({
            "number": 7,
            "html_url": "https://git.example.com/alice/repo/pulls/7",
            "title": "WIP: Add feature",
            "body": "",
            "user": {"id": 1, "login": "alice", "full_name": "Alice", "email": "", "avatar_url": "https://git.example.com/avatar/1"},
            "labels": [{"name": "bug", "color": "ee0701"}],
            "draft": true,
            "merged": merged,
            "mergeable": true,
            "comments": 3,
            "head": {"ref": "feature", "sha": "deadbeef", "repo_id": head_repo_id,
                     "repo": {"ssh_url": "git@git.example.com:bob/repo.git", "clone_url": "https://git.example.com/bob/repo.git", "owner": {"id": 2, "login": "bob"}}},
            "base": {"ref": "main", "sha": "cafef00d", "repo_id": 10,
                     "repo": {"ssh_url": "git@git.example.com:alice/repo.git", "clone_url": "https://git.example.com/alice/repo.git", "owner": {"id": 1, "login": "alice"}}},
            "merge_commit_sha": if merged { serde_json::json!("abc123") } else { serde_json::Value::Null },
            "merged_at": if merged { serde_json::json!("2026-09-01T00:00:00Z") } else { serde_json::Value::Null },
            "closed_at": closed_at,
            "requested_reviewers": [{"id": 3, "login": "carol"}]
        })
        .to_string()
    }

    fn parse(json: &str) -> ForgejoPullRequest {
        serde_json::from_str::<ForgejoApiPullRequest>(json)
            .unwrap()
            .into()
    }

    #[test]
    fn parses_an_open_fork_pull_request() {
        let pr = parse(&pr_json(false, None, 20));
        assert!(pr.is_open());
        assert!(pr.draft);
        assert_eq!(pr.source_branch, "feature");
        assert_eq!(pr.target_branch, "main");
        assert_eq!(pr.sha, "deadbeef");
        assert_eq!(pr.body, None, "an empty body is no body");
        assert_eq!(pr.author.unwrap().email, None, "a hidden email is no email");
        assert!(
            pr.head_repo_is_fork,
            "head and base live in different repos"
        );
        assert_eq!(pr.repo_owner.as_deref(), Some("bob"));
        assert_eq!(
            pr.base_repository_https_url.as_deref(),
            Some("https://git.example.com/alice/repo.git"),
            "the base URL is the target repo, not the fork"
        );
        assert_eq!(pr.requested_reviewers[0].login, "carol");
        assert_eq!((pr.comments, pr.mergeable), (3, true));
    }

    #[test]
    fn merged_pull_requests_are_not_also_closed() {
        let pr = parse(&pr_json(true, Some("2026-09-01T00:00:00Z"), 10));
        assert_eq!(pr.merged_at.as_deref(), Some("2026-09-01T00:00:00Z"));
        assert_eq!(
            pr.closed_at, None,
            "Forgejo also closes merged pull requests; that isn't a decline"
        );
        assert_eq!(pr.merge_commit_sha.as_deref(), Some("abc123"));
        assert!(!pr.head_repo_is_fork);
    }

    #[test]
    fn declined_pull_requests_are_closed() {
        let pr = parse(&pr_json(false, Some("2026-09-02T00:00:00Z"), 10));
        assert!(!pr.is_open());
        assert_eq!(pr.closed_at.as_deref(), Some("2026-09-02T00:00:00Z"));
        assert_eq!(pr.merged_at, None);
    }
}
