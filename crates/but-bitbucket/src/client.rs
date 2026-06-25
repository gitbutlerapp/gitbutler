use anyhow::{Result, bail};
use base64::Engine as _;
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;

const BITBUCKET_API_BASE_URL: &str = "https://api.bitbucket.org/2.0";
const BITBUCKET_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Safety cap on pagination so a malformed `next` cursor can't loop forever.
const MAX_PAGES: usize = 25;

/// An HTTP error with a status code, returned when the API responds with a non-success status.
///
/// This can be downcasted from `anyhow::Error` to distinguish auth failures (401/403) from other errors.
#[derive(Debug, thiserror::Error)]
#[error("HTTP {status}")]
pub struct HttpStatusError {
    pub status: reqwest::StatusCode,
}

pub struct BitbucketClient {
    pub(crate) client: reqwest::Client,
    pub(crate) base_url: String,
}

impl BitbucketClient {
    /// Build a client authenticating with an Atlassian API token over HTTP Basic.
    ///
    /// Bitbucket Cloud API tokens use the Atlassian account email as the Basic-auth
    /// username and the token as the password.
    pub fn new(email: &str, access_token: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-bitbucket-integration"),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let basic =
            base64::engine::general_purpose::STANDARD.encode(format!("{email}:{}", access_token.0));
        let mut auth_value = HeaderValue::from_str(&format!("Basic {basic}"))?;
        auth_value.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(BITBUCKET_REQUEST_TIMEOUT)
            .build()?;

        Ok(Self {
            client,
            base_url: BITBUCKET_API_BASE_URL.to_string(),
        })
    }

    pub fn from_storage(
        storage: &but_forge_storage::Controller,
        preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    ) -> Result<Self> {
        let account_id = resolve_account(preferred_account, storage)?;
        if let Some(access_token) = crate::token::get_bb_access_token(&account_id, storage)? {
            account_id.client(&access_token)
        } else {
            Err(anyhow::anyhow!(
                "No Bitbucket access token found for account '{account_id}'.\nRun 'but config forge auth' to re-authenticate."
            ))
        }
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        let url = format!("{}/user", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(HttpStatusError {
                status: response.status(),
            }
            .into());
        }

        let user: BitbucketApiUser = response.json().await?;
        Ok(AuthenticatedUser {
            username: user.username(),
            avatar_url: user.avatar_url(),
            name: user.display_name,
            account_id: user.account_id,
        })
    }

    /// Fetch every `values` entry across a paginated Bitbucket collection,
    /// following the `next` cursor URL until exhausted. Errors out if the
    /// `MAX_PAGES` safety cap is hit rather than silently truncating the result.
    async fn get_paginated<T: DeserializeOwned>(&self, initial_url: String) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut next = Some(initial_url);
        let mut pages = 0;

        while let Some(url) = next.take() {
            if pages >= MAX_PAGES {
                bail!("Bitbucket pagination exceeded the {MAX_PAGES}-page safety cap");
            }
            pages += 1;

            let response = self.client.get(&url).send().await?;
            if !response.status().is_success() {
                bail!("Bitbucket request failed: {}", response.status());
            }
            let page: Paginated<T> = response.json().await?;
            items.extend(page.values);
            next = page.next;
        }

        Ok(items)
    }

    /// Fetch a single page of a Bitbucket collection without following the
    /// `next` cursor. Used for "most recent" listings where the caller sorts
    /// server-side and only needs the head of the result set.
    async fn get_first_page<T: DeserializeOwned>(&self, url: String) -> Result<Vec<T>> {
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            bail!("Bitbucket request failed: {}", response.status());
        }
        let page: Paginated<T> = response.json().await?;
        Ok(page.values)
    }

    pub async fn list_open_prs(
        &self,
        workspace: &str,
        repo_slug: &str,
    ) -> Result<Vec<BitbucketPullRequest>> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests?state=OPEN&pagelen=50",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
        );
        let prs: Vec<BitbucketApiPullRequest> = self.get_paginated(url).await?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    pub async fn list_prs_for_target(
        &self,
        workspace: &str,
        repo_slug: &str,
        target_branch: &str,
    ) -> Result<Vec<BitbucketPullRequest>> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests?pagelen=50&sort=-updated_on&q={}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            urlencoding::encode(&list_for_target_query(target_branch)),
        );
        let prs: Vec<BitbucketApiPullRequest> = self.get_first_page(url).await?;
        Ok(prs.into_iter().map(Into::into).collect())
    }

    pub async fn get_pull_request(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: i64,
    ) -> Result<BitbucketPullRequest> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests/{}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            id,
        );
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(HttpStatusError {
                status: response.status(),
            }
            .into());
        }
        let pr: BitbucketApiPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<BitbucketPullRequest> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests",
            self.base_url,
            urlencoding::encode(params.workspace),
            urlencoding::encode(params.repo_slug),
        );

        let source = SourceBody {
            branch: BranchBody {
                name: params.source_branch,
            },
            // A source repository is only required when opening from a fork.
            repository: params
                .source_repo_full_name
                .map(|full_name| RepositoryBody { full_name }),
        };
        let body = CreatePullRequestBody {
            title: params.title,
            description: params.body,
            draft: params.draft,
            close_source_branch: true,
            source,
            destination: DestinationBody {
                branch: BranchBody {
                    name: params.target_branch,
                },
            },
        };

        let response = self.client.post(&url).json(&body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to create pull request: {status} - {error_text}");
        }
        let pr: BitbucketApiPullRequest = response.json().await?;
        Ok(pr.into())
    }

    /// Fetch the fields needed to safely PUT a pull request. Bitbucket's
    /// `PUT /pullrequests/{id}` is a full replace - any field omitted from the
    /// body is reset (reviewers dropped, draft cleared, close-source-branch
    /// reset) - so every update must re-send the fields it isn't changing.
    async fn fetch_pr_edit_context(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: i64,
    ) -> Result<PrEditContext> {
        #[derive(Deserialize)]
        struct Raw {
            title: String,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            draft: bool,
            #[serde(default)]
            close_source_branch: bool,
            #[serde(default)]
            destination: Option<BitbucketEndpoint>,
            #[serde(default)]
            reviewers: Vec<ReviewerRef>,
        }
        #[derive(Deserialize)]
        struct ReviewerRef {
            #[serde(default)]
            account_id: Option<String>,
        }

        let url = format!(
            "{}/repositories/{}/{}/pullrequests/{}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            id,
        );
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(HttpStatusError {
                status: response.status(),
            }
            .into());
        }
        let raw: Raw = response.json().await?;
        Ok(PrEditContext {
            title: raw.title,
            description: raw.description,
            draft: raw.draft,
            close_source_branch: raw.close_source_branch,
            destination_branch: raw.destination.and_then(|d| d.branch).and_then(|b| b.name),
            reviewer_account_ids: raw
                .reviewers
                .into_iter()
                .filter_map(|r| r.account_id)
                .collect(),
        })
    }

    /// Issue a full PUT that preserves the PR's current reviewers, draft state
    /// and close-source-branch flag, overriding only the provided fields.
    async fn send_pr_update(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: i64,
        body: &UpdatePullRequestBody<'_>,
    ) -> Result<BitbucketPullRequest> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests/{}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            id,
        );
        let response = self.client.put(&url).json(body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to update pull request: {status} - {error_text}");
        }
        let pr: BitbucketApiPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn update_pull_request(
        &self,
        params: &UpdatePullRequestParams<'_>,
    ) -> Result<BitbucketPullRequest> {
        let ctx = self
            .fetch_pr_edit_context(params.workspace, params.repo_slug, params.id)
            .await?;
        let body = build_update_body(
            &ctx,
            params.title,
            params.description,
            params.target_branch,
            None,
        );
        self.send_pr_update(params.workspace, params.repo_slug, params.id, &body)
            .await
    }

    pub async fn set_pull_request_draft_state(
        &self,
        params: &SetPullRequestDraftStateParams<'_>,
    ) -> Result<()> {
        let ctx = self
            .fetch_pr_edit_context(params.workspace, params.repo_slug, params.id)
            .await?;
        if ctx.draft == params.is_draft {
            return Ok(());
        }
        let body = build_update_body(&ctx, None, None, None, Some(params.is_draft));
        self.send_pr_update(params.workspace, params.repo_slug, params.id, &body)
            .await?;
        Ok(())
    }

    pub async fn merge_pull_request(&self, params: &MergePullRequestParams<'_>) -> Result<()> {
        let body = MergePullRequestBody {
            merge_strategy: params.strategy.as_str(),
        };
        let url = format!(
            "{}/repositories/{}/{}/pullrequests/{}/merge",
            self.base_url,
            urlencoding::encode(params.workspace),
            urlencoding::encode(params.repo_slug),
            params.id,
        );
        let response = self.client.post(&url).json(&body).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to merge pull request: {status} - {error_text}");
        }
        Ok(())
    }

    /// Decline (close) a pull request.
    pub async fn decline_pull_request(
        &self,
        workspace: &str,
        repo_slug: &str,
        id: i64,
    ) -> Result<()> {
        let url = format!(
            "{}/repositories/{}/{}/pullrequests/{}/decline",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            id,
        );
        let response = self.client.post(&url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to decline pull request: {status} - {error_text}");
        }
        Ok(())
    }

    pub async fn fetch_repo(&self, workspace: &str, repo_slug: &str) -> Result<BitbucketRepo> {
        let url = format!(
            "{}/repositories/{}/{}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
        );
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Failed to fetch repository: {status} - {error_text}");
        }
        let repo: BitbucketApiRepository = response.json().await?;

        // The caller's permission level lives behind a separate endpoint.
        let permission = self
            .fetch_repo_permission(workspace, repo_slug)
            .await
            .ok()
            .flatten();

        Ok(BitbucketRepo {
            is_fork: repo.parent.is_some(),
            permission,
        })
    }

    /// Resolve the authenticated user's permission (`admin`/`write`/`read`) on a repo.
    async fn fetch_repo_permission(
        &self,
        workspace: &str,
        repo_slug: &str,
    ) -> Result<Option<String>> {
        let full_name = escape_bbql(&format!("{workspace}/{repo_slug}"));
        let query = format!("repository.full_name = \"{full_name}\"");
        let url = format!(
            "{}/user/permissions/repositories?q={}",
            self.base_url,
            urlencoding::encode(&query),
        );
        let mut perms: Vec<RepoPermissionEntry> = self.get_paginated(url).await?;
        Ok(perms.pop().and_then(|p| p.permission))
    }

    /// List commit build statuses for a git reference.
    ///
    /// Bitbucket's statuses endpoint is keyed by commit hash, so a branch name is
    /// first resolved to its head commit; anything else is treated as a commit
    /// revspec directly.
    pub async fn list_checks_for_ref(
        &self,
        workspace: &str,
        repo_slug: &str,
        reference: &str,
    ) -> Result<Vec<BitbucketBuildStatus>> {
        let commit = self
            .resolve_commit_hash(workspace, repo_slug, reference)
            .await?;
        let url = format!(
            "{}/repositories/{}/{}/commit/{}/statuses?pagelen=100",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            urlencoding::encode(&commit),
        );
        let statuses: Vec<BitbucketApiBuildStatus> = self.get_paginated(url).await?;
        Ok(statuses
            .into_iter()
            .map(|s| BitbucketBuildStatus::from_api(s, &commit))
            .collect())
    }

    /// Resolve a branch name to its head commit hash, falling back to treating
    /// `reference` as a commit revspec when it isn't a branch.
    async fn resolve_commit_hash(
        &self,
        workspace: &str,
        repo_slug: &str,
        reference: &str,
    ) -> Result<String> {
        if is_full_commit_hash(reference) {
            return Ok(reference.to_string());
        }
        let url = format!(
            "{}/repositories/{}/{}/refs/branches/{}",
            self.base_url,
            urlencoding::encode(workspace),
            urlencoding::encode(repo_slug),
            encode_branch_path(reference),
        );
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        if status.is_success() {
            #[derive(Deserialize)]
            struct Branch {
                target: Target,
            }
            #[derive(Deserialize)]
            struct Target {
                hash: String,
            }
            let branch: Branch = response.json().await?;
            return Ok(branch.target.hash);
        }
        // 404 means it isn't a branch — treat the reference as a commit revspec.
        // Any other status (auth failure, server error) is a real error to surface
        // rather than silently returning empty checks.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(reference.to_string());
        }
        Err(HttpStatusError { status }.into())
    }
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::BitbucketAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::BitbucketAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_bitbucket_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!(
            "No authenticated Bitbucket users found.\nRun 'but config forge auth' to authenticate with Bitbucket."
        );
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred Bitbucket account '{account}' has not authenticated yet.\nRun 'but config forge auth' to authenticate, or choose another account."
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}

/// Whether `reference` is a full 40-character SHA-1 commit hash, in which case it
/// can be used against the statuses endpoint directly without a branch lookup.
fn is_full_commit_hash(reference: &str) -> bool {
    reference.len() == 40 && reference.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Percent-encode a git ref for use as the `refs/branches/{name}` path, encoding
/// each path segment individually so `/` stays a literal separator (encoding the
/// whole ref would turn `feature/x` into `feature%2Fx`, which Bitbucket 404s).
fn encode_branch_path(reference: &str) -> String {
    reference
        .split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Escape a value for embedding inside a double-quoted Bitbucket query-language
/// string, so a branch name with quotes/backslashes can't break or inject into
/// the query.
fn escape_bbql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// BBQL predicate matching pull requests in any state targeting `target_branch`.
/// States are folded into `q` (not separate `state=` params) so the predicate is
/// unambiguous; the branch name is escaped.
fn list_for_target_query(target_branch: &str) -> String {
    format!(
        "(state = \"OPEN\" OR state = \"MERGED\" OR state = \"DECLINED\" OR state = \"SUPERSEDED\") AND destination.branch.name = \"{}\"",
        escape_bbql(target_branch)
    )
}

/// Build a full PUT body for a pull-request update, preserving the PR's current
/// reviewers / draft / close-source-branch flag and overriding only the provided
/// fields. Bitbucket's PUT is a full replace, so omitting these would clear them.
fn build_update_body<'a>(
    ctx: &'a PrEditContext,
    title: Option<&'a str>,
    description: Option<&'a str>,
    target_branch: Option<&'a str>,
    draft: Option<bool>,
) -> UpdatePullRequestBody<'a> {
    UpdatePullRequestBody {
        title: title.unwrap_or(&ctx.title),
        description: description.or(ctx.description.as_deref()).unwrap_or(""),
        destination: target_branch
            .or(ctx.destination_branch.as_deref())
            .map(|name| DestinationBody {
                branch: BranchBody { name },
            }),
        draft: draft.unwrap_or(ctx.draft),
        close_source_branch: ctx.close_source_branch,
        reviewers: ctx
            .reviewer_account_ids
            .iter()
            .map(|account_id| ReviewerBody { account_id })
            .collect(),
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub account_id: Option<String>,
}

/// A Bitbucket user as returned by the API (`/user`, PR authors, reviewers).
///
/// Bitbucket has deprecated the privacy-sensitive `username` field in many
/// responses, so we fall back to `nickname` / `account_id` for a stable handle.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BitbucketApiUser {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub links: Option<BitbucketLinks>,
}

impl BitbucketApiUser {
    pub(crate) fn username(&self) -> String {
        self.username
            .clone()
            .or_else(|| self.nickname.clone())
            .or_else(|| self.display_name.clone())
            .or_else(|| self.account_id.clone())
            .unwrap_or_default()
    }

    pub(crate) fn avatar_url(&self) -> Option<String> {
        self.links
            .as_ref()
            .and_then(|l| l.avatar.as_ref())
            .and_then(|a| a.href.clone())
    }

    pub(crate) fn is_bot(&self) -> bool {
        self.kind.as_deref() == Some("app_user")
    }
}

/// Bitbucket embeds resource URLs under a `links` object.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BitbucketLinks {
    #[serde(default)]
    pub html: Option<BitbucketLink>,
    #[serde(default)]
    pub avatar: Option<BitbucketLink>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BitbucketLink {
    #[serde(default)]
    pub href: Option<String>,
}

/// A Bitbucket user mapped to the shape `but_forge` expects for review participants.
#[derive(Debug)]
pub struct BitbucketUser {
    /// Bitbucket users have no numeric id; this is a stable hash of the account
    /// id (or handle) so consumers that key by id don't collapse distinct users.
    pub id: i64,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub is_bot: bool,
}

impl From<BitbucketApiUser> for BitbucketUser {
    fn from(user: BitbucketApiUser) -> Self {
        let is_bot = user.is_bot();
        let username = user.username();
        let avatar_url = user.avatar_url();
        let id_seed = user.account_id.clone().unwrap_or_else(|| username.clone());
        BitbucketUser {
            id: crate::stable_id_hash(&id_seed),
            username,
            name: user.display_name,
            email: None,
            avatar_url,
            is_bot,
        }
    }
}

/// Generic wrapper around Bitbucket's paginated list responses.
#[derive(Debug, Deserialize)]
struct Paginated<T> {
    #[serde(default = "Vec::new")]
    values: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoPermissionEntry {
    #[serde(default)]
    permission: Option<String>,
}

/// A Bitbucket Cloud pull request, normalised to the fields `but_forge` needs.
#[derive(Debug)]
pub struct BitbucketPullRequest {
    pub html_url: String,
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    /// One of `OPEN`, `MERGED`, `DECLINED`, `SUPERSEDED`.
    pub state: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub source_commit_hash: String,
    pub merge_commit_hash: Option<String>,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
    pub comment_count: i64,
    pub author: Option<BitbucketUser>,
    pub reviewers: Vec<BitbucketUser>,
}

impl BitbucketPullRequest {
    pub fn is_open(&self) -> bool {
        self.state == "OPEN"
    }

    // NOTE: Bitbucket's PR object has no dedicated merged/closed timestamp, so
    // these approximate it with `updated_on` (the only available timestamp). A
    // post-merge/decline edit shifts the value, so consumers that bucket by date
    // (e.g. "merged this week") are approximate, not exact.
    pub fn merged_at(&self) -> Option<String> {
        (self.state == "MERGED")
            .then(|| self.updated_on.clone())
            .flatten()
    }

    pub fn closed_at(&self) -> Option<String> {
        matches!(self.state.as_str(), "DECLINED" | "SUPERSEDED")
            .then(|| self.updated_on.clone())
            .flatten()
    }
}

#[derive(Debug, Deserialize)]
struct BitbucketApiPullRequest {
    id: i64,
    title: String,
    #[serde(default)]
    description: Option<String>,
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    created_on: Option<String>,
    #[serde(default)]
    updated_on: Option<String>,
    #[serde(default)]
    comment_count: i64,
    #[serde(default)]
    author: Option<BitbucketApiUser>,
    #[serde(default)]
    reviewers: Vec<BitbucketApiUser>,
    #[serde(default)]
    merge_commit: Option<BitbucketCommitRef>,
    source: BitbucketEndpoint,
    destination: BitbucketEndpoint,
    #[serde(default)]
    links: Option<BitbucketLinks>,
}

#[derive(Debug, Deserialize)]
struct BitbucketEndpoint {
    #[serde(default)]
    branch: Option<BitbucketBranchRef>,
    #[serde(default)]
    commit: Option<BitbucketCommitRef>,
}

#[derive(Debug, Deserialize)]
struct BitbucketBranchRef {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BitbucketCommitRef {
    #[serde(default)]
    hash: Option<String>,
}

impl From<BitbucketApiPullRequest> for BitbucketPullRequest {
    fn from(pr: BitbucketApiPullRequest) -> Self {
        let html_url = pr
            .links
            .and_then(|l| l.html)
            .and_then(|h| h.href)
            .unwrap_or_default();
        BitbucketPullRequest {
            html_url,
            id: pr.id,
            title: pr.title,
            description: pr.description,
            state: pr.state,
            draft: pr.draft,
            source_branch: pr.source.branch.and_then(|b| b.name).unwrap_or_default(),
            target_branch: pr
                .destination
                .branch
                .and_then(|b| b.name)
                .unwrap_or_default(),
            source_commit_hash: pr.source.commit.and_then(|c| c.hash).unwrap_or_default(),
            merge_commit_hash: pr.merge_commit.and_then(|c| c.hash),
            created_on: pr.created_on,
            updated_on: pr.updated_on,
            comment_count: pr.comment_count,
            author: pr.author.map(Into::into),
            reviewers: pr.reviewers.into_iter().map(Into::into).collect(),
        }
    }
}

/// Repository metadata used to populate `but_forge`'s `RepoInfo`.
#[derive(Debug)]
pub struct BitbucketRepo {
    pub is_fork: bool,
    /// The authenticated user's permission: `admin`, `write` or `read`.
    pub permission: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BitbucketApiRepository {
    /// Present (and non-null) only when the repository is a fork.
    #[serde(default)]
    parent: Option<serde::de::IgnoredAny>,
}

/// A Bitbucket commit build status (Pipelines or any external CI that reports back).
#[derive(Debug)]
pub struct BitbucketBuildStatus {
    /// Unique key of the build status on the commit.
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    /// One of `SUCCESSFUL`, `FAILED`, `INPROGRESS`, `STOPPED`.
    pub state: String,
    pub url: Option<String>,
    pub commit_hash: String,
    pub created_on: Option<String>,
    pub updated_on: Option<String>,
}

impl BitbucketBuildStatus {
    fn from_api(status: BitbucketApiBuildStatus, fallback_commit: &str) -> Self {
        let commit_hash = status
            .commit
            .and_then(|c| c.hash)
            .unwrap_or_else(|| fallback_commit.to_owned());
        // Bitbucket requires `key`; `name` is optional, so fall back to it.
        let name = status.name.unwrap_or_else(|| status.key.clone());
        BitbucketBuildStatus {
            key: status.key,
            name,
            description: status.description,
            state: status.state,
            url: status.url,
            commit_hash,
            created_on: status.created_on,
            updated_on: status.updated_on,
        }
    }
}

#[derive(Debug, Deserialize)]
struct BitbucketApiBuildStatus {
    key: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    state: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    commit: Option<BitbucketCommitRef>,
    #[serde(default)]
    created_on: Option<String>,
    #[serde(default)]
    updated_on: Option<String>,
}

pub struct CreatePullRequestParams<'a> {
    pub workspace: &'a str,
    pub repo_slug: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub source_branch: &'a str,
    pub target_branch: &'a str,
    /// `workspace/repo_slug` of the source repository when opening from a fork.
    pub source_repo_full_name: Option<&'a str>,
    pub draft: bool,
}

pub struct UpdatePullRequestParams<'a> {
    pub workspace: &'a str,
    pub repo_slug: &'a str,
    pub id: i64,
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub target_branch: Option<&'a str>,
}

pub struct SetPullRequestDraftStateParams<'a> {
    pub workspace: &'a str,
    pub repo_slug: &'a str,
    pub id: i64,
    pub is_draft: bool,
}

pub struct MergePullRequestParams<'a> {
    pub workspace: &'a str,
    pub repo_slug: &'a str,
    pub id: i64,
    pub strategy: MergeStrategy,
}

/// Bitbucket Cloud merge strategies.
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    MergeCommit,
    Squash,
    FastForward,
    SquashFastForward,
    RebaseFastForward,
    RebaseMerge,
}

impl MergeStrategy {
    fn as_str(self) -> &'static str {
        match self {
            MergeStrategy::MergeCommit => "merge_commit",
            MergeStrategy::Squash => "squash",
            MergeStrategy::FastForward => "fast_forward",
            MergeStrategy::SquashFastForward => "squash_fast_forward",
            MergeStrategy::RebaseFastForward => "rebase_fast_forward",
            MergeStrategy::RebaseMerge => "rebase_merge",
        }
    }
}

#[derive(Serialize)]
struct CreatePullRequestBody<'a> {
    title: &'a str,
    description: &'a str,
    draft: bool,
    close_source_branch: bool,
    source: SourceBody<'a>,
    destination: DestinationBody<'a>,
}

struct PrEditContext {
    title: String,
    description: Option<String>,
    draft: bool,
    close_source_branch: bool,
    destination_branch: Option<String>,
    reviewer_account_ids: Vec<String>,
}

#[derive(Serialize)]
struct UpdatePullRequestBody<'a> {
    title: &'a str,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    destination: Option<DestinationBody<'a>>,
    draft: bool,
    close_source_branch: bool,
    reviewers: Vec<ReviewerBody<'a>>,
}

#[derive(Serialize)]
struct ReviewerBody<'a> {
    account_id: &'a str,
}

#[derive(Serialize)]
struct MergePullRequestBody<'a> {
    merge_strategy: &'a str,
}

#[derive(Serialize)]
struct SourceBody<'a> {
    branch: BranchBody<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<RepositoryBody<'a>>,
}

#[derive(Serialize)]
struct DestinationBody<'a> {
    branch: BranchBody<'a>,
}

#[derive(Serialize)]
struct BranchBody<'a> {
    name: &'a str,
}

#[derive(Serialize)]
struct RepositoryBody<'a> {
    full_name: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_strategy_serializes_to_bitbucket_names() {
        assert_eq!(MergeStrategy::MergeCommit.as_str(), "merge_commit");
        assert_eq!(MergeStrategy::Squash.as_str(), "squash");
        assert_eq!(MergeStrategy::FastForward.as_str(), "fast_forward");
        assert_eq!(
            MergeStrategy::SquashFastForward.as_str(),
            "squash_fast_forward"
        );
        assert_eq!(
            MergeStrategy::RebaseFastForward.as_str(),
            "rebase_fast_forward"
        );
        assert_eq!(MergeStrategy::RebaseMerge.as_str(), "rebase_merge");
    }

    #[test]
    fn parses_pull_request_json() {
        let json = r#"{
            "id": 42,
            "title": "Add feature",
            "description": "Body text",
            "state": "OPEN",
            "draft": true,
            "created_on": "2026-01-01T00:00:00Z",
            "updated_on": "2026-01-02T00:00:00Z",
            "comment_count": 3,
            "author": { "type": "user", "nickname": "alice", "display_name": "Alice", "account_id": "a1" },
            "reviewers": [ { "type": "user", "nickname": "bob", "account_id": "b1" } ],
            "source": { "branch": { "name": "feature" }, "commit": { "hash": "deadbeef" } },
            "destination": { "branch": { "name": "main" } },
            "links": { "html": { "href": "https://bitbucket.org/ws/repo/pull-requests/42" } }
        }"#;

        let api: BitbucketApiPullRequest = serde_json::from_str(json).unwrap();
        let pr: BitbucketPullRequest = api.into();

        assert_eq!(pr.id, 42);
        assert_eq!(pr.title, "Add feature");
        assert_eq!(pr.description.as_deref(), Some("Body text"));
        assert!(pr.draft);
        assert!(pr.is_open());
        assert_eq!(pr.source_branch, "feature");
        assert_eq!(pr.target_branch, "main");
        assert_eq!(pr.source_commit_hash, "deadbeef");
        assert_eq!(pr.comment_count, 3);
        assert_eq!(
            pr.html_url,
            "https://bitbucket.org/ws/repo/pull-requests/42"
        );
        assert_eq!(pr.merged_at(), None);
        assert_eq!(pr.closed_at(), None);
        assert_eq!(pr.reviewers.len(), 1);
        assert_eq!(pr.author.unwrap().username, "alice");
    }

    #[test]
    fn derives_merged_and_closed_timestamps_from_state() {
        let base = BitbucketApiPullRequest {
            id: 1,
            title: "t".into(),
            description: None,
            state: "MERGED".into(),
            draft: false,
            created_on: None,
            updated_on: Some("2026-01-02T00:00:00Z".into()),
            comment_count: 0,
            author: None,
            reviewers: vec![],
            merge_commit: Some(BitbucketCommitRef {
                hash: Some("cafef00d".into()),
            }),
            source: BitbucketEndpoint {
                branch: Some(BitbucketBranchRef {
                    name: Some("feature".into()),
                }),
                commit: Some(BitbucketCommitRef {
                    hash: Some("deadbeef".into()),
                }),
            },
            destination: BitbucketEndpoint {
                branch: Some(BitbucketBranchRef {
                    name: Some("main".into()),
                }),
                commit: None,
            },
            links: None,
        };
        let merged: BitbucketPullRequest = base.into();
        assert_eq!(merged.merged_at().as_deref(), Some("2026-01-02T00:00:00Z"));
        assert_eq!(merged.closed_at(), None);
        assert!(!merged.is_open());
        assert_eq!(merged.merge_commit_hash.as_deref(), Some("cafef00d"));
    }

    fn pr_in_state(state: &str) -> BitbucketPullRequest {
        BitbucketApiPullRequest {
            id: 1,
            title: "t".into(),
            description: None,
            state: state.into(),
            draft: false,
            created_on: None,
            updated_on: Some("2026-01-02T00:00:00Z".into()),
            comment_count: 0,
            author: None,
            reviewers: vec![],
            merge_commit: None,
            source: BitbucketEndpoint {
                branch: None,
                commit: None,
            },
            destination: BitbucketEndpoint {
                branch: None,
                commit: None,
            },
            links: None,
        }
        .into()
    }

    #[test]
    fn declined_and_superseded_states_derive_closed_at() {
        for state in ["DECLINED", "SUPERSEDED"] {
            let pr = pr_in_state(state);
            assert_eq!(
                pr.closed_at().as_deref(),
                Some("2026-01-02T00:00:00Z"),
                "{state} should derive closed_at"
            );
            assert_eq!(pr.merged_at(), None, "{state} should not be merged");
            assert!(!pr.is_open(), "{state} should not be open");
        }
    }

    #[test]
    fn open_state_has_no_merged_or_closed_timestamp() {
        let pr = pr_in_state("OPEN");
        assert!(pr.is_open());
        assert_eq!(pr.merged_at(), None);
        assert_eq!(pr.closed_at(), None);
    }

    #[test]
    fn encode_branch_path_keeps_slashes_literal() {
        assert_eq!(encode_branch_path("feature/login"), "feature/login");
        assert_eq!(encode_branch_path("main"), "main");
        // Per-segment encoding still escapes characters within a segment.
        assert_eq!(encode_branch_path("feat/a b"), "feat/a%20b");
        assert_eq!(encode_branch_path("a/b/c"), "a/b/c");
    }

    #[test]
    fn is_full_commit_hash_only_matches_40_char_hex() {
        assert!(is_full_commit_hash(
            "0123456789abcdef0123456789abcdef01234567"
        ));
        assert!(is_full_commit_hash(
            "0123456789ABCDEF0123456789ABCDEF01234567"
        ));
        // A branch name, a short hash, and an over-long string are all rejected.
        assert!(!is_full_commit_hash("main"));
        assert!(!is_full_commit_hash("0123456"));
        assert!(!is_full_commit_hash(
            "0123456789abcdef0123456789abcdef012345678"
        ));
        // 40 chars but not all hex.
        assert!(!is_full_commit_hash(
            "feature/0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn escape_bbql_escapes_quotes_and_backslashes() {
        assert_eq!(escape_bbql("main"), "main");
        assert_eq!(escape_bbql(r#"a"b"#), r#"a\"b"#);
        assert_eq!(escape_bbql(r"a\b"), r"a\\b");
        // Backslash is escaped before the quote, so an escaped quote stays escaped.
        assert_eq!(escape_bbql(r#"\""#), r#"\\\""#);
    }

    fn edit_ctx() -> PrEditContext {
        PrEditContext {
            title: "Original".into(),
            description: Some("orig body".into()),
            draft: true,
            close_source_branch: true,
            destination_branch: Some("main".into()),
            reviewer_account_ids: vec!["acc-1".into(), "acc-2".into()],
        }
    }

    #[test]
    fn build_update_body_preserves_reviewers_draft_and_close_flag() {
        // Override only the title; everything else must round-trip from the PR.
        let ctx = edit_ctx();
        let body = build_update_body(&ctx, Some("New title"), None, None, None);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["title"], "New title");
        assert_eq!(json["description"], "orig body");
        assert_eq!(json["destination"]["branch"]["name"], "main");
        assert_eq!(json["draft"], true);
        assert_eq!(json["close_source_branch"], true);
        assert_eq!(
            json["reviewers"],
            serde_json::json!([{"account_id": "acc-1"}, {"account_id": "acc-2"}])
        );
    }

    #[test]
    fn build_update_body_applies_overrides() {
        let ctx = edit_ctx();
        let body = build_update_body(&ctx, None, Some("new body"), Some("develop"), Some(false));
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["title"], "Original");
        assert_eq!(json["description"], "new body");
        assert_eq!(json["destination"]["branch"]["name"], "develop");
        assert_eq!(json["draft"], false);
        assert_eq!(json["close_source_branch"], true);
        assert_eq!(json["reviewers"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn list_for_target_query_includes_all_states_and_escapes_branch() {
        let q = list_for_target_query(r#"feat/"x"#);
        for state in ["OPEN", "MERGED", "DECLINED", "SUPERSEDED"] {
            assert!(
                q.contains(&format!("state = \"{state}\"")),
                "missing {state}"
            );
        }
        assert!(q.contains(r#"destination.branch.name = "feat/\"x""#));
    }

    #[test]
    fn stable_id_hash_is_stable_distinct_nonzero() {
        use crate::stable_id_hash;
        assert_eq!(stable_id_hash("acc-1"), stable_id_hash("acc-1"));
        assert_ne!(stable_id_hash("acc-1"), stable_id_hash("acc-2"));
        assert_ne!(stable_id_hash("acc-1"), 0);
    }
}
