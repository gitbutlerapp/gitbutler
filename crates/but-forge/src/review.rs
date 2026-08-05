use std::{
    fmt::Display,
    path::{self},
};

use anyhow::{Context as _, Error, Result};
use but_github::CredentialCheckResult;
use but_gitlab::GitLabProjectId;
use but_utils::list_files;
use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::forge::ForgeName;

pub const STACKING_FOOTER_BOUNDARY_TOP: &str = "<!-- GitButler Footer Boundary Top -->";
pub const STACKING_FOOTER_BOUNDARY_BOTTOM: &str = "<!-- GitButler Footer Boundary Bottom -->";

/// Get a list of available review template paths for a project
///
/// The paths are relative to the root path
pub fn available_review_templates(root_path: &path::Path, forge_name: &ForgeName) -> Vec<String> {
    let ReviewTemplateFunctions {
        is_review_template,
        get_root,
        supported_template_directories,
        ..
    } = get_review_template_functions(forge_name);

    let forge_root_path = get_root(root_path);
    let forge_root_path = forge_root_path.as_path();

    // let walked_paths = list_files(forge_root_path, &[forge_root_path]).unwrap_or_default();

    supported_template_directories
        .iter()
        .flat_map(|dir| match dir {
            SupportedTemplateDirectory::ProjectRoot => {
                list_files(root_path, &[root_path], false, Some(root_path)).unwrap_or_default()
            }
            SupportedTemplateDirectory::ForgeRoot => {
                list_files(forge_root_path, &[root_path], true, Some(root_path)).unwrap_or_default()
            }
            SupportedTemplateDirectory::Custom(custom_dir) => {
                let custom_path = root_path.join(custom_dir);
                list_files(custom_path.as_path(), &[root_path], true, Some(root_path))
                    .unwrap_or_default()
            }
        })
        .filter_map(|entry| {
            let path_entry = entry.as_path();
            let path_str = path_entry.to_string_lossy();

            if is_review_template(&path_str) {
                return Some(path_str.to_string());
            }
            None
        })
        .collect()
}

pub enum SupportedTemplateDirectory {
    ProjectRoot,
    ForgeRoot,
    Custom(&'static str),
}

pub struct ReviewTemplateFunctions {
    /// Check if a file is a review template
    pub is_review_template: fn(&str) -> bool,
    /// Get the forge directory path
    pub get_root: fn(&path::Path) -> path::PathBuf,
    /// Check if a relative path is a valid review template path
    ///
    /// First argument is the relative path to the file
    /// Second argument is the root path of the project
    pub is_valid_review_template_path: fn(&path::Path) -> bool,
    /// The supported template directories
    pub supported_template_directories: &'static [SupportedTemplateDirectory],
}

pub fn get_review_template_functions(forge_name: &ForgeName) -> ReviewTemplateFunctions {
    match forge_name {
        ForgeName::GitHub => ReviewTemplateFunctions {
            is_review_template: is_review_template_github,
            get_root: get_github_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_github,
            supported_template_directories: &[
                SupportedTemplateDirectory::ForgeRoot,
                SupportedTemplateDirectory::ProjectRoot,
                SupportedTemplateDirectory::Custom("docs"),
            ],
        },
        ForgeName::GitLab => ReviewTemplateFunctions {
            is_review_template: is_review_template_gitlab,
            get_root: get_gitlab_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_gitlab,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
        ForgeName::Bitbucket => ReviewTemplateFunctions {
            is_review_template: is_review_template_bitbucket,
            get_root: get_bitbucket_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_bitbucket,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
        ForgeName::Azure => ReviewTemplateFunctions {
            is_review_template: is_review_template_azure,
            get_root: get_azure_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_azure,
            supported_template_directories: &[SupportedTemplateDirectory::ForgeRoot],
        },
    }
}

fn get_github_directory_path(root_path: &path::Path) -> path::PathBuf {
    let mut path = root_path.to_path_buf();
    path.push(".github");
    path
}

fn is_review_template_github(path_str: &str) -> bool {
    let normalized_path = path_str.replace('\\', "/");
    normalized_path == "PULL_REQUEST_TEMPLATE.md"
        || normalized_path == "pull_request_template.md"
        || normalized_path.contains(".github/PULL_REQUEST_TEMPLATE")
            && normalized_path.ends_with(".md")
        || normalized_path.contains(".github/pull_request_template")
            && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/PULL_REQUEST_TEMPLATE")
            && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/pull_request_template")
            && normalized_path.ends_with(".md")
}

fn is_valid_review_template_path_github(path: &path::Path) -> bool {
    is_review_template_github(path.to_str().unwrap_or_default())
}

fn get_gitlab_directory_path(root_path: &path::Path) -> path::PathBuf {
    let mut path = root_path.to_path_buf();
    path.push(".gitlab");
    path
}

fn is_review_template_gitlab(path_str: &str) -> bool {
    let normalized_path = path_str.replace('\\', "/");
    normalized_path.contains(".gitlab/merge_request_templates/") && normalized_path.ends_with(".md")
}

fn is_valid_review_template_path_gitlab(path: &path::Path) -> bool {
    is_review_template_gitlab(path.to_str().unwrap_or_default())
}

fn get_bitbucket_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_bitbucket(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn is_valid_review_template_path_bitbucket(_path: &path::Path) -> bool {
    // TODO: implement
    false
}

fn get_azure_directory_path(root_path: &path::Path) -> path::PathBuf {
    // TODO: implement
    root_path.to_path_buf()
}

fn is_review_template_azure(_path_str: &str) -> bool {
    // TODO: implement
    false
}

fn is_valid_review_template_path_azure(_path: &path::Path) -> bool {
    // TODO: implement
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewLabel {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewLabel);

impl From<but_github::GitHubPrLabel> for ForgeReviewLabel {
    fn from(label: but_github::GitHubPrLabel) -> Self {
        ForgeReviewLabel {
            name: label.name,
            description: label.description,
            color: label.color,
        }
    }
}

impl From<but_gitlab::GitLabLabel> for ForgeReviewLabel {
    fn from(label: but_gitlab::GitLabLabel) -> Self {
        ForgeReviewLabel {
            name: label.name,
            description: None,
            color: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
/// Represents a user from a forge platform (e.g., GitHub, GitLab).
///
/// This structure contains information about a user account on a forge platform,
/// including their identification details and profile information.
pub struct ForgeReviewUser {
    /// The unique numeric identifier for the user on the forge platform
    pub id: i64,
    /// The user's login username
    pub login: String,
    /// The user's display name, if available
    pub name: Option<String>,
    /// The user's email address, if publicly available
    pub email: Option<String>,
    /// URL to the user's profile avatar image, if available
    pub avatar_url: Option<String>,
    /// Indicates whether this account is a bot account
    pub is_bot: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewUser);

impl Display for ForgeReviewUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "login: {}, name: {} ",
            self.login,
            self.name.as_deref().unwrap_or("N/A")
        )
    }
}

impl From<but_github::GitHubUser> for ForgeReviewUser {
    fn from(user: but_github::GitHubUser) -> Self {
        ForgeReviewUser {
            id: user.id,
            login: user.login,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.is_bot,
        }
    }
}

impl From<but_gitlab::GitLabUser> for ForgeReviewUser {
    fn from(user: but_gitlab::GitLabUser) -> Self {
        ForgeReviewUser {
            id: user.id,
            login: user.username,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.is_bot,
        }
    }
}

impl From<but_bitbucket::BitbucketUser> for ForgeReviewUser {
    fn from(user: but_bitbucket::BitbucketUser) -> Self {
        ForgeReviewUser {
            id: user.id,
            login: user.username,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.is_bot,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
/// Represents a review (pull request/merge request) from a forge platform (GitHub, GitLab, etc.).
///
/// Contains metadata and state information about a code review, including its location,
/// participants, labels, and timestamps for various lifecycle events.
pub struct ForgeReview {
    /// The URL to view this review in a web browser
    pub html_url: String,
    /// The unique identifier number for this review within its repository.
    /// This can be a PR or MR number.
    pub number: i64,
    /// The title/summary of the review
    pub title: String,
    /// The detailed description or body text of the review, if provided.
    pub body: Option<String>,
    /// The user who created this review.
    pub author: Option<ForgeReviewUser>,
    /// Labels or tags applied to categorize this review.
    pub labels: Vec<ForgeReviewLabel>,
    /// Whether this review is in draft state (not ready for final review).
    pub draft: bool,
    /// The name of the branch containing the proposed changes.
    /// This is the short name of the branch (e.g., "feature-branch")
    pub source_branch: String,
    /// The name of the branch that will receive the changes when merged.
    /// This is the short name of the branch (e.g., "main" or "develop")
    pub target_branch: String,
    /// The git commit SHA that this review is based on.
    pub sha: String,
    /// Commits on the target branch that represent this review having landed.
    pub integration_commit_shas: Vec<String>,
    /// ISO 8601 timestamp of when the review was created.
    pub created_at: Option<String>,
    /// ISO 8601 timestamp of when the review was last modified.
    pub modified_at: Option<String>,
    /// ISO 8601 timestamp of when the review was merged, if applicable.
    pub merged_at: Option<String>,
    /// ISO 8601 timestamp of when the review was closed, if applicable.
    pub closed_at: Option<String>,
    /// SSH URL for cloning the repository containing this review.
    pub repository_ssh_url: Option<String>,
    /// HTTPS URL for cloning the repository containing this review.
    pub repository_https_url: Option<String>,
    /// The owner (user or organization) of the repository from which the branch originates.
    /// In the case of a fork, this will be the fork owner's username.
    pub repo_owner: Option<String>,
    /// Whether the source/head repository for this review is a fork.
    pub head_repo_is_fork: bool,
    /// Users who have been requested to review or have reviewed this code.
    pub reviewers: Vec<ForgeReviewUser>,
    /// Whether auto-merge (merge once the forge's requirements pass) is enabled.
    pub auto_merge_enabled: bool,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for MRs).
    pub unit_symbol: String,
    /// The timestamp when this review was last fetched from the forge.
    pub last_sync_at: chrono::NaiveDateTime,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReview);

impl ForgeReview {
    /// Whether the review is still open (not merged or closed)
    pub fn is_open(&self) -> bool {
        self.merged_at.is_none() && self.closed_at.is_none()
    }

    /// Whether the review has been merged
    pub fn is_merged(&self) -> bool {
        self.merged_at.is_some()
    }

    /// Whether the review points to the given commit ID and has been merged
    pub fn is_merged_at_commit(&self, commit_id: &str) -> bool {
        self.is_merged() && self.sha == commit_id
    }

    /// The struct version for persistence compatibility purposes
    pub fn struct_version() -> i32 {
        4
    }
}

impl Display for ForgeReview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}: {}\n - author: {}\n - description: {}\n - created at: {}\n",
            self.unit_symbol,
            self.number,
            self.title,
            self.author
                .as_ref()
                .map(|a| a.to_string())
                .unwrap_or("-unknown-".to_string()),
            self.body.as_deref().unwrap_or("-no description-"),
            self.created_at.as_deref().unwrap_or("-unknown-"),
        )
    }
}

impl From<but_github::PullRequest> for ForgeReview {
    fn from(pr: but_github::PullRequest) -> Self {
        ForgeReview {
            html_url: pr.html_url,
            number: pr.number,
            title: pr.title,
            body: pr.body,
            author: pr.author.map(ForgeReviewUser::from),
            labels: pr.labels.into_iter().map(ForgeReviewLabel::from).collect(),
            draft: pr.draft,
            source_branch: pr.source_branch,
            target_branch: pr.target_branch,
            sha: pr.sha,
            integration_commit_shas: pr.integration_commit_shas,
            created_at: pr.created_at,
            modified_at: pr.modified_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: pr.repository_ssh_url,
            repository_https_url: pr.repository_https_url,
            repo_owner: pr.repo_owner,
            head_repo_is_fork: pr.head_repo_is_fork,
            reviewers: pr
                .requested_reviewers
                .into_iter()
                .map(ForgeReviewUser::from)
                .collect(),
            auto_merge_enabled: pr.auto_merge_enabled,
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

impl From<but_gitlab::MergeRequest> for ForgeReview {
    fn from(mr: but_gitlab::MergeRequest) -> Self {
        ForgeReview {
            html_url: mr.web_url,
            number: mr.iid,
            title: mr.title,
            body: mr.description,
            author: mr.author.map(ForgeReviewUser::from),
            labels: mr.labels.into_iter().map(ForgeReviewLabel::from).collect(),
            draft: mr.draft,
            source_branch: mr.source_branch,
            target_branch: mr.target_branch,
            sha: mr.sha,
            integration_commit_shas: mr.integration_commit_shas,
            created_at: mr.created_at,
            modified_at: mr.updated_at,
            merged_at: mr.merged_at,
            closed_at: mr.closed_at,
            repository_ssh_url: mr.repository_ssh_url,
            repository_https_url: mr.repository_https_url,
            repo_owner: mr.repo_owner,
            head_repo_is_fork: mr.source_project_is_fork,
            reviewers: mr
                .reviewers
                .into_iter()
                .map(ForgeReviewUser::from)
                .collect(),
            auto_merge_enabled: mr.auto_merge_enabled,
            unit_symbol: "!".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

impl From<but_bitbucket::BitbucketPullRequest> for ForgeReview {
    fn from(pr: but_bitbucket::BitbucketPullRequest) -> Self {
        let merged_at = pr.merged_at();
        let closed_at = pr.closed_at();
        let integration_commit_shas = pr.merge_commit_hash.clone().into_iter().collect();
        ForgeReview {
            html_url: pr.html_url,
            number: pr.id,
            title: pr.title,
            body: pr.description,
            author: pr.author.map(ForgeReviewUser::from),
            // Bitbucket Cloud pull requests don't carry labels.
            labels: Vec::new(),
            draft: pr.draft,
            source_branch: pr.source_branch,
            target_branch: pr.target_branch,
            sha: pr.source_commit_hash,
            integration_commit_shas,
            created_at: pr.created_on,
            modified_at: pr.updated_on,
            merged_at,
            closed_at,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: pr.repo_owner,
            head_repo_is_fork: pr.head_repo_is_fork,
            reviewers: pr
                .reviewers
                .into_iter()
                .map(ForgeReviewUser::from)
                .collect(),
            // Bitbucket Cloud has no auto-merge.
            auto_merge_enabled: false,
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum CacheConfig {
    CacheOnly,
    CacheWithFallback {
        max_age_seconds: u64,
    },
    #[default]
    NoCache,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CacheConfig);

/// List the open reviews (e.g. pull requests) for a given forge repository
pub fn list_forge_reviews_with_cache(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
    db: &mut but_db::DbHandle,
    cache_config: Option<CacheConfig>,
) -> Result<Vec<ForgeReview>> {
    let cache_config = cache_config.unwrap_or_default();
    let reviews = match cache_config {
        CacheConfig::CacheOnly => crate::list_cached_forge_reviews(db)?,
        CacheConfig::CacheWithFallback { max_age_seconds } => {
            let cached = crate::db::reviews_from_cache(db)?;
            if let Some(reviews) =
                cached.fresh_rows(max_age_seconds, chrono::Local::now().naive_local())
            {
                return Ok(reviews);
            }
            let reviews = list_forge_reviews(preferred_forge_user, forge_repo_info, storage)?;
            crate::db::cache_reviews(db, &reviews).ok();
            reviews
        }
        CacheConfig::NoCache => {
            let reviews = list_forge_reviews(preferred_forge_user, forge_repo_info, storage)?;
            crate::db::cache_reviews(db, &reviews).ok();
            reviews
        }
    };
    Ok(reviews)
}

/// Optimistically cache a single review — e.g. one just created via
/// [`create_forge_review`] — so it appears in cache-only projections before the
/// next full review-list sync.
///
/// Unlike [`list_forge_reviews_with_cache`], this upserts a single row and never
/// deletes other cached reviews. The reconcile pass protects such a freshly
/// written row from deletion for a short grace window, so it survives even if the
/// forge's own list endpoint hasn't caught up to the new review yet.
pub fn cache_review(db: &mut but_db::DbHandle, review: &ForgeReview) -> Result<()> {
    crate::db::upsert_review(db, review)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForgeAccountValidity {
    Valid,
    Invalid,
    NoCredentials,
}

impl From<but_github::CredentialCheckResult> for ForgeAccountValidity {
    fn from(value: but_github::CredentialCheckResult) -> Self {
        match value {
            CredentialCheckResult::Invalid => ForgeAccountValidity::Invalid,
            CredentialCheckResult::NoCredentials => ForgeAccountValidity::NoCredentials,
            CredentialCheckResult::Valid => ForgeAccountValidity::Valid,
        }
    }
}

impl From<but_gitlab::CredentialCheckResult> for ForgeAccountValidity {
    fn from(value: but_gitlab::CredentialCheckResult) -> Self {
        match value {
            but_gitlab::CredentialCheckResult::Invalid => ForgeAccountValidity::Invalid,
            but_gitlab::CredentialCheckResult::NoCredentials => ForgeAccountValidity::NoCredentials,
            but_gitlab::CredentialCheckResult::Valid => ForgeAccountValidity::Valid,
        }
    }
}

impl From<but_bitbucket::CredentialCheckResult> for ForgeAccountValidity {
    fn from(value: but_bitbucket::CredentialCheckResult) -> Self {
        match value {
            but_bitbucket::CredentialCheckResult::Invalid => ForgeAccountValidity::Invalid,
            but_bitbucket::CredentialCheckResult::NoCredentials => {
                ForgeAccountValidity::NoCredentials
            }
            but_bitbucket::CredentialCheckResult::Valid => ForgeAccountValidity::Valid,
        }
    }
}

/// Check whether there's an account that would be used for this repository is authenticated.
pub async fn check_forge_account_is_valid(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeAccountValidity> {
    match forge_repo_info.forge {
        ForgeName::GitHub => {
            let preferred_account = match preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned())
            {
                Some(account) => account,
                None => {
                    let known_accounts = but_github::list_known_github_accounts(storage)?;
                    match known_accounts.first() {
                        Some(account) => account.clone(),
                        None => {
                            return Ok(ForgeAccountValidity::NoCredentials);
                        }
                    }
                }
            };

            but_github::check_credentials(&preferred_account, storage)
                .await
                .map(Into::into)
        }
        ForgeName::GitLab => {
            let preferred_account = match preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab().cloned())
            {
                Some(account) => account,
                None => {
                    let known_accounts = but_gitlab::list_known_gitlab_accounts(storage)?;
                    match known_accounts.first() {
                        Some(account) => account.clone(),
                        None => {
                            return Ok(ForgeAccountValidity::NoCredentials);
                        }
                    }
                }
            };

            but_gitlab::check_credentials(&preferred_account, storage)
                .await
                .map(Into::into)
        }
        ForgeName::Bitbucket => {
            let preferred_account = match preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket().cloned())
            {
                Some(account) => account,
                None => {
                    let known_accounts = but_bitbucket::list_known_bitbucket_accounts(storage)?;
                    match known_accounts.first() {
                        Some(account) => account.clone(),
                        None => {
                            return Ok(ForgeAccountValidity::NoCredentials);
                        }
                    }
                }
            };

            but_bitbucket::check_credentials(&preferred_account, storage)
                .await
                .map(Into::into)
        }
        _ => Err(Error::msg(format!(
            "Checking reviews for forge {:?} is not implemented yet",
            forge_repo_info.forge
        ))),
    }
}

fn list_forge_reviews(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReview>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    let reviews = match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());

            // Clone owned data for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let storage = storage.clone();

            let pulls = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(but_github::pr::list(
                        preferred_account.as_ref(),
                        &owner,
                        &repo,
                        &storage,
                    ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            pulls
                .into_iter()
                .map(ForgeReview::from)
                .collect::<Vec<ForgeReview>>()
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab().cloned());

            // Clone owned data for thread
            let project_id = GitLabProjectId::new(owner, repo);
            let storage = storage.clone();

            let mrs = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(but_gitlab::mr::list(
                        preferred_account.as_ref(),
                        project_id,
                        &storage,
                    ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            mrs.into_iter()
                .map(ForgeReview::from)
                .collect::<Vec<ForgeReview>>()
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket().cloned());

            // Clone owned data for thread
            let workspace = owner.clone();
            let repo_slug = repo.clone();
            let storage = storage.clone();

            let prs = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(but_bitbucket::pr::list(
                        preferred_account.as_ref(),
                        &workspace,
                        &repo_slug,
                        &storage,
                    ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            prs.into_iter()
                .map(ForgeReview::from)
                .collect::<Vec<ForgeReview>>()
        }
        _ => {
            return Err(Error::msg(format!(
                "Listing reviews for forge {forge:?} is not implemented yet.",
            )));
        }
    };
    Ok(reviews)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ForgeReviewFilter {
    Today,
    ThisWeek,
    ThisMonth,
    #[default]
    All,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewFilter);

pub async fn list_forge_reviews_for_branch(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    branch: &str,
    storage: &but_forge_storage::Controller,
    filter: Option<ForgeReviewFilter>,
) -> Result<Vec<ForgeReview>> {
    let filter = filter.unwrap_or_default();
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());
            let prs = but_github::pr::list_all_for_branch(
                preferred_account.as_ref(),
                owner,
                repo,
                branch,
                storage,
            )
            .await?;

            let prs = filter_prs(prs, &filter);

            Ok(prs.into_iter().map(ForgeReview::from).collect())
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab().cloned());
            let project_id = GitLabProjectId::new(owner, repo);
            let mrs = but_gitlab::mr::list_all_for_target(
                preferred_account.as_ref(),
                project_id,
                branch,
                storage,
            )
            .await?;
            let mrs = filter_mrs(mrs, &filter);
            Ok(mrs.into_iter().map(ForgeReview::from).collect())
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket().cloned());
            let prs = but_bitbucket::pr::list_all_for_target(
                preferred_account.as_ref(),
                owner,
                repo,
                branch,
                storage,
            )
            .await?;
            let prs = filter_bb_prs(prs, &filter);
            Ok(prs.into_iter().map(ForgeReview::from).collect())
        }
        _ => Err(Error::msg(format!(
            "Listing reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

fn filter_prs(
    prs: Vec<but_github::PullRequest>,
    filter: &ForgeReviewFilter,
) -> Vec<but_github::PullRequest> {
    let now = chrono::Utc::now();
    prs.into_iter()
        .filter(|pr| {
            if pr.merged_at.is_none() {
                return false;
            }
            match filter {
                ForgeReviewFilter::Today => {
                    if let Some(merged_at_str) = &pr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        return merged_at.date_naive() == now.date_naive();
                    }
                    false
                }
                ForgeReviewFilter::ThisWeek => {
                    if let Some(merged_at_str) = &pr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        let week_start = now
                            - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
                        return merged_at.date_naive() >= week_start.date_naive();
                    }
                    false
                }
                ForgeReviewFilter::ThisMonth => {
                    if let Some(merged_at_str) = &pr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        return merged_at.year() == now.year() && merged_at.month() == now.month();
                    }
                    false
                }
                ForgeReviewFilter::All => true,
            }
        })
        .collect()
}

fn filter_mrs(
    mrs: Vec<but_gitlab::MergeRequest>,
    filter: &ForgeReviewFilter,
) -> Vec<but_gitlab::MergeRequest> {
    let now = chrono::Utc::now();
    mrs.into_iter()
        .filter(|mr| {
            if mr.merged_at.is_none() {
                return false;
            }
            match filter {
                ForgeReviewFilter::Today => {
                    if let Some(merged_at_str) = &mr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        return merged_at.date_naive() == now.date_naive();
                    }
                    false
                }
                ForgeReviewFilter::ThisWeek => {
                    if let Some(merged_at_str) = &mr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        let week_start = now
                            - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
                        return merged_at.date_naive() >= week_start.date_naive();
                    }
                    false
                }
                ForgeReviewFilter::ThisMonth => {
                    if let Some(merged_at_str) = &mr.merged_at
                        && let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(merged_at_str)
                    {
                        return merged_at.year() == now.year() && merged_at.month() == now.month();
                    }
                    false
                }
                ForgeReviewFilter::All => true,
            }
        })
        .collect()
}

fn filter_bb_prs(
    prs: Vec<but_bitbucket::BitbucketPullRequest>,
    filter: &ForgeReviewFilter,
) -> Vec<but_bitbucket::BitbucketPullRequest> {
    let now = chrono::Utc::now();
    prs.into_iter()
        .filter(|pr| {
            let Some(merged_at_str) = pr.merged_at() else {
                return false;
            };
            let Ok(merged_at) = chrono::DateTime::parse_from_rfc3339(&merged_at_str) else {
                return false;
            };
            match filter {
                ForgeReviewFilter::Today => merged_at.date_naive() == now.date_naive(),
                ForgeReviewFilter::ThisWeek => {
                    let week_start =
                        now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
                    merged_at.date_naive() >= week_start.date_naive()
                }
                ForgeReviewFilter::ThisMonth => {
                    merged_at.year() == now.year() && merged_at.month() == now.month()
                }
                ForgeReviewFilter::All => true,
            }
        })
        .collect()
}

async fn get_forge_review_inner(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReview> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr =
                but_github::pr::get(preferred_account, owner, repo, review_number, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr =
                but_gitlab::mr::get(preferred_account, project_id, review_number, storage).await?;
            Ok(ForgeReview::from(mr))
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            let pr = but_bitbucket::pr::get(preferred_account, owner, repo, review_number, storage)
                .await?;
            Ok(ForgeReview::from(pr))
        }
        _ => Err(Error::msg(format!(
            "Getting reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

/// Forge-agnostic runtime merge state for a review. Always fetched
/// fresh from the forge; not cached. Used by the UI to render the
/// merge-button hint and comment count without forcing every review
/// consumer to subscribe to those expensive fields.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviewMergeStatus {
    /// Forge-reported merge state. GitHub strings: `clean`, `dirty`,
    /// `unknown`, `blocked`, `behind`, `unstable`, `has_hooks`,
    /// `draft`. GitLab strings: `can_be_merged`, `cannot_be_merged`,
    /// `checking`, etc. `None` when the forge hasn't computed it.
    /// Used by the UI only to surface a specific reason tooltip.
    pub mergeable_state: Option<String>,
    pub comments_count: i64,
    /// Forge-normalized: whether merging is allowed. Drives the merge
    /// button without forcing the UI to know per-forge state strings.
    pub is_mergeable: bool,
}

/// A top-level comment on a review's conversation thread. Fetched fresh
/// from the forge; not cached. Diff-anchored review comments are not
/// part of this type.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewComment {
    /// Forge-assigned identifier of the comment.
    pub id: i64,
    /// The comment text, as forge-flavored markdown.
    pub body: String,
    /// The comment's author.
    pub author: Option<ForgeReviewUser>,
    /// ISO 8601 timestamp of when the comment was created.
    pub created_at: Option<String>,
    /// ISO 8601 timestamp of the comment's last edit.
    pub modified_at: Option<String>,
    /// The URL to view this comment in a web browser.
    pub html_url: String,
    /// Reaction tallies on this comment, nonzero kinds only.
    pub reactions: Vec<ForgeReviewReactionCount>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewComment);

/// One reaction kind's tally. `kind` is the forge's native reaction name
/// (GitHub: `+1`, `laugh`, …) — an open set, since forges like GitLab
/// allow arbitrary award emoji.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewReactionCount {
    pub kind: String,
    pub count: i64,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewReactionCount);

fn github_reaction_counts(
    reactions: but_github::CommentReactions,
) -> Vec<ForgeReviewReactionCount> {
    [
        ("+1", reactions.plus_one),
        ("-1", reactions.minus_one),
        ("laugh", reactions.laugh),
        ("confused", reactions.confused),
        ("heart", reactions.heart),
        ("hooray", reactions.hooray),
        ("rocket", reactions.rocket),
        ("eyes", reactions.eyes),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(kind, count)| ForgeReviewReactionCount {
        kind: kind.to_owned(),
        count,
    })
    .collect()
}

impl From<but_github::PullRequestComment> for ForgeReviewComment {
    fn from(comment: but_github::PullRequestComment) -> Self {
        ForgeReviewComment {
            id: comment.id,
            body: comment.body,
            author: comment.author.map(ForgeReviewUser::from),
            created_at: comment.created_at,
            modified_at: comment.modified_at,
            html_url: comment.html_url,
            reactions: github_reaction_counts(comment.reactions),
        }
    }
}

/// List the labels defined on the repository backing a review.
pub async fn list_repo_labels(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewLabel>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let labels =
                but_github::pr::list_repo_labels(preferred_account, owner, repo, storage).await?;
            Ok(labels.into_iter().map(Into::into).collect())
        }
        _ => Err(anyhow::anyhow!(
            "Repository labels for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Add labels to a review; returns the resulting label set.
pub async fn add_review_labels(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    labels: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewLabel>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let labels = but_github::pr::add_labels(
                preferred_account,
                owner,
                repo,
                review_number,
                labels,
                storage,
            )
            .await?;
            Ok(labels.into_iter().map(Into::into).collect())
        }
        _ => Err(anyhow::anyhow!(
            "Review labels for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Remove one label from a review.
pub async fn remove_review_label(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    label: &str,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::remove_label(
                preferred_account,
                owner,
                repo,
                review_number,
                label,
                storage,
            )
            .await
        }
        _ => Err(anyhow::anyhow!(
            "Review labels for forge {forge:?} are not implemented yet."
        )),
    }
}

/// List users who can be requested to review on the repository.
pub async fn list_reviewer_candidates(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewUser>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let users =
                but_github::pr::list_reviewer_candidates(preferred_account, owner, repo, storage)
                    .await?;
            Ok(users.into_iter().map(Into::into).collect())
        }
        _ => Err(anyhow::anyhow!(
            "Reviewer candidates for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Request reviews from the given users on a review.
pub async fn request_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    logins: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::request_reviewers(
                preferred_account,
                owner,
                repo,
                review_number,
                logins,
                storage,
            )
            .await
        }
        _ => Err(anyhow::anyhow!(
            "Review requests for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Withdraw review requests for the given users on a review.
pub async fn withdraw_review_request(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    logins: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::remove_requested_reviewers(
                preferred_account,
                owner,
                repo,
                review_number,
                logins,
                storage,
            )
            .await
        }
        _ => Err(anyhow::anyhow!(
            "Review requests for forge {forge:?} are not implemented yet."
        )),
    }
}

/// The verdict a submitted review carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ForgeReviewSubmissionState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewSubmissionState);

/// A submitted review (approval, change request, or review comment) on a
/// review. Fetched fresh from the forge; not cached. The caller's own
/// unsubmitted (pending) drafts are excluded.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewSubmission {
    /// Forge-assigned identifier of the submission.
    pub id: i64,
    /// Who submitted the review.
    pub author: Option<ForgeReviewUser>,
    /// The verdict of this submission.
    pub state: ForgeReviewSubmissionState,
    /// The summary text accompanying the submission, if any.
    pub body: Option<String>,
    /// ISO 8601 timestamp of when the review was submitted.
    pub submitted_at: Option<String>,
    /// The URL to view this submission in a web browser.
    pub html_url: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewSubmission);

/// Edit a top-level conversation comment. The forge enforces permissions;
/// the UI additionally only offers this on the caller's own comments.
pub async fn update_review_comment(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    comment_id: i64,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReviewComment> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let comment = but_github::pr::update_comment(
                preferred_account,
                owner,
                repo,
                comment_id,
                body,
                storage,
            )
            .await?;
            Ok(comment.into())
        }
        _ => Err(anyhow::anyhow!(
            "Review comments for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Delete a top-level conversation comment.
pub async fn delete_review_comment(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    comment_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::delete_comment(preferred_account, owner, repo, comment_id, storage)
                .await
        }
        _ => Err(anyhow::anyhow!(
            "Review comments for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Map GitHub's review-state string to the forge-agnostic verdict.
/// `PENDING` (the caller's own unsubmitted draft) and unknown future states
/// map to `None` and are omitted from listings.
fn github_submission_state(raw: &str) -> Option<ForgeReviewSubmissionState> {
    match raw {
        "APPROVED" => Some(ForgeReviewSubmissionState::Approved),
        "CHANGES_REQUESTED" => Some(ForgeReviewSubmissionState::ChangesRequested),
        "COMMENTED" => Some(ForgeReviewSubmissionState::Commented),
        "DISMISSED" => Some(ForgeReviewSubmissionState::Dismissed),
        _ => None,
    }
}

/// List the submitted reviews on a review, oldest first. Each call hits
/// the forge fresh (no DB cache).
pub async fn list_review_submissions(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewSubmission>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let reviews = but_github::pr::list_pr_reviews(
                preferred_account,
                owner,
                repo,
                review_number,
                storage,
            )
            .await?;
            Ok(reviews
                .into_iter()
                .filter_map(|review| {
                    let state = github_submission_state(&review.state)?;
                    Some(ForgeReviewSubmission {
                        id: review.id,
                        author: review.author.map(ForgeReviewUser::from),
                        state,
                        body: review.body.filter(|body| !body.trim().is_empty()),
                        submitted_at: review.submitted_at,
                        html_url: review.html_url,
                    })
                })
                .collect())
        }
        // Read as empty rather than erroring; see list_review_comments.
        _ => Ok(Vec::new()),
    }
}

/// List the top-level conversation comments on a review, oldest first.
/// Each call hits the forge fresh (no DB cache).
pub async fn list_review_comments(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewComment>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let comments = but_github::pr::list_comments(
                preferred_account,
                owner,
                repo,
                review_number,
                storage,
            )
            .await?;
            Ok(comments.into_iter().map(Into::into).collect())
        }
        // Read as empty rather than erroring: the UI polls this for every
        // open review, and a forge without comment support shouldn't turn
        // that into a permanent failure loop.
        _ => Ok(Vec::new()),
    }
}

/// One individual reaction, with who left it and the forge id that
/// addresses its removal. `kind` is the forge's native reaction name — an
/// open set; unknown kinds pass through rather than being dropped.
/// Fetched fresh; not cached.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewReaction {
    pub id: i64,
    pub kind: String,
    pub user: Option<ForgeReviewUser>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewReaction);

fn github_reactions(reactions: Vec<but_github::Reaction>) -> Vec<ForgeReviewReaction> {
    reactions.into_iter().map(github_reaction).collect()
}

fn github_reaction(reaction: but_github::Reaction) -> ForgeReviewReaction {
    ForgeReviewReaction {
        id: reaction.id,
        kind: reaction.content,
        user: reaction.user.map(ForgeReviewUser::from),
    }
}

/// List the individual reactions on the review itself.
pub async fn list_review_reactions(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewReaction>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let reactions = but_github::pr::list_review_reactions(
                preferred_account,
                owner,
                repo,
                review_number,
                storage,
            )
            .await?;
            Ok(github_reactions(reactions))
        }
        // Read as empty rather than erroring; see list_review_comments.
        _ => Ok(Vec::new()),
    }
}

/// List the individual reactions on one conversation comment.
pub async fn list_comment_reactions(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    comment_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewReaction>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let reactions = but_github::pr::list_comment_reactions(
                preferred_account,
                owner,
                repo,
                comment_id,
                storage,
            )
            .await?;
            Ok(github_reactions(reactions))
        }
        // Read as empty rather than erroring; see list_review_comments.
        _ => Ok(Vec::new()),
    }
}

/// Add the caller's reaction to the review itself. Idempotent per kind;
/// the forge rejects kinds it doesn't support.
pub async fn add_review_reaction(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    kind: &str,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReviewReaction> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let reaction = but_github::pr::add_review_reaction(
                preferred_account,
                owner,
                repo,
                review_number,
                kind,
                storage,
            )
            .await?;
            Ok(github_reaction(reaction))
        }
        _ => Err(anyhow::anyhow!(
            "Reactions for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Remove one of the caller's reactions from the review itself.
pub async fn remove_review_reaction(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    reaction_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::remove_review_reaction(
                preferred_account,
                owner,
                repo,
                review_number,
                reaction_id,
                storage,
            )
            .await
        }
        _ => Err(anyhow::anyhow!(
            "Reactions for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Add the caller's reaction to one conversation comment.
pub async fn add_comment_reaction(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    comment_id: i64,
    kind: &str,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReviewReaction> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let reaction = but_github::pr::add_comment_reaction(
                preferred_account,
                owner,
                repo,
                comment_id,
                kind,
                storage,
            )
            .await?;
            Ok(github_reaction(reaction))
        }
        _ => Err(anyhow::anyhow!(
            "Reactions for forge {forge:?} are not implemented yet."
        )),
    }
}

/// Remove one of the caller's reactions from one conversation comment.
pub async fn remove_comment_reaction(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    comment_id: i64,
    reaction_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            but_github::pr::remove_comment_reaction(
                preferred_account,
                owner,
                repo,
                comment_id,
                reaction_id,
                storage,
            )
            .await
        }
        _ => Err(anyhow::anyhow!(
            "Reactions for forge {forge:?} are not implemented yet."
        )),
    }
}

/// What a non-comment timeline row represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ForgeReviewTimelineEventKind {
    Committed,
    ReviewRequested,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewTimelineEventKind);

/// A non-comment row of a review's conversation timeline: a pushed commit
/// or a review request. Fetched fresh from the forge; not cached. Commit
/// rows carry the git author name (not a forge user); review requests
/// carry the requesting and requested users.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewTimelineEvent {
    pub kind: ForgeReviewTimelineEventKind,
    pub actor: Option<ForgeReviewUser>,
    pub requested_reviewer: Option<ForgeReviewUser>,
    pub commit_sha: Option<String>,
    pub commit_summary: Option<String>,
    pub commit_author_name: Option<String>,
    /// ISO 8601 timestamp of the event.
    pub created_at: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewTimelineEvent);

/// List the pushed commits and review requests on a review's conversation
/// timeline, oldest first. Each call hits the forge fresh (no DB cache).
pub async fn list_review_timeline_events(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReviewTimelineEvent>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let events = but_github::pr::list_timeline_events(
                preferred_account,
                owner,
                repo,
                review_number,
                storage,
            )
            .await?;
            Ok(events
                .into_iter()
                .map(|event| ForgeReviewTimelineEvent {
                    kind: match event.kind {
                        but_github::PullRequestTimelineEventKind::Committed => {
                            ForgeReviewTimelineEventKind::Committed
                        }
                        but_github::PullRequestTimelineEventKind::ReviewRequested => {
                            ForgeReviewTimelineEventKind::ReviewRequested
                        }
                    },
                    actor: event.actor.map(ForgeReviewUser::from),
                    requested_reviewer: event.requested_reviewer.map(ForgeReviewUser::from),
                    commit_sha: event.commit_sha,
                    commit_summary: event.commit_summary,
                    commit_author_name: event.commit_author_name,
                    created_at: event.created_at,
                })
                .collect())
        }
        // Read as empty rather than erroring; see list_review_comments.
        _ => Ok(Vec::new()),
    }
}

/// Post a top-level conversation comment on a review.
pub async fn create_review_comment(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReviewComment> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let comment = but_github::pr::create_comment(
                preferred_account,
                owner,
                repo,
                review_number,
                body,
                storage,
            )
            .await?;
            Ok(comment.into())
        }
        _ => Err(anyhow::anyhow!(
            "Review comments for forge {forge:?} are not implemented yet."
        )),
    }
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ReviewMergeStatus);

/// Canonical clone URL for a review's base repo (the repo the PR
/// targets). `None` for forges without a URL-based fork model.
pub async fn get_review_base_repo_url(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Option<String>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            but_github::GitHubClient::from_storage(storage, preferred_account)?
                .get_pull_request_base_repo_url(owner, repo, pr_number)
                .await
                .context("Failed to fetch PR base repo URL")
        }
        // None tells the UI to fall back to a branch-name-only check.
        ForgeName::GitLab | ForgeName::Bitbucket | ForgeName::Azure => Ok(None),
    }
}

/// Fetch the runtime merge state for a review. Each call hits the
/// forge fresh (no DB cache) so the UI can render up-to-date hints
/// without paying for the rest of the review payload.
pub async fn get_review_merge_status(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<ReviewMergeStatus> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let status = but_github::GitHubClient::from_storage(storage, preferred_account)?
                .get_pull_request_merge_status(owner, repo, pr_number)
                .await
                .context("Failed to fetch PR merge status")?;
            Ok(ReviewMergeStatus {
                mergeable_state: status.mergeable_state,
                comments_count: status.comments_count,
                is_mergeable: status.is_mergeable,
            })
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr_iid = review_number
                .try_into()
                .context("MR: Failed to cast usize to i64, somehow")?;
            let status = but_gitlab::GitLabClient::from_storage(storage, preferred_account)?
                .get_merge_request_merge_status(project_id, mr_iid)
                .await
                .context("Failed to fetch MR merge status")?;
            Ok(ReviewMergeStatus {
                mergeable_state: status.mergeable_state,
                comments_count: status.comments_count,
                is_mergeable: status.is_mergeable,
            })
        }
        ForgeName::Bitbucket => {
            // Bitbucket has no dedicated mergeability endpoint; derive a basic
            // status from the PR's state and comment count.
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            let pr = but_bitbucket::pr::get(preferred_account, owner, repo, review_number, storage)
                .await?;
            Ok(ReviewMergeStatus {
                is_mergeable: pr.is_open(),
                // Bitbucket's state strings ("OPEN"/"MERGED"/…) don't map to the
                // forge-agnostic mergeable_state vocabulary the UI understands, so
                // leave it unset rather than feed a value it can't interpret.
                mergeable_state: None,
                comments_count: pr.comment_count,
            })
        }
        _ => Err(anyhow::anyhow!(
            "Merge status for forge {forge:?} is not implemented yet."
        )),
    }
}

/// Get a specific review (e.g. pull request) for a given forge repository
///
/// The resulting review will be cached.
pub fn get_forge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    db: &mut but_db::DbHandle,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReview> {
    let preferred_forge_user = preferred_forge_user.clone();
    let forge_repo_info = forge_repo_info.clone();
    let storage = storage.clone();

    let review = std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| {
            anyhow::anyhow!(
                "Failed fetch review {review_number}: failed to create Tokio runtime: {e}"
            )
        })?;

        runtime.block_on(get_forge_review_inner(
            &preferred_forge_user,
            &forge_repo_info,
            review_number,
            &storage,
        ))
    })
    .join()
    .map_err(|e| {
        anyhow::anyhow!("Failed to fetch review {review_number}: thread panicked: {e:?}")
    })??;

    // Cache the review and ignore any issues, if any.
    crate::db::upsert_review(db, &review).ok();
    Ok(review)
}

/// How to merge a review on the forge. GitHub honours all three;
/// other forges fall back to their default merge strategy when the
/// caller asks for `Squash`/`Rebase`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ReviewMergeMethod {
    #[default]
    Merge,
    Squash,
    Rebase,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ReviewMergeMethod);

impl From<&ReviewMergeMethod> for but_github::MergeMethod {
    fn from(value: &ReviewMergeMethod) -> Self {
        match value {
            ReviewMergeMethod::Merge => but_github::MergeMethod::Merge,
            ReviewMergeMethod::Squash => but_github::MergeMethod::Squash,
            ReviewMergeMethod::Rebase => but_github::MergeMethod::Rebase,
        }
    }
}

/// The values to update for an existing review.  Each `Some` is applied;
/// `None` leaves the field unchanged on the forge.
pub struct ReviewUpdatePayload {
    title: Option<String>,
    body: Option<String>,
    state: Option<ReviewState>,
    target_base: Option<String>,
}

impl ReviewUpdatePayload {
    /// Create a new instance of the parameters of the review to update.
    pub fn new(
        title: Option<String>,
        body: Option<String>,
        state: Option<ReviewState>,
        target_base: Option<String>,
    ) -> Self {
        Self {
            title,
            body,
            state,
            target_base,
        }
    }
}

/// Update arbitrary fields of a single review.
pub async fn update_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    payload: ReviewUpdatePayload,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let ReviewUpdatePayload {
        title,
        body,
        state,
        target_base,
    } = payload;
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let state_str = state.as_ref().map(|s| s.as_github_str());
            let params = but_github::UpdatePullRequestParams {
                owner,
                repo,
                pr_number,
                title: title.as_deref(),
                body: body.as_deref(),
                base: target_base.as_deref(),
                state: state_str,
            };
            but_github::GitHubClient::from_storage(storage, preferred_account)?
                .update_pull_request(&params)
                .await
                .context("Failed to update PR")?;
            Ok(())
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr_iid = review_number
                .try_into()
                .context("MR: Failed to cast usize to i64, somehow")?;
            // GitLab uses `state_event` ("close" / "reopen") rather than
            // a `state` field. Map the forge-agnostic ReviewState onto that.
            let state_event = state.as_ref().map(|s| s.as_gitlab_state_event());
            let params = but_gitlab::UpdateMergeRequestParams {
                project_id,
                mr_iid,
                title: title.as_deref(),
                description: body.as_deref(),
                target_branch: target_base.as_deref(),
                state_event,
            };
            but_gitlab::mr::update(preferred_account, params, storage).await?;
            Ok(())
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            if matches!(state, Some(ReviewState::Open)) {
                return Err(anyhow::anyhow!(
                    "Bitbucket does not support reopening a declined pull request via the API."
                ));
            }
            if title.is_some() || body.is_some() || target_base.is_some() {
                let id = review_number
                    .try_into()
                    .context("PR: Failed to cast usize to i64, somehow")?;
                let params = but_bitbucket::UpdatePullRequestParams {
                    workspace: owner,
                    repo_slug: repo,
                    id,
                    title: title.as_deref(),
                    description: body.as_deref(),
                    target_branch: target_base.as_deref(),
                };
                but_bitbucket::pr::update(preferred_account, params, storage).await?;
            }
            if matches!(state, Some(ReviewState::Closed)) {
                but_bitbucket::pr::decline(preferred_account, owner, repo, review_number, storage)
                    .await?;
            }
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "Updating pull requests for forge {forge:?} is not implemented yet."
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum ReviewState {
    Open,
    Closed,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ReviewState);

impl ReviewState {
    fn as_github_str(&self) -> &'static str {
        match self {
            ReviewState::Open => "open",
            ReviewState::Closed => "closed",
        }
    }

    fn as_gitlab_state_event(&self) -> &'static str {
        match self {
            ReviewState::Open => "reopen",
            ReviewState::Closed => "close",
        }
    }
}

/// Merge a review to it's target branch
pub async fn merge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    merge_method: Option<ReviewMergeMethod>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_github::MergePullRequestParams {
                owner,
                repo,
                pr_number,
                commit_message: None,
                commit_title: None,
                merge_method: merge_method.as_ref().map(Into::into),
            };
            but_github::pr::merge(preferred_account, params, storage).await
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr_iid = review_number
                .try_into()
                .context("MR: Failed to cast usize to i64, somehow")?;
            let params = but_gitlab::MergeMergeRequestParams {
                project_id,
                mr_iid,
                squash: None,
            };

            but_gitlab::mr::merge(preferred_account, params, storage).await
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            let id = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let strategy = match merge_method {
                Some(ReviewMergeMethod::Squash) => but_bitbucket::MergeStrategy::Squash,
                Some(ReviewMergeMethod::Rebase) => but_bitbucket::MergeStrategy::RebaseFastForward,
                Some(ReviewMergeMethod::Merge) | None => but_bitbucket::MergeStrategy::MergeCommit,
            };
            let params = but_bitbucket::MergePullRequestParams {
                workspace: owner,
                repo_slug: repo,
                id,
                strategy,
            };
            but_bitbucket::pr::merge(preferred_account, params, storage).await
        }
        _ => Err(Error::msg(format!(
            "Merging reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

/// Set a review to automatically merge when all prerequisites are met.
pub async fn set_review_auto_merge_state(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    enable: bool,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;

    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_github::SetPullRequestAutoMergeParams {
                owner,
                repo,
                pr_number,
                state: enable.into(),
            };
            but_github::pr::set_auto_merge(preferred_account, params, storage).await
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr_iid = review_number
                .try_into()
                .context("MR: Failed to cast usize to i64, somehow")?;
            let params = but_gitlab::SetMergeRequestAutoMergeParams {
                project_id,
                mr_iid,
                enabled: enable,
            };
            but_gitlab::mr::set_auto_merge(preferred_account, params, storage).await
        }
        ForgeName::Bitbucket => Err(Error::msg(
            "Bitbucket Cloud does not support auto-merge for pull requests.",
        )),
        _ => Err(Error::msg(format!(
            "Setting the auto-merge state of reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

/// Set the draftiness of a review: Should it be a draft or is it ready to review?
pub async fn set_review_draftiness(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    draft: bool,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;

    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_github::SetPullRequestDraftStateParams {
                owner,
                repo,
                pr_number,
                draft,
            };
            but_github::pr::set_draft_state(preferred_account, params, storage).await
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr_iid = review_number
                .try_into()
                .context("MR: Failed to cast usize to i64, somehow")?;
            let params = but_gitlab::SetMergeRequestDraftStateParams {
                project_id,
                mr_iid,
                is_draft: draft,
            };
            but_gitlab::mr::set_draft_state(preferred_account, params, storage).await
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            let id = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_bitbucket::SetPullRequestDraftStateParams {
                workspace: owner,
                repo_slug: repo,
                id,
                is_draft: draft,
            };
            but_bitbucket::pr::set_draft_state(preferred_account, params, storage).await
        }
        _ => Err(Error::msg(format!(
            "Setting the draftiness of reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateForgeReviewParams {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
}

fn github_head_owner_and_repo<'a>(
    forge_repo_info: &'a crate::forge::ForgeRepoInfo,
    forge_push_repo_info: &'a Option<crate::forge::ForgeRepoInfo>,
) -> (&'a str, Option<&'a str>) {
    if let Some(forge_push_repo_info) = forge_push_repo_info
        && forge_push_repo_info != forge_repo_info
    {
        // If there's a push repo defined, it means we're handling a fork.
        // The head owner is the repository were we push the branches to (the fork) and
        // the target repo (the one holding the base branch) is the original repository.
        (
            forge_push_repo_info.owner.as_str(),
            Some(forge_push_repo_info.repo.as_str()),
        )
    } else {
        // If there's no push repo, we assume the owner is the same as the owner of the target repo.
        // We don't need a `head_repo`` in that case.
        (forge_repo_info.owner.as_str(), None)
    }
}

/// Create a new review (e.g. pull request) for a given forge repository
///
/// Some info on the push repo:
/// If there's a push repository specified and it's different from the main repository,
/// we assume we're opening a review from a fork.
pub async fn create_forge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    forge_push_repo_info: &Option<crate::forge::ForgeRepoInfo>,
    params: &CreateForgeReviewParams,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReview> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let (head_owner, head_repo) =
                github_head_owner_and_repo(forge_repo_info, forge_push_repo_info);

            let head = format!("{}:{}", head_owner, params.source_branch);
            let pr_params = but_github::CreatePullRequestParams {
                owner,
                repo,
                title: &params.title,
                body: &params.body,
                head: &head,
                head_repo,
                base: &params.target_branch,
                draft: params.draft,
            };
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr = but_github::pr::create(preferred_account, pr_params, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        ForgeName::GitLab => {
            let project_id = GitLabProjectId::new(owner, repo);
            // If there's a push repo defined, we consider that the source repository.
            let source_project_id = forge_push_repo_info
                .as_ref()
                .map(|repo_info| GitLabProjectId::new(&repo_info.owner, &repo_info.repo));

            let mr_params = but_gitlab::CreateMergeRequestParams {
                project_id,
                title: &params.title,
                body: &params.body,
                source_branch: &params.source_branch,
                target_branch: &params.target_branch,
                source_project_id,
                draft: params.draft,
            };
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let mr = but_gitlab::mr::create(preferred_account, mr_params, storage).await?;
            Ok(ForgeReview::from(mr))
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            // When opening from a fork, the source repository is the push repo
            // (`workspace/repo_slug`).
            let source_repo_full_name = forge_push_repo_info
                .as_ref()
                .filter(|push| *push != forge_repo_info)
                .map(|push| format!("{}/{}", push.owner, push.repo));

            let pr_params = but_bitbucket::CreatePullRequestParams {
                workspace: owner,
                repo_slug: repo,
                title: &params.title,
                body: &params.body,
                source_branch: &params.source_branch,
                target_branch: &params.target_branch,
                source_repo_full_name: source_repo_full_name.as_deref(),
                draft: params.draft,
            };
            let pr = but_bitbucket::pr::create(preferred_account, pr_params, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        _ => Err(Error::msg(format!(
            "Creating reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewUpdate {
    /// The unique identifier number for this review within its repository. This can be a PR or MR number.
    pub number: i64,
    /// The current body/description of the review, which may be None if no description is set.
    pub body: Option<String>,
    /// Whether the description should be synchronized. If false, the current description is
    /// fetched from the forge before applying the configured stacking policy.
    #[serde(default = "default_update_description")]
    pub update_description: bool,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for MRs).
    pub unit_symbol: String,
    /// If set, update the base/target branch of this review to the given value.
    pub target_branch: Option<String>,
}

fn default_update_description() -> bool {
    true
}

/// The best-effort result of synchronizing review descriptions and target branches after a
/// durable operation such as a push or review creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ReviewSyncOutcome {
    /// There were no remote ref updates or no associated reviews in the selected ref slice.
    NotNeeded,
    /// Every associated review in the selected ref slice was synchronized.
    Succeeded,
    /// The durable operation succeeded, but one or more review updates failed.
    Failed { message: String },
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ReviewSyncOutcome);

/// A newly-created review together with the best-effort stack synchronization result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublishReviewOutcome {
    pub review: ForgeReview,
    pub review_sync: ReviewSyncOutcome,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(PublishReviewOutcome);

/// Controls the managed GitButler stack block in review descriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStackingDescription {
    Bottom,
    Top,
    Disabled,
}

/// Controls whether same-repository GitHub reviews use GitHub's native stacks API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubStackingMode {
    /// Use native stacks when the repository supports them, description metadata otherwise.
    Auto,
    Disabled,
    Native,
}

/// Remove native GitHub stack membership before changing review target branches.
///
/// The caller temporarily flattens reordered reviews onto trunk before pushing their refs. GitHub
/// refuses to change the base branch of any stacked pull request — even when membership and
/// ordering stay the same — so every native stack containing these reviews is dissolved first.
///
/// Returns the ordered memberships of the dissolved stacks so a failed push can restore them via
/// [`restore_native_stacks`]. After a successful push, regular synchronization re-registers the
/// desired membership.
pub async fn prepare_review_target_updates(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    forge_push_repo_info: &Option<crate::forge::ForgeRepoInfo>,
    reviews: &[ForgeReviewUpdate],
    storage: &but_forge_storage::Controller,
    github_stacking_mode: GitHubStackingMode,
) -> Result<Vec<Vec<i64>>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    if *forge != ForgeName::GitHub
        || github_stacking_mode == GitHubStackingMode::Disabled
        || reviews.is_empty()
    {
        return Ok(Vec::new());
    }

    let (_, head_repo) = github_head_owner_and_repo(forge_repo_info, forge_push_repo_info);
    if head_repo.is_some() {
        return Ok(Vec::new());
    }

    let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
    let pr_numbers = reviews
        .iter()
        .map(|review| review.number)
        .collect::<Vec<_>>();
    let dissolved =
        but_github::stacks::dissolve(preferred_account, owner, repo, &pr_numbers, storage).await?;
    let but_github::stacks::Availability::Supported(stacks) = dissolved else {
        return Ok(Vec::new());
    };
    // GitHub keeps closed pull requests as stack members but refuses to register a stack
    // containing one, so they are dropped from the rollback snapshot. A stack needs two open
    // members to be restorable at all.
    Ok(stacks
        .into_iter()
        .filter_map(|stack| {
            let open: Vec<i64> = stack
                .pull_requests
                .into_iter()
                .filter(|review| !review.is_closed())
                .map(|review| review.number)
                .collect();
            (open.len() >= 2).then_some(open)
        })
        .collect())
}

/// Re-register native stacks dissolved by [`prepare_review_target_updates`] after a failed push.
///
/// Call this only after the affected review target branches have been restored, so the recreated
/// stacks match the bases GitHub sees.
pub async fn restore_native_stacks(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    stacks: &[Vec<i64>],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo { owner, repo, .. } = forge_repo_info;
    let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
    let mut errors = Vec::new();
    for pull_requests in stacks {
        if let Err(err) =
            but_github::stacks::create(preferred_account, owner, repo, pull_requests, storage).await
        {
            errors.push(format!("{err:#}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "Could not restore native GitHub stack membership after the push failed:\n{}",
            errors.join("\n")
        )
    }
}

async fn github_review_bodies(
    preferred_account: Option<&but_github::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    reviews: &[ForgeReviewUpdate],
    storage: &but_forge_storage::Controller,
) -> Result<(Vec<Option<Option<String>>>, Vec<String>)> {
    let mut bodies = Vec::with_capacity(reviews.len());
    let mut errors = Vec::new();
    for review in reviews {
        if review.update_description {
            bodies.push(Some(review.body.clone()));
            continue;
        }
        match but_github::pr::get(
            preferred_account,
            owner,
            repo,
            review.number.try_into()?,
            storage,
        )
        .await
        {
            Ok(review) => bodies.push(Some(review.body)),
            Err(err) => {
                errors.push(format!("PR #{} description: {err}", review.number));
                bodies.push(None);
            }
        }
    }
    Ok((bodies, errors))
}

async fn sync_github_reviews_with_descriptions(
    preferred_account: Option<&but_github::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    reviews: &[ForgeReviewUpdate],
    storage: &but_forge_storage::Controller,
    description_mode: ReviewStackingDescription,
    update_bases: bool,
) -> Result<Vec<String>> {
    let pr_numbers: Vec<i64> = reviews.iter().map(|review| review.number).collect();
    let (bodies, mut errors) =
        github_review_bodies(preferred_account, owner, repo, reviews, storage).await?;
    for (review, current_body) in reviews.iter().zip(bodies) {
        let updated_body = current_body.map(|body| {
            update_body_with_mode(
                body.as_deref(),
                review.number,
                &pr_numbers,
                "#",
                description_mode,
            )
        });
        let params = but_github::UpdatePullRequestParams {
            owner,
            repo,
            pr_number: review.number,
            title: None,
            body: updated_body.as_deref(),
            base: if update_bases {
                review.target_branch.as_deref()
            } else {
                None
            },
            state: None,
        };
        if let Err(err) = but_github::pr::update(preferred_account, params, storage).await {
            errors.push(format!("PR #{}: {err}", review.number));
        }
    }
    Ok(errors)
}

async fn sync_github_native_reviews(
    preferred_account: Option<&but_github::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    reviews: &[ForgeReviewUpdate],
    storage: &but_forge_storage::Controller,
) -> Result<but_github::stacks::Availability<Vec<String>>> {
    let pr_numbers: Vec<i64> = reviews.iter().map(|review| review.number).collect();
    let prepared =
        but_github::stacks::prepare(preferred_account, owner, repo, &pr_numbers, storage).await?;
    let but_github::stacks::Availability::Supported(mut prepared) = prepared else {
        return Ok(but_github::stacks::Availability::Unsupported);
    };

    // A surviving stack pins every member's base except the bottom one, whose base is the
    // stack's target branch. That base can still be stale — e.g. the workspace target changed
    // while pre-push flattening was skipped for lack of cached current targets. Skipping it as
    // locked would report success and mask the drift, so verify it and rebuild on mismatch.
    if !prepared.locked.is_empty()
        && let Some(bottom) = reviews.first()
        && let Some(desired_target) = bottom.target_branch.as_deref()
    {
        let current = but_github::pr::get(
            preferred_account,
            owner,
            repo,
            bottom.number.try_into()?,
            storage,
        )
        .await?;
        if current.target_branch != desired_target {
            but_github::stacks::dissolve(preferred_account, owner, repo, &pr_numbers, storage)
                .await?;
            prepared = but_github::stacks::PreparedPlan {
                plan: but_github::stacks::ReconcilePlan::Create {
                    desired: pr_numbers.clone(),
                },
                locked: Vec::new(),
            };
        }
    }

    let (bodies, mut errors) =
        github_review_bodies(preferred_account, owner, repo, reviews, storage).await?;

    // Rebuilds have to dissolve the old native membership before GitHub will accept reordered
    // target branches. Legacy footers remain in place until the new native shape is durable.
    but_github::stacks::unstack_conflicting(
        preferred_account,
        owner,
        repo,
        &prepared.plan,
        storage,
    )
    .await?;

    let mut target_errors = Vec::new();
    for review in reviews {
        let Some(target_branch) = review.target_branch.as_deref() else {
            continue;
        };
        // GitHub rejects base updates for current stack members, even when the base is
        // unchanged. Surviving members keep the bases the stack already enforces.
        if prepared.locked.contains(&review.number) {
            continue;
        }
        let params = but_github::UpdatePullRequestParams {
            owner,
            repo,
            pr_number: review.number,
            title: None,
            body: None,
            base: Some(target_branch),
            state: None,
        };
        if let Err(err) = but_github::pr::update(preferred_account, params, storage).await {
            target_errors.push(format!("PR #{} target: {err}", review.number));
        }
    }
    if !target_errors.is_empty() {
        // Dissolved membership stays dissolved here; the next successful synchronization sees
        // no registered stacks and recreates the desired one.
        anyhow::bail!(
            "Could not update all PR targets before native stack registration:\n{}",
            target_errors.join("\n")
        );
    }

    but_github::stacks::finish(preferred_account, owner, repo, &prepared.plan, storage).await?;

    // GitHub now renders the native stack, so GitButler's description footer is redundant.
    // Strip any existing footers regardless of the configured description mode, which only
    // applies to footer-based stacking. User text is preserved, and reviews whose body would
    // not change are skipped entirely — unless the caller supplied a new description to push.
    for (review, current_body) in reviews.iter().zip(bodies) {
        let Some(current_body) = current_body else {
            continue;
        };
        let updated_body = update_body_with_mode(
            current_body.as_deref(),
            review.number,
            &pr_numbers,
            "#",
            ReviewStackingDescription::Disabled,
        );
        if !review.update_description && current_body.as_deref().unwrap_or("") == updated_body {
            continue;
        }
        let params = but_github::UpdatePullRequestParams {
            owner,
            repo,
            pr_number: review.number,
            title: None,
            body: Some(&updated_body),
            base: None,
            state: None,
        };
        if let Err(err) = but_github::pr::update(preferred_account, params, storage).await {
            errors.push(format!("PR #{} description: {err}", review.number));
        }
    }
    Ok(but_github::stacks::Availability::Supported(errors))
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewUpdate);

impl From<ForgeReview> for ForgeReviewUpdate {
    fn from(review: ForgeReview) -> Self {
        ForgeReviewUpdate {
            number: review.number,
            body: review.body,
            update_description: true,
            unit_symbol: review.unit_symbol,
            target_branch: Some(review.target_branch),
        }
    }
}

impl From<ForgeReviewTargetUpdate> for ForgeReviewUpdate {
    fn from(update: ForgeReviewTargetUpdate) -> Self {
        ForgeReviewUpdate {
            number: update.number,
            body: None,
            update_description: false,
            unit_symbol: String::new(),
            target_branch: Some(update.target_branch),
        }
    }
}

/// Update reviews: description footers and, optionally, target/base branches.
///
/// Per-review failures are collected rather than aborting the batch.
pub async fn sync_reviews(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    forge_push_repo_info: &Option<crate::forge::ForgeRepoInfo>,
    reviews: &[ForgeReviewUpdate],
    storage: &but_forge_storage::Controller,
    description_mode: ReviewStackingDescription,
    github_stacking_mode: GitHubStackingMode,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;

    let mut errors: Vec<String> = Vec::new();

    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_numbers: Vec<i64> = reviews.iter().map(|r| r.number).collect();
            let (_, head_repo) = github_head_owner_and_repo(forge_repo_info, forge_push_repo_info);
            let use_native = github_stacking_mode != GitHubStackingMode::Disabled
                && head_repo.is_none()
                && !reviews.is_empty();
            if use_native {
                match sync_github_native_reviews(preferred_account, owner, repo, reviews, storage)
                    .await
                {
                    Ok(but_github::stacks::Availability::Supported(native_errors)) => {
                        errors.extend(native_errors);
                    }
                    Ok(but_github::stacks::Availability::Unsupported) => {
                        // Under `Auto` an unenrolled repository is the expected case, not an error.
                        if github_stacking_mode == GitHubStackingMode::Native {
                            errors.push(
                                "GitHub native stacks are not enabled for this repository"
                                    .to_string(),
                            );
                        }
                        errors.extend(
                            sync_github_reviews_with_descriptions(
                                preferred_account,
                                owner,
                                repo,
                                reviews,
                                storage,
                                description_mode,
                                true,
                            )
                            .await?,
                        );
                    }
                    Err(err) => {
                        errors.push(format!("GitHub native stack: {err}"));
                        // Some reviews may still be native stack members, and GitHub rejects
                        // base updates for members, so sync only descriptions. Because this
                        // function returns an error, the caller does not cache the desired
                        // targets, and the next synchronization still sees the drift and
                        // reconciles bases and membership.
                        errors.extend(
                            sync_github_reviews_with_descriptions(
                                preferred_account,
                                owner,
                                repo,
                                reviews,
                                storage,
                                description_mode,
                                false,
                            )
                            .await?,
                        );
                    }
                }
            } else {
                if github_stacking_mode == GitHubStackingMode::Disabled
                    && head_repo.is_none()
                    && !reviews.is_empty()
                    && let Err(err) = but_github::stacks::dissolve(
                        preferred_account,
                        owner,
                        repo,
                        &pr_numbers,
                        storage,
                    )
                    .await
                {
                    errors.push(format!("GitHub native stack removal: {err}"));
                }
                errors.extend(
                    sync_github_reviews_with_descriptions(
                        preferred_account,
                        owner,
                        repo,
                        reviews,
                        storage,
                        description_mode,
                        true,
                    )
                    .await?,
                );
            }
        }
        ForgeName::GitLab => {
            let project_id = GitLabProjectId::new(owner, repo);
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let mr_iids: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let current_body = if !review.update_description {
                    match but_gitlab::mr::get(
                        preferred_account,
                        project_id.clone(),
                        review.number.try_into()?,
                        storage,
                    )
                    .await
                    {
                        Ok(review) => Some(review.description),
                        Err(err) => {
                            errors.push(format!("MR !{} description: {err}", review.number));
                            None
                        }
                    }
                } else {
                    Some(review.body.clone())
                };
                let updated_body = current_body.map(|body| {
                    update_body_with_mode(
                        body.as_deref(),
                        review.number,
                        &mr_iids,
                        "!",
                        description_mode,
                    )
                });

                let params = but_gitlab::UpdateMergeRequestParams {
                    project_id: project_id.clone(),
                    mr_iid: review.number,
                    title: None,
                    description: updated_body.as_deref(),
                    target_branch: review.target_branch.as_deref(),
                    state_event: None,
                };

                if let Err(err) = but_gitlab::mr::update(preferred_account, params, storage).await {
                    errors.push(format!("MR !{}: {err}", review.number));
                }
            }
        }
        ForgeName::Bitbucket => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.bitbucket());
            let pr_ids: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let current_body = if !review.update_description {
                    match but_bitbucket::pr::get(
                        preferred_account,
                        owner,
                        repo,
                        review.number.try_into()?,
                        storage,
                    )
                    .await
                    {
                        Ok(review) => Some(review.description),
                        Err(err) => {
                            errors.push(format!("PR #{} description: {err}", review.number));
                            None
                        }
                    }
                } else {
                    Some(review.body.clone())
                };
                let updated_body = current_body.map(|body| {
                    update_body_with_mode(
                        body.as_deref(),
                        review.number,
                        &pr_ids,
                        "#",
                        description_mode,
                    )
                });

                let params = but_bitbucket::UpdatePullRequestParams {
                    workspace: owner,
                    repo_slug: repo,
                    id: review.number,
                    title: None,
                    description: updated_body.as_deref(),
                    target_branch: review.target_branch.as_deref(),
                };

                if let Err(err) =
                    but_bitbucket::pr::update(preferred_account, params, storage).await
                {
                    errors.push(format!("PR #{}: {err}", review.number));
                }
            }
        }
        _ => {
            return Err(Error::msg(format!(
                "Updating reviews for forge {forge:?} is not implemented yet.",
            )));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "Some reviews failed to update:\n{}",
            errors.join("\n")
        )))
    }
}

/// A target branch update for a single review (PR/MR).
#[derive(Debug, Clone)]
pub struct ForgeReviewTargetUpdate {
    pub number: i64,
    pub target_branch: String,
}

/// Compute the expected target branch for each review in a stack.
/// Walks branches bottom-to-top; each review targets the preceding reviewed branch
/// (or `base_branch` for the bottom-most review). Branches without reviews are skipped:
/// they are part of the Git stack, but cannot be valid forge review targets.
pub fn compute_review_target_updates(
    heads: &[(String, Option<i64>)],
    base_branch: &str,
) -> Vec<ForgeReviewTargetUpdate> {
    let mut updates = Vec::new();
    let mut current_target = base_branch;
    // heads are expected bottom-to-top (base branch first).
    for (branch_name, review_number) in heads {
        if let Some(number) = review_number {
            updates.push(ForgeReviewTargetUpdate {
                number: *number,
                target_branch: current_target.to_string(),
            });
            current_target = branch_name;
        }
    }
    updates
}

/// Replaces or inserts a new footer into an existing body of text.
///
/// If there is only one PR in the stack, no footer is appended and any existing
/// footer is removed.
///
/// # Arguments
/// * `body` - The existing PR body text (may be None or empty)
/// * `pr_number` - The PR number for which to update the body
/// * `all_pr_numbers` - All PR numbers in the stack (ordered from base to top)
/// * `symbol` - The symbol to use before the PR number (e.g., "#" or "!")
///
/// # Returns
/// The updated body with the footer replaced, inserted, or removed
#[cfg(test)]
fn update_body(body: Option<&str>, pr_number: i64, all_pr_numbers: &[i64], symbol: &str) -> String {
    update_body_with_mode(
        body,
        pr_number,
        all_pr_numbers,
        symbol,
        ReviewStackingDescription::Bottom,
    )
}

fn update_body_with_mode(
    body: Option<&str>,
    pr_number: i64,
    all_pr_numbers: &[i64],
    symbol: &str,
    mode: ReviewStackingDescription,
) -> String {
    let body = body.unwrap_or("");
    let user_body = match strip_managed_footers(body) {
        ManagedFooter::Complete { user_body } => user_body,
        ManagedFooter::Malformed => {
            // Never infer a boundary from incomplete or out-of-order markers: they may be user
            // content. In particular, don't add another managed block alongside them.
            return body.to_string();
        }
        ManagedFooter::Absent => {
            return match (all_pr_numbers.len(), mode) {
                (0 | 1, _) | (_, ReviewStackingDescription::Disabled) => body.to_string(),
                (_, ReviewStackingDescription::Bottom) => compose_body(
                    body,
                    &generate_footer_with_mode(
                        pr_number,
                        all_pr_numbers,
                        symbol,
                        ReviewStackingDescription::Bottom,
                    ),
                    "",
                ),
                (_, ReviewStackingDescription::Top) => compose_body(
                    "",
                    &generate_footer_with_mode(
                        pr_number,
                        all_pr_numbers,
                        symbol,
                        ReviewStackingDescription::Top,
                    ),
                    body,
                ),
            };
        }
    };

    if all_pr_numbers.len() <= 1 || mode == ReviewStackingDescription::Disabled {
        return user_body;
    }

    let footer = generate_footer_with_mode(pr_number, all_pr_numbers, symbol, mode);
    match mode {
        ReviewStackingDescription::Bottom => compose_body(&user_body, &footer, ""),
        ReviewStackingDescription::Top => compose_body("", &footer, &user_body),
        ReviewStackingDescription::Disabled => unreachable!("handled above"),
    }
}

enum ManagedFooter {
    Absent,
    Complete { user_body: String },
    Malformed,
}

fn strip_managed_footers(body: &str) -> ManagedFooter {
    let mut remainder = body;
    let mut user_parts = Vec::new();
    let mut found = false;

    loop {
        let top = remainder.find(STACKING_FOOTER_BOUNDARY_TOP);
        let bottom = remainder.find(STACKING_FOOTER_BOUNDARY_BOTTOM);
        match (top, bottom) {
            (None, None) => {
                if !found {
                    return ManagedFooter::Absent;
                }
                user_parts.push(remainder);
                return ManagedFooter::Complete {
                    user_body: user_parts
                        .into_iter()
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                };
            }
            (None, Some(_)) => return ManagedFooter::Malformed,
            (Some(top), Some(bottom)) if bottom < top => {
                return ManagedFooter::Malformed;
            }
            (Some(top), _) => {
                let after_top = top + STACKING_FOOTER_BOUNDARY_TOP.len();
                let Some(bottom) = remainder[after_top..]
                    .find(STACKING_FOOTER_BOUNDARY_BOTTOM)
                    .map(|bottom| bottom + after_top)
                else {
                    return ManagedFooter::Malformed;
                };
                user_parts.push(&remainder[..top]);
                remainder = &remainder[bottom + STACKING_FOOTER_BOUNDARY_BOTTOM.len()..];
                found = true;
            }
        }
    }
}

fn compose_body(head: &str, footer: &str, tail: &str) -> String {
    [head.trim(), footer.trim(), tail.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Generates a footer for use in pull request descriptions when part of a stack.
///
/// # Arguments
/// * `for_pr_number` - The PR number for which to generate the footer
/// * `all_pr_numbers` - All PR numbers in the stack (ordered from base to top)
/// * `symbol` - The symbol to use before the PR number (e.g., "#" or "!")
///
/// # Returns
/// A formatted markdown footer string with stack information
#[cfg(test)]
fn generate_footer(for_pr_number: i64, all_pr_numbers: &[i64], symbol: &str) -> String {
    generate_footer_with_mode(
        for_pr_number,
        all_pr_numbers,
        symbol,
        ReviewStackingDescription::Bottom,
    )
}

fn generate_footer_with_mode(
    for_pr_number: i64,
    all_pr_numbers: &[i64],
    symbol: &str,
    mode: ReviewStackingDescription,
) -> String {
    let stack_length = all_pr_numbers.len();
    let stack_index = all_pr_numbers
        .iter()
        .position(|&n| n == for_pr_number)
        .unwrap_or(0);
    let nth = stack_index + 1;

    let mut footer = String::new();
    footer.push_str(STACKING_FOOTER_BOUNDARY_TOP);
    footer.push('\n');
    if mode == ReviewStackingDescription::Bottom {
        footer.push_str("---\n");
    }
    footer.push_str(&format!(
        "This is **part {nth} of {stack_length} in a stack** made with GitButler:\n"
    ));

    for (i, &pr_number) in all_pr_numbers.iter().rev().enumerate() {
        let current = pr_number == for_pr_number;
        let indicator = if current { "👈 " } else { "" };
        footer.push_str(&format!(
            "- <kbd>&nbsp;{}&nbsp;</kbd> {}{}{}{}\n",
            stack_length - i,
            symbol,
            pr_number,
            if current { " " } else { "" },
            indicator
        ));
    }

    if mode == ReviewStackingDescription::Top {
        footer.push_str("---\n");
    }
    footer.push_str(STACKING_FOOTER_BOUNDARY_BOTTOM);
    footer
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn p(path: &str) -> &Path {
        Path::new(path)
    }

    #[test]
    fn github_submission_states_map_and_unknowns_drop() {
        use ForgeReviewSubmissionState as S;
        assert_eq!(github_submission_state("APPROVED"), Some(S::Approved));
        assert_eq!(
            github_submission_state("CHANGES_REQUESTED"),
            Some(S::ChangesRequested)
        );
        assert_eq!(github_submission_state("COMMENTED"), Some(S::Commented));
        assert_eq!(github_submission_state("DISMISSED"), Some(S::Dismissed));
        // The caller's own draft, and any state GitHub adds later, are omitted.
        assert_eq!(github_submission_state("PENDING"), None);
        assert_eq!(github_submission_state("APPROVED_WITH_COMMENTS"), None);
    }

    fn repo_info(owner: &str, repo: &str) -> crate::forge::ForgeRepoInfo {
        crate::forge::ForgeRepoInfo {
            forge: crate::forge::ForgeName::GitHub,
            owner: owner.to_string(),
            repo: repo.to_string(),
            protocol: "https".to_string(),
        }
    }

    #[test]
    fn test_github_head_owner_and_repo_without_push_repo() {
        let forge_repo_info = repo_info("target-owner", "target-repo");

        let (head_owner, head_repo) = github_head_owner_and_repo(&forge_repo_info, &None);

        assert_eq!(head_owner, "target-owner");
        assert_eq!(head_repo, None);
    }

    #[test]
    fn test_github_head_owner_and_repo_with_fork_push_repo() {
        let forge_repo_info = repo_info("target-owner", "target-repo");
        let forge_push_repo_info = Some(repo_info("fork-owner", "fork-repo"));

        let (head_owner, head_repo) =
            github_head_owner_and_repo(&forge_repo_info, &forge_push_repo_info);

        assert_eq!(head_owner, "fork-owner");
        assert_eq!(head_repo, Some("fork-repo"));
    }

    #[test]
    fn test_github_head_owner_and_repo_with_equal_push_repo() {
        let forge_repo_info = repo_info("target-owner", "target-repo");
        let forge_push_repo_info = Some(repo_info("target-owner", "target-repo"));

        let (head_owner, head_repo) =
            github_head_owner_and_repo(&forge_repo_info, &forge_push_repo_info);

        assert_eq!(head_owner, "target-owner");
        assert_eq!(head_repo, None);
    }

    #[test]
    fn gitlab_review_preserves_source_project_clone_urls() {
        let review = ForgeReview::from(but_gitlab::MergeRequest {
            web_url: "https://gitlab.example/acme/widgets/-/merge_requests/42".into(),
            iid: 42,
            title: "Fork MR".into(),
            description: None,
            author: None,
            labels: vec![],
            draft: false,
            source_branch: "fork-feature".into(),
            target_branch: "main".into(),
            sha: "1234567890abcdef1234567890abcdef12345678".into(),
            integration_commit_shas: vec![],
            created_at: None,
            updated_at: None,
            merged_at: None,
            closed_at: None,
            project_id: 10,
            source_project_id: Some(20),
            target_project_id: Some(10),
            repository_ssh_url: Some("git@gitlab.example:contributor/widgets.git".into()),
            repository_https_url: Some("https://gitlab.example/contributor/widgets.git".into()),
            repo_owner: Some("contributor".into()),
            source_project_is_fork: true,
            assignees: vec![],
            reviewers: vec![],
            auto_merge_enabled: false,
        });

        assert_eq!(
            review.repository_https_url.as_deref(),
            Some("https://gitlab.example/contributor/widgets.git"),
            "GitLab review apply needs the source project HTTPS URL"
        );
        assert_eq!(
            review.repository_ssh_url.as_deref(),
            Some("git@gitlab.example:contributor/widgets.git"),
            "GitLab review apply needs the source project SSH URL"
        );
        assert_eq!(
            review.repo_owner.as_deref(),
            Some("contributor"),
            "GitLab fork remotes should be named from the source project namespace"
        );
        assert!(
            review.head_repo_is_fork,
            "GitLab fork MRs should preserve that the source project differs from the target"
        );
    }

    #[test]
    fn test_is_valid_review_template_path_github() {
        assert!(is_valid_review_template_path_github(p(
            ".github/PULL_REQUEST_TEMPLATE.md"
        )));
        assert!(is_valid_review_template_path_github(p(
            ".github/pull_request_template.md"
        )));
        assert!(is_valid_review_template_path_github(p(
            ".github/PULL_REQUEST_TEMPLATE/something.md"
        )));
        assert!(is_valid_review_template_path_github(p(
            ".docs/PULL_REQUEST_TEMPLATE.md"
        )));
        assert!(is_valid_review_template_path_github(p(
            "PULL_REQUEST_TEMPLATE.md"
        )));
        assert!(!is_valid_review_template_path_github(p("README.md"),));
    }

    #[test]
    fn test_is_valid_review_template_path_github_windows() {
        assert!(is_valid_review_template_path_github(p(
            ".github\\PULL_REQUEST_TEMPLATE.md"
        ),));
        assert!(is_valid_review_template_path_github(p(
            ".github\\pull_request_template.md"
        ),));
        assert!(is_valid_review_template_path_github(p(
            ".github\\PULL_REQUEST_TEMPLATE\\something.md"
        ),));
        assert!(is_valid_review_template_path_github(p(
            ".docs\\PULL_REQUEST_TEMPLATE.md"
        ),));
        assert!(is_valid_review_template_path_github(p(
            "PULL_REQUEST_TEMPLATE.md"
        ),));
        assert!(!is_valid_review_template_path_github(p("README.md"),));
    }

    #[test]
    fn test_is_valid_review_template_path_gitlab() {
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab/merge_request_templates/Default.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab/merge_request_templates/Documentation.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab/merge_request_templates/Security Fix.md"
        )));
        assert!(!is_valid_review_template_path_gitlab(p("README.md")));
        assert!(!is_valid_review_template_path_gitlab(p(
            ".gitlab/issue_templates/Bug.md"
        )));
        assert!(!is_valid_review_template_path_gitlab(p(
            ".gitlab/merge_request_templates/Default.txt"
        )));
    }

    #[test]
    fn test_is_valid_review_template_path_gitlab_windows() {
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_templates\\Default.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_templates\\Documentation.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_templates\\Security Fix.md"
        )));
        assert!(!is_valid_review_template_path_gitlab(p("README.md")));
        assert!(!is_valid_review_template_path_gitlab(p(
            ".gitlab\\issue_templates\\Bug.md"
        )));
        assert!(!is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_templates\\Default.txt"
        )));
    }

    #[test]
    fn test_get_gitlab_directory_path() {
        let root_path = p("/path/to/project");
        let gitlab_path = get_gitlab_directory_path(root_path);
        assert_eq!(gitlab_path, p("/path/to/project/.gitlab"));
    }

    #[test]
    fn test_is_review_template_gitlab() {
        // Valid GitLab merge request templates
        assert!(is_review_template_gitlab(
            ".gitlab/merge_request_templates/Default.md"
        ));
        assert!(is_review_template_gitlab(
            ".gitlab/merge_request_templates/Documentation.md"
        ));
        assert!(is_review_template_gitlab(
            ".gitlab/merge_request_templates/Security Fix.md"
        ));

        // Invalid paths
        assert!(!is_review_template_gitlab("README.md"));
        assert!(!is_review_template_gitlab(".gitlab/issue_templates/Bug.md"));
        assert!(!is_review_template_gitlab(
            ".gitlab/merge_request_templates/Default.txt"
        ));
        assert!(!is_review_template_gitlab(
            "merge_request_templates/Default.md"
        ));

        // Windows path separators should work
        assert!(is_review_template_gitlab(
            ".gitlab\\merge_request_templates\\Default.md"
        ));
    }

    #[test]
    fn test_generate_footer_single_pr() {
        let footer = generate_footer(123, &[123], "#");
        assert!(footer.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(footer.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(footer.contains("part 1 of 1 in a stack"));
        assert!(footer.contains("#123"));
        assert!(footer.contains("👈"));
    }

    #[test]
    fn test_generate_footer_multiple_prs() {
        let all_prs = vec![100, 101, 102];
        let footer = generate_footer(101, &all_prs, "#");

        assert!(footer.contains("part 2 of 3 in a stack"));
        assert!(footer.contains("#100"));
        assert!(footer.contains("#101"));
        assert!(footer.contains("#102"));

        // The current PR (101) should have the pointing emoji
        let lines: Vec<&str> = footer.lines().collect();
        let pr_101_line = lines.iter().find(|l| l.contains("#101")).unwrap();
        assert!(pr_101_line.contains("👈"));

        // Other PRs should not have the emoji
        let pr_100_line = lines.iter().find(|l| l.contains("#100")).unwrap();
        assert!(!pr_100_line.contains("👈"));
    }

    #[test]
    fn test_generate_footer_with_custom_symbol() {
        let footer = generate_footer(42, &[41, 42, 43], "!");
        assert!(footer.contains("!41"));
        assert!(footer.contains("!42"));
        assert!(footer.contains("!43"));
    }

    #[test]
    fn test_generate_footer_numbering() {
        let all_prs = vec![100, 101, 102, 103];
        let footer = generate_footer(101, &all_prs, "#");

        let lines: Vec<&str> = footer.lines().collect();

        // Check that numbering goes from top (4) to bottom (1)
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<kbd>&nbsp;1&nbsp;</kbd>") && l.contains("#100"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<kbd>&nbsp;2&nbsp;</kbd>") && l.contains("#101"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<kbd>&nbsp;3&nbsp;</kbd>") && l.contains("#102"))
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<kbd>&nbsp;4&nbsp;</kbd>") && l.contains("#103"))
        );
    }

    #[test]
    fn test_update_body_none() {
        let result = update_body(None, 123, &[123, 124], "#");
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(result.contains("#123"));
    }

    #[test]
    fn test_update_body_empty() {
        let result = update_body(Some(""), 123, &[123, 124], "#");
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(result.contains("#123"));
    }

    #[test]
    fn test_update_body_with_existing_content() {
        let body = "This is my PR description.\n\nIt has multiple lines.";
        let result = update_body(Some(body), 123, &[123, 124], "#");

        assert!(result.starts_with("This is my PR description.\n\nIt has multiple lines."));
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(result.contains("#123"));
    }

    #[test]
    fn test_footer_ordering_base_to_top() {
        // PRs should be listed from base (oldest) at bottom to top (newest) at top
        let all_prs = vec![100, 101, 102, 103]; // base to top order
        let footer = generate_footer(102, &all_prs, "#");

        let lines: Vec<&str> = footer.lines().collect();

        // Find the indices of each PR in the footer
        let pr_100_idx = lines.iter().position(|l| l.contains("#100")).unwrap();
        let pr_101_idx = lines.iter().position(|l| l.contains("#101")).unwrap();
        let pr_102_idx = lines.iter().position(|l| l.contains("#102")).unwrap();
        let pr_103_idx = lines.iter().position(|l| l.contains("#103")).unwrap();

        // The top PR (103) should appear first, base PR (100) should appear last
        assert!(pr_103_idx < pr_102_idx);
        assert!(pr_102_idx < pr_101_idx);
        assert!(pr_101_idx < pr_100_idx);
    }

    #[test]
    fn test_footer_position_indicator_first_pr() {
        let all_prs = vec![100, 101, 102];
        let footer = generate_footer(100, &all_prs, "#");

        let lines: Vec<&str> = footer.lines().collect();
        let pr_100_line = lines.iter().find(|l| l.contains("#100")).unwrap();

        assert!(pr_100_line.contains("👈"));
        assert!(pr_100_line.contains("<kbd>&nbsp;1&nbsp;</kbd>"));
    }

    #[test]
    fn test_footer_position_indicator_last_pr() {
        let all_prs = vec![100, 101, 102];
        let footer = generate_footer(102, &all_prs, "#");

        let lines: Vec<&str> = footer.lines().collect();
        let pr_102_line = lines.iter().find(|l| l.contains("#102")).unwrap();

        assert!(pr_102_line.contains("👈"));
        assert!(pr_102_line.contains("<kbd>&nbsp;3&nbsp;</kbd>"));
    }

    #[test]
    fn test_update_body_multiple_prs_to_single_pr() {
        let old_footer = generate_footer(123, &[122, 123, 124], "#");
        let body = format!("Description\n\n{old_footer}");

        // Update to a single PR stack
        let result = update_body(Some(&body), 123, &[123], "#");

        assert_eq!(result, "Description");
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
    }

    #[test]
    fn test_update_body_maintains_proper_spacing() {
        let body = "First paragraph\n\nSecond paragraph";
        let result = update_body(Some(body), 100, &[100, 101], "#");

        // Should have proper spacing between description and footer
        assert!(result.contains("First paragraph\n\nSecond paragraph\n\n"));
        assert!(result.contains(STACKING_FOOTER_BOUNDARY_TOP));
    }

    #[test]
    fn test_generate_footer_large_stack() {
        let all_prs: Vec<i64> = (1..=10).collect();
        let footer = generate_footer(5, &all_prs, "#");

        assert!(footer.contains("part 5 of 10 in a stack"));

        // Verify all PRs are listed
        for pr in &all_prs {
            assert!(footer.contains(&format!("#{pr}")));
        }
    }

    #[test]
    fn test_footer_part_number_matches_position_badge() {
        let all_prs = vec![100, 101, 102, 103, 104];
        for (idx, &pr) in all_prs.iter().enumerate() {
            let footer = generate_footer(pr, &all_prs, "#");
            let position = idx + 1;

            assert!(
                footer.contains(&format!("part {position} of {}", all_prs.len())),
                "PR #{pr} (base position {position}) should be \"part {position}\":\n{footer}"
            );

            let current_line = footer
                .lines()
                .find(|l| l.contains(&format!("#{pr}")) && l.contains("👈"))
                .unwrap_or_else(|| panic!("no current line for #{pr}:\n{footer}"));
            assert!(
                current_line.contains(&format!("<kbd>&nbsp;{position}&nbsp;</kbd>")),
                "PR #{pr} badge should be {position}:\n{current_line}"
            );
        }
    }

    #[test]
    fn test_update_body_with_tail_and_multiple_newlines() {
        let old_footer = generate_footer(100, &[100, 101], "#");
        let body = format!("Head\n\n{old_footer}\n\n\n\nTail with gaps");

        let result = update_body(Some(&body), 100, &[100, 101, 102], "#");

        assert!(result.contains("Head"));
        assert!(result.contains("Tail with gaps"));
        assert!(result.contains("#102"));
    }

    #[test]
    fn test_update_body_replaces_existing_footer() {
        let old_footer = generate_footer(123, &[123], "#");
        let body = format!("My description\n\n{old_footer}\n\nSome trailing content");

        let result = update_body(Some(&body), 123, &[123, 124], "#");

        // Should contain the original description
        assert!(result.contains("My description"));
        // Should contain the trailing content
        assert!(result.contains("Some trailing content"));
        // Should have the new footer with both PRs
        assert!(result.contains("#123"));
        assert!(result.contains("#124"));
        // Should only have one footer (not duplicated)
        let boundary_count = result.matches(STACKING_FOOTER_BOUNDARY_TOP).count();
        assert_eq!(boundary_count, 1);
    }

    #[test]
    fn test_update_body_preserves_head_and_tail() {
        let body = format!(
            "Head content\n\n{STACKING_FOOTER_BOUNDARY_TOP}\n---\nOld footer\n{STACKING_FOOTER_BOUNDARY_BOTTOM}\n\nTail content"
        );

        let result = update_body(Some(&body), 456, &[456, 457], "!");

        assert!(result.starts_with("Head content"));
        assert!(result.contains("Head content\n\nTail content"));
        assert!(result.ends_with(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(result.contains("!456"));
        assert!(result.contains("!457"));
        assert!(!result.contains("Old footer"));
    }

    #[test]
    fn test_update_body_trims_whitespace() {
        let body = "  Content with spaces  ";
        let result = update_body(Some(body), 100, &[100, 101], "#");

        assert!(result.starts_with("Content with spaces"));
        assert!(!result.starts_with("  Content"));
    }

    #[test]
    fn test_update_body_single_pr_no_footer() {
        let body = "This is my PR description.";
        let result = update_body(Some(body), 123, &[123], "#");

        // Should contain the description
        assert_eq!(result, "This is my PR description.");
        // Should NOT contain any footer
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
        assert!(!result.contains("#123"));
    }

    #[test]
    fn test_update_body_single_pr_removes_existing_footer() {
        let old_footer = generate_footer(123, &[123, 124], "#");
        let body = format!("My description\n\n{old_footer}\n\nSome trailing content");

        // Now updating with just one PR should remove the footer
        let result = update_body(Some(&body), 123, &[123], "#");

        assert!(result.contains("My description"));
        assert!(result.contains("Some trailing content"));
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_BOTTOM));
    }

    #[test]
    fn test_update_body_single_pr_empty_body() {
        let result = update_body(None, 123, &[123], "#");

        // Should return empty string (or just whitespace)
        assert!(result.is_empty() || result.trim().is_empty());
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
    }

    #[test]
    fn test_update_body_single_pr_with_tail() {
        let old_footer = generate_footer(123, &[123], "#");
        let body = format!("Head content\n\n{old_footer}\n\nTail content");

        let result = update_body(Some(&body), 123, &[123], "#");

        assert_eq!(result, "Head content\n\nTail content");
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
    }

    #[test]
    fn update_body_top_places_footer_before_user_content() {
        let result = update_body_with_mode(
            Some("User description"),
            101,
            &[100, 101],
            "#",
            ReviewStackingDescription::Top,
        );

        assert!(
            result.starts_with(STACKING_FOOTER_BOUNDARY_TOP),
            "top mode places the managed block first"
        );
        assert!(
            result.ends_with("User description"),
            "top mode preserves user content after the managed block"
        );
    }

    #[test]
    fn update_body_bottom_produces_stack_information_after_user_content() {
        let result = update_body_with_mode(
            Some("User description"),
            101,
            &[100, 101],
            "#",
            ReviewStackingDescription::Bottom,
        );

        assert_eq!(
            result,
            "User description\n\n\
             <!-- GitButler Footer Boundary Top -->\n\
             ---\n\
             This is **part 2 of 2 in a stack** made with GitButler:\n\
             - <kbd>&nbsp;2&nbsp;</kbd> #101 👈 \n\
             - <kbd>&nbsp;1&nbsp;</kbd> #100\n\
             <!-- GitButler Footer Boundary Bottom -->",
            "bottom mode separates user content from the complete stack information section"
        );
    }

    #[test]
    fn update_body_top_produces_stack_information_before_user_content() {
        let result = update_body_with_mode(
            Some("User description"),
            101,
            &[100, 101],
            "#",
            ReviewStackingDescription::Top,
        );

        assert_eq!(
            result,
            "<!-- GitButler Footer Boundary Top -->\n\
             This is **part 2 of 2 in a stack** made with GitButler:\n\
             - <kbd>&nbsp;2&nbsp;</kbd> #101 👈 \n\
             - <kbd>&nbsp;1&nbsp;</kbd> #100\n\
             ---\n\
             <!-- GitButler Footer Boundary Bottom -->\n\n\
             User description",
            "top mode puts the separator below the complete stack information section"
        );
    }

    #[test]
    fn update_body_relocates_footer_between_bottom_and_top() {
        let bottom = update_body_with_mode(
            Some("User description"),
            100,
            &[100, 101],
            "#",
            ReviewStackingDescription::Bottom,
        );
        let top = update_body_with_mode(
            Some(&bottom),
            100,
            &[100, 101],
            "#",
            ReviewStackingDescription::Top,
        );
        let bottom_again = update_body_with_mode(
            Some(&top),
            100,
            &[100, 101],
            "#",
            ReviewStackingDescription::Bottom,
        );

        assert!(
            top.starts_with(STACKING_FOOTER_BOUNDARY_TOP),
            "changing to top relocates the existing block"
        );
        assert!(
            bottom_again.starts_with("User description"),
            "changing back to bottom restores user content first"
        );
        assert_eq!(
            bottom_again.matches(STACKING_FOOTER_BOUNDARY_TOP).count(),
            1,
            "mode changes keep exactly one managed block"
        );
    }

    #[test]
    fn update_body_disabled_removes_footer_and_preserves_user_content() {
        let body = format!("Head\n\n{}\n\nTail", generate_footer(100, &[100, 101], "#"));
        let result = update_body_with_mode(
            Some(&body),
            100,
            &[100, 101],
            "#",
            ReviewStackingDescription::Disabled,
        );

        assert_eq!(result, "Head\n\nTail");
    }

    #[test]
    fn update_body_leaves_malformed_boundaries_untouched() {
        let cases = [
            format!("Intro\n\n{STACKING_FOOTER_BOUNDARY_TOP}\ndangling"),
            format!("dangling\n{STACKING_FOOTER_BOUNDARY_BOTTOM}\n\nTail"),
            format!("{STACKING_FOOTER_BOUNDARY_BOTTOM}\ntext\n{STACKING_FOOTER_BOUNDARY_TOP}"),
        ];

        for body in cases {
            let result = update_body_with_mode(
                Some(&body),
                100,
                &[100, 101],
                "#",
                ReviewStackingDescription::Bottom,
            );
            assert_eq!(
                result, body,
                "incomplete or out-of-order boundaries may be user content"
            );
        }
    }

    #[test]
    fn update_body_is_idempotent_for_each_mode() {
        for mode in [
            ReviewStackingDescription::Bottom,
            ReviewStackingDescription::Top,
            ReviewStackingDescription::Disabled,
        ] {
            let once = update_body_with_mode(Some("Description"), 100, &[100, 101], "#", mode);
            let twice = update_body_with_mode(Some(&once), 100, &[100, 101], "#", mode);
            assert_eq!(twice, once, "repeating {mode:?} does not rewrite the body");
        }
    }

    #[test]
    fn update_body_collapses_duplicate_managed_blocks() {
        let first = generate_footer(100, &[100, 101], "#");
        let second = generate_footer(100, &[100, 101, 102], "#");
        let body = format!("Head\n\n{first}\n\nMiddle\n\n{second}\n\nTail");

        let result = update_body_with_mode(
            Some(&body),
            100,
            &[100, 101, 102],
            "#",
            ReviewStackingDescription::Bottom,
        );

        assert!(
            result.starts_with("Head\n\nMiddle\n\nTail"),
            "content around duplicate blocks is preserved in order"
        );
        assert_eq!(
            result.matches(STACKING_FOOTER_BOUNDARY_TOP).count(),
            1,
            "synchronization repairs duplicate managed blocks"
        );
    }

    // --- compute_review_target_updates tests ---

    fn heads(specs: &[(&str, Option<i64>)]) -> Vec<(String, Option<i64>)> {
        specs
            .iter()
            .map(|(name, id)| (name.to_string(), *id))
            .collect()
    }

    #[test]
    fn target_updates_empty_stack() {
        let result = compute_review_target_updates(&[], "main");
        assert!(result.is_empty());
    }

    #[test]
    fn target_updates_no_reviews() {
        let h = heads(&[("branch-a", None), ("branch-b", None)]);
        let result = compute_review_target_updates(&h, "main");
        assert!(result.is_empty());
    }

    #[test]
    fn target_updates_single_branch_with_review() {
        let h = heads(&[("branch-a", Some(1))]);
        let result = compute_review_target_updates(&h, "main");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].number, 1);
        assert_eq!(result[0].target_branch, "main");
    }

    #[test]
    fn target_updates_stacked_reviews_point_to_parent() {
        // bottom-to-top: branch-a -> branch-b -> branch-c
        let h = heads(&[
            ("branch-a", Some(1)),
            ("branch-b", Some(2)),
            ("branch-c", Some(3)),
        ]);
        let result = compute_review_target_updates(&h, "main");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].number, 1);
        assert_eq!(result[0].target_branch, "main");
        assert_eq!(result[1].number, 2);
        assert_eq!(result[1].target_branch, "branch-a");
        assert_eq!(result[2].number, 3);
        assert_eq!(result[2].target_branch, "branch-b");
    }

    #[test]
    fn target_updates_request_remote_description_hydration() {
        let update: ForgeReviewUpdate = ForgeReviewTargetUpdate {
            number: 42,
            target_branch: "parent".to_string(),
        }
        .into();

        assert!(
            !update.update_description,
            "target-only updates ask sync_reviews to hydrate the remote description"
        );
        assert_eq!(update.body, None);
        assert_eq!(update.target_branch.as_deref(), Some("parent"));
    }

    #[test]
    fn review_update_defaults_to_description_sync_when_field_is_omitted() {
        let omitted: ForgeReviewUpdate = serde_json::from_value(serde_json::json!({
            "number": 42,
            "body": "Caller-provided description",
            "unitSymbol": "#",
            "targetBranch": null
        }))
        .expect("an update from an older caller should deserialize");
        let explicit_false: ForgeReviewUpdate = serde_json::from_value(serde_json::json!({
            "number": 42,
            "body": null,
            "updateDescription": false,
            "unitSymbol": "",
            "targetBranch": "parent"
        }))
        .expect("a target-only update should deserialize");

        assert!(
            omitted.update_description,
            "older callers that omit the field should synchronize their provided description"
        );
        assert!(
            !explicit_false.update_description,
            "target-only callers should still be able to request remote hydration"
        );
    }

    #[test]
    fn target_updates_skips_branches_without_reviews() {
        // branch-a has no review, so branch-b must target the trunk.
        let h = heads(&[("branch-a", None), ("branch-b", Some(5))]);
        let result = compute_review_target_updates(&h, "main");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].number, 5);
        assert_eq!(result[0].target_branch, "main");
    }

    #[test]
    fn target_updates_gap_in_middle() {
        // a(PR) -> b(no PR) -> c(PR): c should skip b and target a.
        let h = heads(&[
            ("branch-a", Some(1)),
            ("branch-b", None),
            ("branch-c", Some(3)),
        ]);
        let result = compute_review_target_updates(&h, "main");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].number, 1);
        assert_eq!(result[0].target_branch, "main");
        assert_eq!(result[1].number, 3);
        assert_eq!(result[1].target_branch, "branch-a");
    }

    #[test]
    fn forge_review_from_bitbucket_pull_request_maps_fields() {
        let pr = but_bitbucket::BitbucketPullRequest {
            html_url: "https://bitbucket.org/ws/repo/pull-requests/7".into(),
            id: 7,
            title: "Add feature".into(),
            description: Some("body".into()),
            state: "MERGED".into(),
            draft: false,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            source_commit_hash: "deadbeef".into(),
            merge_commit_hash: Some("cafef00d".into()),
            created_on: Some("2026-06-01T00:00:00Z".into()),
            updated_on: Some("2026-06-02T00:00:00Z".into()),
            comment_count: 0,
            author: Some(but_bitbucket::BitbucketUser {
                id: 0,
                username: "alice".into(),
                name: Some("Alice".into()),
                email: None,
                avatar_url: Some("https://avatar".into()),
                is_bot: false,
            }),
            reviewers: vec![but_bitbucket::BitbucketUser {
                id: 0,
                username: "bob".into(),
                name: None,
                email: None,
                avatar_url: None,
                is_bot: false,
            }],
            head_repo_is_fork: false,
            repo_owner: None,
        };

        let review = ForgeReview::from(pr);

        assert_eq!(review.number, 7);
        assert_eq!(
            review.html_url,
            "https://bitbucket.org/ws/repo/pull-requests/7"
        );
        assert_eq!(review.title, "Add feature");
        assert_eq!(review.body.as_deref(), Some("body"));
        assert_eq!(review.source_branch, "feature");
        assert_eq!(review.target_branch, "main");
        assert_eq!(review.sha, "deadbeef");
        assert_eq!(review.integration_commit_shas, vec!["cafef00d".to_string()]);
        assert_eq!(review.merged_at.as_deref(), Some("2026-06-02T00:00:00Z"));
        assert_eq!(review.closed_at, None);
        assert!(review.labels.is_empty());
        assert!(!review.draft);
        assert!(!review.head_repo_is_fork);
        assert_eq!(review.unit_symbol, "#");
        assert_eq!(
            review.author.as_ref().map(|a| a.login.as_str()),
            Some("alice")
        );
        assert_eq!(review.reviewers.len(), 1);
        assert_eq!(review.reviewers[0].login, "bob");
    }
}
