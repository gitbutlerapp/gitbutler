use anyhow::{Result, bail};
use but_secret::Sensitive;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::graphql::{
    GQL_DISABLE_PR_AUTO_MERGE, GQL_ENABLE_PR_AUTO_MERGE, GQL_GET_PR_NODE_ID, GQL_SET_PR_DRAFT,
    GQL_SET_PR_READY_FOR_REVIEW,
};

const GITHUB_API_BASE_URL: &str = "https://api.github.com";

pub struct GitHubClient {
    client: reqwest::Client,
    base_url: String,
}

impl GitHubClient {
    /// Create a new instance of the GitHub client
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-github-integration"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: GITHUB_API_BASE_URL.to_string(),
        })
    }

    /// Create a new instance of a GitHub client out of the stored accounts information.
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

    /// Create a new instance of a GitHub client, with a custom base URL.
    ///
    /// This is used to create the GitHub client for Enterprise users.
    pub fn new_with_host_override(access_token: &Sensitive<String>, host: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("gb-github-integration"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token.0))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            base_url: host.to_string(),
        })
    }

    /// Get the authenticated user.
    ///
    /// This is used to verify that the client has been correctly created and it holds the right authentication.
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

    /// Fetch the CI checks for a given branch reference
    ///
    /// Will fetch max 100 CI checks, and if the actual total checks
    /// excede that, it will paginate until all checks are fetched
    pub async fn list_checks_for_ref(
        &self,
        owner: &str,
        repo: &str,
        reference: &str,
    ) -> Result<Vec<CheckRun>> {
        #[derive(Deserialize)]
        struct CheckRunsResponse {
            total_count: usize,
            check_runs: Vec<CheckRun>,
        }

        let url = format!(
            "{}/repos/{}/{}/commits/{}/check-runs",
            self.base_url, owner, repo, reference
        );

        let mut page = 1;

        let response = self.fetch_check_runs(&url, page).await?;

        let result: CheckRunsResponse = response.json().await?;
        let total_count = result.total_count;
        let mut check_runs = result.check_runs;

        while check_runs.len() < total_count {
            page += 1;

            let response = self.fetch_check_runs(&url, page).await?;
            let result: CheckRunsResponse = response.json().await?;
            if result.check_runs.is_empty() {
                break;
            }

            check_runs.extend(result.check_runs);
        }

        Ok(check_runs)
    }

    /// The actual REST API call to fetch a page of the checks.
    async fn fetch_check_runs(
        &self,
        url: &str,
        page: usize,
    ) -> Result<reqwest::Response> {
        let response = self
            .client
            .get(url)
            .query(&CheckRunsQuery {
                filter: "latest",
                per_page: 100,
                page,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list checks for ref: {}", response.status());
        }

        Ok(response)
    }

    /// Fetch the list of the open PRs on a repo.
    pub async fn list_open_pulls(&self, owner: &str, repo: &str) -> Result<Vec<PullRequest>> {
        let url = format!("{}/repos/{}/{}/pulls", self.base_url, owner, repo);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "open"),
                ("sort", "updated"),
                ("direction", "desc"),
                ("per_page", "100"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to list open pulls: {}", response.status());
        }

        let pulls: Vec<GitHubPullRequest> = response.json().await?;
        Ok(pulls.into_iter().map(Into::into).collect())
    }

    /// List the PRs for a given target.
    pub async fn list_pulls_for_base(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
    ) -> Result<Vec<PullRequest>> {
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

    /// Create a PR.
    pub async fn create_pull_request(
        &self,
        params: &CreatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
        #[derive(Serialize)]
        struct CreatePullRequestBody<'a> {
            title: &'a str,
            body: &'a str,
            head: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            head_repo: Option<&'a str>,
            base: &'a str,
            draft: bool,
        }

        let url = format!(
            "{}/repos/{}/{}/pulls",
            self.base_url, params.owner, params.repo
        );

        let body = CreatePullRequestBody {
            title: params.title,
            body: params.body,
            head: params.head,
            head_repo: params.head_repo,
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

    /// Fetch the information of a given PR.
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: i64,
    ) -> Result<PullRequest> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, owner, repo, pr_number
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            bail!("Failed to get pull request: {}", response.status());
        }

        let pr: GitHubPullRequest = response.json().await?;
        Ok(pr.into())
    }

    /// Update the information of a given PR.
    ///
    /// This is used e.g. to update the description footers for stacked reviews.
    pub async fn update_pull_request(
        &self,
        params: &UpdatePullRequestParams<'_>,
    ) -> Result<PullRequest> {
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

    /// Merge a PR.
    pub async fn merge_pull_request(&self, params: &MergePullRequestParams<'_>) -> Result<()> {
        #[derive(Serialize)]
        struct MergePullRequestBody<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            commit_title: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            commit_message: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_method: Option<&'a str>,
        }

        let url = format!(
            "{}/repos/{}/{}/pulls/{}/merge",
            self.base_url, params.owner, params.repo, params.pr_number
        );

        let merge_method = params.merge_method.as_ref().map(Into::into);

        let body = MergePullRequestBody {
            commit_title: params.commit_title,
            commit_message: params.commit_message,
            merge_method,
        };

        let response = self.client.put(&url).json(&body).send().await?;

        if !response.status().is_success() {
            bail!("Failed to merge pull request: {}", response.status());
        }

        Ok(())
    }

    /// Set the draftiness of a PR.
    pub async fn set_pull_request_draft_state(
        &self,
        params: &SetPullRequestDraftStateParams<'_>,
    ) -> Result<()> {
        if params.pr_number <= 0 {
            bail!("PR number must be greater than 0");
        }

        let pull_request_id = self
            .get_pull_request_node_id(params.owner, params.repo, params.pr_number)
            .await?;

        if params.draft {
            self.set_pull_request_to_draft(&pull_request_id).await
        } else {
            self.set_pull_request_to_ready_for_review(&pull_request_id)
                .await
        }
    }

    async fn set_pull_request_to_ready_for_review(
        &self,
        pull_request_id: &PullRequestNodeId,
    ) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Variables<'a> {
            pull_request_id: &'a PullRequestNodeId,
        }
        #[derive(Deserialize)]
        struct MutationData {
            #[serde(rename = "markPullRequestReadyForReview")]
            mark_pull_request_ready_for_review: MutationPayload,
        }

        #[derive(Deserialize)]
        struct MutationPayload {
            #[serde(rename = "pullRequest")]
            pull_request: GraphQlPullRequest,
        }

        #[derive(Deserialize)]
        struct GraphQlPullRequest {
            id: String,
        }

        let data: MutationData = self
            .graphql_query(GQL_SET_PR_READY_FOR_REVIEW, &Variables { pull_request_id })
            .await?;

        if data
            .mark_pull_request_ready_for_review
            .pull_request
            .id
            .is_empty()
        {
            bail!("GitHub GraphQL markPullRequestReadyForReview returned an empty pull request id");
        }

        Ok(())
    }

    async fn set_pull_request_to_draft(&self, pull_request_id: &PullRequestNodeId) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Variables<'a> {
            pull_request_id: &'a PullRequestNodeId,
        }

        #[derive(Deserialize)]
        struct MutationData {
            #[serde(rename = "convertPullRequestToDraft")]
            convert_pull_request_to_draft: MutationPayload,
        }

        #[derive(Deserialize)]
        struct MutationPayload {
            #[serde(rename = "pullRequest")]
            pull_request: GraphQlPullRequest,
        }

        #[derive(Deserialize)]
        struct GraphQlPullRequest {
            id: String,
        }

        let data: MutationData = self
            .graphql_query(GQL_SET_PR_DRAFT, &Variables { pull_request_id })
            .await?;

        if data
            .convert_pull_request_to_draft
            .pull_request
            .id
            .is_empty()
        {
            bail!("GitHub GraphQL convertPullRequestToDraft returned an empty pull request id");
        }

        Ok(())
    }

    /// Enable or disable a PR's auto-merge.
    pub async fn set_pull_request_auto_merge(
        &self,
        params: &SetPullRequestAutoMergeParams<'_>,
    ) -> Result<()> {
        let pull_request_id = self
            .get_pull_request_node_id(params.owner, params.repo, params.pr_number)
            .await?;

        match &params.state {
            AutoMergeState::Disabled => {
                self.disable_auto_merge_pull_request(&pull_request_id).await
            }
            AutoMergeState::Enabled(params) => {
                self.enable_auto_merge_pull_request(&pull_request_id, params)
                    .await
            }
        }
    }

    async fn enable_auto_merge_pull_request(
        &self,
        pull_request_id: &PullRequestNodeId,
        params: &AutoMergeEnableParams<'_>,
    ) -> Result<()> {
        #[derive(Deserialize)]
        struct MutationPayload {
            #[serde(rename = "pullRequest")]
            pull_request: GraphQlPullRequest,
        }

        #[derive(Deserialize)]
        struct GraphQlPullRequest {
            id: String,
        }
        #[derive(Serialize)]
        struct Variables<'a> {
            input: EnablePullRequestAutoMergeInput<'a>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct EnablePullRequestAutoMergeInput<'a> {
            pull_request_id: &'a PullRequestNodeId,
            #[serde(skip_serializing_if = "Option::is_none")]
            merge_method: Option<GraphQlMergeMethod>,
            #[serde(skip_serializing_if = "Option::is_none")]
            expected_head_oid: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            commit_headline: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            commit_body: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            author_email: Option<&'a str>,
        }

        #[derive(Deserialize)]
        struct MutationData {
            #[serde(rename = "enablePullRequestAutoMerge")]
            enable_pull_request_auto_merge: MutationPayload,
        }

        let data: MutationData = self
            .graphql_query(
                GQL_ENABLE_PR_AUTO_MERGE,
                &Variables {
                    input: EnablePullRequestAutoMergeInput {
                        pull_request_id,
                        merge_method: params.merge_method.as_ref().map(Into::into),
                        expected_head_oid: params.expected_head_oid,
                        commit_headline: params.commit_headline,
                        commit_body: params.commit_body,
                        author_email: params.author_email,
                    },
                },
            )
            .await?;

        if data
            .enable_pull_request_auto_merge
            .pull_request
            .id
            .is_empty()
        {
            bail!("GitHub GraphQL enablePullRequestAutoMerge returned an empty pull request id");
        }

        Ok(())
    }

    async fn disable_auto_merge_pull_request(
        &self,
        pull_request_id: &PullRequestNodeId,
    ) -> Result<()> {
        #[derive(Deserialize)]
        struct MutationPayload {
            #[serde(rename = "pullRequest")]
            pull_request: GraphQlPullRequest,
        }

        #[derive(Deserialize)]
        struct GraphQlPullRequest {
            id: String,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Variables<'a> {
            pull_request_id: &'a PullRequestNodeId,
        }

        #[derive(Deserialize)]
        struct MutationData {
            #[serde(rename = "disablePullRequestAutoMerge")]
            disable_pull_request_auto_merge: MutationPayload,
        }

        let data: MutationData = self
            .graphql_query(GQL_DISABLE_PR_AUTO_MERGE, &Variables { pull_request_id })
            .await?;

        if data
            .disable_pull_request_auto_merge
            .pull_request
            .id
            .is_empty()
        {
            bail!("GitHub GraphQL disablePullRequestAutoMerge returned an empty pull request id");
        }

        Ok(())
    }

    async fn get_pull_request_node_id(
        &self,
        owner: &str,
        repo: &str,
        pr_number: i64,
    ) -> Result<PullRequestNodeId> {
        #[derive(Serialize)]
        struct Variables<'a> {
            owner: &'a str,
            repo: &'a str,
            number: i64,
        }

        #[derive(Deserialize)]
        struct QueryData {
            repository: Option<Repository>,
        }

        #[derive(Deserialize)]
        struct Repository {
            #[serde(rename = "pullRequest")]
            pull_request: Option<GraphQlPullRequest>,
        }

        #[derive(Deserialize)]
        struct GraphQlPullRequest {
            id: String,
        }

        let data: QueryData = self
            .graphql_query(
                GQL_GET_PR_NODE_ID,
                &Variables {
                    owner,
                    repo,
                    number: pr_number,
                },
            )
            .await?;

        let Some(repository) = data.repository else {
            bail!("GitHub GraphQL repository not found for {owner}/{repo}");
        };

        let Some(pull_request) = repository.pull_request else {
            bail!("GitHub GraphQL pull request #{pr_number} not found for {owner}/{repo}",);
        };

        if pull_request.id.is_empty() {
            bail!(
                "GitHub GraphQL pull request #{pr_number} in {owner}/{repo} returned an empty node id",
            );
        }

        Ok(PullRequestNodeId::from_string(pull_request.id))
    }

    async fn graphql_query<T, V>(&self, query: &str, variables: &V) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        V: Serialize,
    {
        #[derive(Serialize)]
        struct GraphQlRequest<'a, V> {
            query: &'a str,
            variables: &'a V,
        }

        #[derive(Deserialize)]
        struct GraphQlError {
            message: String,
        }

        #[derive(Deserialize)]
        struct GraphQlResponse<T> {
            data: Option<T>,
            errors: Option<Vec<GraphQlError>>,
        }

        let url = graphql_endpoint_from_base_url(&self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&GraphQlRequest { query, variables })
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("GitHub GraphQL request failed: {}", response.status());
        }

        let payload: GraphQlResponse<T> = response.json().await?;

        if let Some(errors) = payload.errors {
            let messages = errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; ");
            bail!("GitHub GraphQL returned errors: {messages}");
        }

        let Some(data) = payload.data else {
            bail!("GitHub GraphQL response did not include data");
        };

        Ok(data)
    }
}

pub struct PullRequestNodeId {
    id: String,
}

impl PullRequestNodeId {
    pub fn from_string(value: String) -> Self {
        PullRequestNodeId { id: value }
    }
}

impl Serialize for PullRequestNodeId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.id.serialize(serializer)
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GraphQlMergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl From<&MergeMethod> for GraphQlMergeMethod {
    fn from(value: &MergeMethod) -> Self {
        match value {
            MergeMethod::Merge => GraphQlMergeMethod::Merge,
            MergeMethod::Squash => GraphQlMergeMethod::Squash,
            MergeMethod::Rebase => GraphQlMergeMethod::Rebase,
        }
    }
}

fn graphql_endpoint_from_base_url(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');

    if let Some(base) = base_url.strip_suffix("/api/v3") {
        return format!("{base}/api/graphql");
    }

    if base_url == GITHUB_API_BASE_URL {
        return format!("{base_url}/graphql");
    }

    if base_url.ends_with("/graphql") || base_url.ends_with("/api/graphql") {
        return base_url.to_string();
    }

    format!("{base_url}/graphql")
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
            is_bot: user
                .user_type
                .map(|user_type| user_type == "bot")
                .unwrap_or(false),
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
        bail!(
            "No authenticated GitHub users found.\nRun 'but config forge auth' to authenticate with GitHub."
        );
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

#[derive(Serialize)]
struct CheckRunsQuery<'a> {
    filter: &'a str,
    per_page: usize,
    page: usize,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_endpoint_for_github_cloud() {
        let endpoint = graphql_endpoint_from_base_url("https://api.github.com");
        assert_eq!(endpoint, "https://api.github.com/graphql");
    }

    #[test]
    fn graphql_endpoint_for_ghes_api_v3() {
        let endpoint = graphql_endpoint_from_base_url("https://ghes.example.com/api/v3");
        assert_eq!(endpoint, "https://ghes.example.com/api/graphql");
    }

    #[test]
    fn graphql_endpoint_preserves_existing_graphql_path() {
        let endpoint = graphql_endpoint_from_base_url("https://ghes.example.com/api/graphql");
        assert_eq!(endpoint, "https://ghes.example.com/api/graphql");
    }

    #[test]
    fn graphql_merge_method_serializes_to_graphql_enum_values() {
        let merge = serde_json::to_value(GraphQlMergeMethod::from(&MergeMethod::Merge)).unwrap();
        let squash = serde_json::to_value(GraphQlMergeMethod::from(&MergeMethod::Squash)).unwrap();
        let rebase = serde_json::to_value(GraphQlMergeMethod::from(&MergeMethod::Rebase)).unwrap();

        assert_eq!(merge, serde_json::json!("MERGE"));
        assert_eq!(squash, serde_json::json!("SQUASH"));
        assert_eq!(rebase, serde_json::json!("REBASE"));
    }
}
