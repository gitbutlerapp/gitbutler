use anyhow::{Context, Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

const GITEA_API_BASE_URL: &str = "https://gitea.com/api/v1";

pub struct GiteaClient {
    client: reqwest::Client,
    base_url: String,
}

// Fix for Issue #2904 - Gitea Support
impl GiteaClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let client = build_client(access_token)?;
        Ok(Self {
            client,
            base_url: GITEA_API_BASE_URL.to_string(),
        })
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
                "No Gitea access token found for account '{account_id}'.\nRun 'but config forge auth' to re-authenticate."
            ))
        }
    }

    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let client = build_client(access_token)?;
        let base_url = normalize_base_url(host);
        Ok(Self { client, base_url })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        #[derive(Deserialize)]
        struct User {
            login: Option<String>,
            username: Option<String>,
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
        let username = user
            .login
            .or(user.username)
            .context("Gitea authenticated user did not include a username")?;

        Ok(AuthenticatedUser {
            username,
            avatar_url: user.avatar_url,
            name: user.full_name,
            email: user.email,
        })
    }

    pub async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let url = format!("{}/user/repos", self.base_url);
        let response = self
            .client
            .get(&url)
            .query(&[("page", "1"), ("limit", "100")])
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to list repositories: {}", response.status());
        }
        let repos: Vec<GiteaRepositoryApi> = response.json().await?;
        Ok(repos.into_iter().map(Into::into).collect())
    }

    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{owner}/{repo}/pulls", self.base_url);
        let response = self
            .client
            .get(&url)
            .query(&[("state", "open"), ("page", "1"), ("limit", "100")])
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to list open pulls: {}", response.status());
        }
        let pulls: Vec<GiteaPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    pub async fn list_pulls_for_base(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
    ) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{owner}/{repo}/pulls", self.base_url);
        let response = self
            .client
            .get(&url)
            .query(&[("state", "all"), ("base", base), ("page", "1"), ("limit", "100")])
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to list pulls for base: {}", response.status());
        }
        let pulls: Vec<GiteaPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct Body<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            base: &'a str,
            draft: bool,
        }

        let url = format!("{}/repos/{}/{}/pulls", self.base_url, params.owner, params.repo);
        let response = self
            .client
            .post(&url)
            .json(&Body {
                title: params.title,
                body: params.body,
                head: params.head,
                base: params.base,
                draft: params.draft,
            })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to create pull request: {}", response.status());
        }
        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: i64) -> Result<PullRequest> {
        let url = format!("{}/repos/{owner}/{repo}/pulls/{pr_number}", self.base_url);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            bail!("Failed to get pull request: {}", response.status());
        }
        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn update_pull_request(
        &self,
        params: &UpdatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct Body<'a> {
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
        let response = self
            .client
            .patch(&url)
            .json(&Body {
                title: params.title,
                body: params.body,
                base: params.base,
                state: params.state,
            })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to update pull request: {}", response.status());
        }
        let pr: GiteaPullRequest = response.json().await?;
        Ok(pr.into())
    }

    pub async fn merge_pull_request(&self, params: &MergePullRequestParams<'_>) -> Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_method: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_commit_id: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_message_field: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_title_field: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            delete_branch_after_merge: Option<bool>,
        }

        let url = format!(
            "{}/repos/{}/{}/pulls/{}/merge",
            self.base_url, params.owner, params.repo, params.pr_number
        );
        let merge_method = params.merge_method.as_ref().map(Into::into);
        let response = self
            .client
            .post(&url)
            .json(&Body {
                merge_method,
                merge_commit_id: None,
                merge_message_field: params.commit_message,
                merge_title_field: params.commit_title,
                delete_branch_after_merge: None,
            })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to merge pull request: {}", response.status());
        }
        Ok(())
    }

    pub async fn set_pull_request_draft_state(
        &self,
        params: &SetPullRequestDraftStateParams<'_>,
    ) -> Result<()> {
        let current = self
            .get_pull_request(params.owner, params.repo, params.pr_number)
            .await?;
        let new_title = update_draft_state_in_title(&current.title, params.draft);
        self.update_pull_request(&UpdatePullRequestParams {
            owner: params.owner,
            repo: params.repo,
            pr_number: params.pr_number,
            title: Some(&new_title),
            body: None,
            base: None,
            state: None,
        })
        .await
        .map(|_| ())
    }

    pub async fn set_pull_request_auto_merge(
        &self,
        params: &SetPullRequestAutoMergeParams<'_>,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}/merge",
            self.base_url, params.owner, params.repo, params.pr_number
        );

        if let AutoMergeState::Disabled = params.state {
            let cancel_url = format!(
                "{}/repos/{}/{}/pulls/{}/merge",
                self.base_url, params.owner, params.repo, params.pr_number
            );
            let response = self.client.delete(&cancel_url).send().await?;
            if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(());
            }
            bail!("Failed to disable PR auto-merge: {}", response.status());
        }

        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_when_checks_succeed: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_method: Option<&'a str>,
        }

        let merge_method = match &params.state {
            AutoMergeState::Enabled(cfg) => cfg.merge_method.as_ref().map(Into::into),
            AutoMergeState::Disabled => None,
        };

        let response = self
            .client
            .post(&url)
            .json(&Body {
                merge_when_checks_succeed: Some(true),
                merge_method,
            })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to enable PR auto-merge: {}", response.status());
        }
        Ok(())
    }
}

fn build_client(access_token: &Sensitive<String>) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("gb-gitea-integration"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("token {}", access_token.0))?,
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("Failed to build Gitea HTTP client")
}

fn normalize_base_url(host: &str) -> String {
    if host.ends_with("/api/v1") {
        host.to_string()
    } else if host.ends_with('/') {
        format!("{host}api/v1")
    } else {
        format!("{host}/api/v1")
    }
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
    let Some((prefix, _)) = title.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    prefix.eq_ignore_ascii_case("draft") || prefix.eq_ignore_ascii_case("wip")
}

fn remove_draft_prefix(title: &str) -> &str {
    let Some((prefix, rest)) = title.split_once(':') else {
        return title;
    };
    let prefix = prefix.trim();
    if prefix.eq_ignore_ascii_case("draft") || prefix.eq_ignore_ascii_case("wip") {
        rest.trim_start()
    } else {
        title
    }
}

pub struct CreatePullRequestParams<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub head: &'a str,
    pub head_repo: Option<&'a str>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MergeMethod {
    #[default]
    Merge,
    Squash,
    Rebase,
}

impl From<&MergeMethod> for &str {
    fn from(val: &MergeMethod) -> Self {
        match val {
            MergeMethod::Merge => "merge",
            MergeMethod::Rebase => "rebase",
            MergeMethod::Squash => "squash",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MergePullRequestParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub pr_number: i64,
    pub commit_title: Option<&'a str>,
    pub commit_message: Option<&'a str>,
    pub merge_method: Option<MergeMethod>,
}

#[derive(Debug, Clone)]
pub struct SetPullRequestDraftStateParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub pr_number: i64,
    pub draft: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AutoMergeEnableParams<'a> {
    merge_method: Option<MergeMethod>,
    expected_head_oid: Option<&'a str>,
    commit_headline: Option<&'a str>,
    commit_body: Option<&'a str>,
    author_email: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum AutoMergeState<'a> {
    Disabled,
    Enabled(AutoMergeEnableParams<'a>),
}

impl From<bool> for AutoMergeState<'_> {
    fn from(value: bool) -> Self {
        if value {
            AutoMergeState::Enabled(Default::default())
        } else {
            AutoMergeState::Disabled
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetPullRequestAutoMergeParams<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub pr_number: i64,
    pub state: AutoMergeState<'a>,
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
    login: Option<String>,
    username: Option<String>,
    full_name: Option<String>,
    email: Option<String>,
    avatar_url: Option<String>,
    is_bot: Option<bool>,
}

impl From<GiteaApiUser> for GiteaUser {
    fn from(user: GiteaApiUser) -> Self {
        GiteaUser {
            id: user.id,
            login: user.login.or(user.username).unwrap_or_default(),
            name: user.full_name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.is_bot.unwrap_or(false),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GiteaLabel {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GiteaLabelApi {
    id: i64,
    name: String,
    description: Option<String>,
    color: Option<String>,
}

impl From<GiteaLabelApi> for GiteaLabel {
    fn from(label: GiteaLabelApi) -> Self {
        GiteaLabel {
            id: label.id,
            name: label.name,
            description: label.description,
            color: label.color,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Repository {
    pub ssh_url: Option<String>,
    pub clone_url: Option<String>,
    pub owner: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct GiteaRepositoryApi {
    ssh_url: Option<String>,
    clone_url: Option<String>,
    owner: Option<GiteaRepoOwnerApi>,
    name: String,
}

#[derive(Debug, Deserialize)]
struct GiteaRepoOwnerApi {
    login: Option<String>,
    username: Option<String>,
}

impl From<GiteaRepositoryApi> for Repository {
    fn from(repo: GiteaRepositoryApi) -> Self {
        Repository {
            ssh_url: repo.ssh_url,
            clone_url: repo.clone_url,
            owner: repo.owner.and_then(|owner| owner.login.or(owner.username)),
            name: repo.name,
        }
    }
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
    pub modified_at: Option<String>,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub repository_ssh_url: Option<String>,
    pub repository_https_url: Option<String>,
    pub repo_owner: Option<String>,
    pub requested_reviewers: Vec<GiteaUser>,
}

#[derive(Debug, Deserialize)]
struct GiteaPullRequest {
    html_url: Option<String>,
    url: Option<String>,
    number: i64,
    title: String,
    body: Option<String>,
    user: Option<GiteaApiUser>,
    labels: Vec<GiteaLabelApi>,
    draft: Option<bool>,
    head: GiteaBranch,
    base: GiteaBranch,
    merge_base: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    requested_reviewers: Option<Vec<GiteaApiUser>>,
}

#[derive(Debug, Deserialize)]
struct GiteaBranch {
    #[serde(rename = "ref")]
    reference: String,
    sha: Option<String>,
    repo: Option<GiteaRepositoryApi>,
    owner: Option<GiteaRepoOwnerApi>,
}

impl From<GiteaPullRequest> for PullRequest {
    fn from(pr: GiteaPullRequest) -> Self {
        let (repository_ssh_url, repository_https_url, repo_owner) = match pr.head.repo {
            Some(repo) => {
                let owner = repo.owner.and_then(|o| o.login.or(o.username));
                (repo.ssh_url, repo.clone_url, owner)
            }
            None => (None, None, pr.head.owner.and_then(|o| o.login.or(o.username))),
        };

        PullRequest {
            html_url: pr.html_url.or(pr.url).unwrap_or_default(),
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author: pr.user.map(Into::into),
            labels: pr.labels.into_iter().map(Into::into).collect(),
            draft: pr.draft.unwrap_or(false),
            source_branch: pr.head.reference,
            target_branch: pr.base.reference,
            sha: pr.head.sha.or(pr.merge_base).unwrap_or_default(),
            created_at: pr.created_at,
            modified_at: pr.updated_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url,
            repository_https_url,
            repo_owner,
            requested_reviewers: pr
                .requested_reviewers
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

pub(crate) fn resolve_account(
    preferred_account: Option<&crate::GiteaAccountIdentifier>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::GiteaAccountIdentifier, anyhow::Error> {
    let known_accounts = crate::token::list_known_gitea_accounts(storage)?;
    let Some(default_account) = known_accounts.first() else {
        bail!(
            "No authenticated Gitea users found.\nRun 'but config forge auth' to authenticate with Gitea."
        );
    };
    let account = if let Some(account) = preferred_account {
        if known_accounts.contains(account) {
            account
        } else {
            bail!(
                "Preferred Gitea account '{account}' has not authenticated yet.\nRun 'but config forge auth' to authenticate, or choose another account."
            );
        }
    } else {
        default_account
    };

    Ok(account.to_owned())
}
