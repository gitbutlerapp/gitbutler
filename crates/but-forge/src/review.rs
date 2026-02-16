use std::{
    fmt::Display,
    path::{self},
};

use anyhow::{Error, Result};
use but_fs::list_files;
use but_github::CredentialCheckResult;
use but_gitlab::GitLabProjectId;
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
                list_files(custom_path.as_path(), &[root_path], true, Some(root_path)).unwrap_or_default()
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
        || normalized_path.contains(".github/PULL_REQUEST_TEMPLATE") && normalized_path.ends_with(".md")
        || normalized_path.contains(".github/pull_request_template") && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/PULL_REQUEST_TEMPLATE") && normalized_path.ends_with(".md")
        || normalized_path.contains("docs/pull_request_template") && normalized_path.ends_with(".md")
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
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewLabel {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

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
#[serde(rename_all = "camelCase")]
/// Represents a user from a forge platform (e.g., GitHub, GitLab).
///
/// This structure contains information about a user account on a forge platform,
/// including their identification details and profile information.
pub struct ForgeUser {
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

impl Display for ForgeUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "login: {}, name: {} ",
            self.login,
            self.name.as_deref().unwrap_or("N/A")
        )
    }
}

impl From<but_github::GitHubUser> for ForgeUser {
    fn from(user: but_github::GitHubUser) -> Self {
        ForgeUser {
            id: user.id,
            login: user.login,
            name: user.name,
            email: user.email,
            avatar_url: user.avatar_url,
            is_bot: user.is_bot,
        }
    }
}

impl From<but_gitlab::GitLabUser> for ForgeUser {
    fn from(user: but_gitlab::GitLabUser) -> Self {
        ForgeUser {
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
    pub author: Option<ForgeUser>,
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
    /// Users who have been requested to review or have reviewed this code.
    pub reviewers: Vec<ForgeUser>,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for MRs).
    pub unit_symbol: String,
    /// The timestamp when this review was last fetched from the forge.
    pub last_sync_at: chrono::NaiveDateTime,
}

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
        1
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
            author: pr.author.map(ForgeUser::from),
            labels: pr.labels.into_iter().map(ForgeReviewLabel::from).collect(),
            draft: pr.draft,
            source_branch: pr.source_branch,
            target_branch: pr.target_branch,
            sha: pr.sha,
            created_at: pr.created_at,
            modified_at: pr.modified_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: pr.repository_ssh_url,
            repository_https_url: pr.repository_https_url,
            repo_owner: pr.repo_owner,
            reviewers: pr.requested_reviewers.into_iter().map(ForgeUser::from).collect(),
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
            author: mr.author.map(ForgeUser::from),
            labels: mr.labels.into_iter().map(ForgeReviewLabel::from).collect(),
            draft: mr.draft,
            source_branch: mr.source_branch,
            target_branch: mr.target_branch,
            sha: mr.sha,
            created_at: mr.created_at,
            modified_at: mr.updated_at,
            merged_at: mr.merged_at,
            closed_at: mr.closed_at,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: mr.reviewers.into_iter().map(ForgeUser::from).collect(),
            unit_symbol: "!".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum CacheConfig {
    CacheOnly,
    CacheWithFallback {
        max_age_seconds: u64,
    },
    #[default]
    NoCache,
}

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
        CacheConfig::CacheOnly => crate::db::reviews_from_cache(db)?,
        CacheConfig::CacheWithFallback { max_age_seconds } => {
            let cached = crate::db::reviews_from_cache(db)?;
            if let Some(last_sync) = cached.first().map(|r| r.last_sync_at) {
                let age = chrono::Local::now().naive_local() - last_sync;
                if !cached.is_empty() && age.num_seconds() as u64 <= max_age_seconds {
                    return Ok(cached);
                }
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

/// Check whether there's an account that would be used for this repository is authenticated.
pub async fn check_forge_account_is_valid(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeAccountValidity> {
    match forge_repo_info.forge {
        ForgeName::GitHub => {
            let preferred_account = match preferred_forge_user.as_ref().and_then(|user| user.github().cloned()) {
                Some(account) => account,
                None => {
                    let known_accounts = but_github::list_known_github_accounts(storage).await?;
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
            let preferred_account = match preferred_forge_user.as_ref().and_then(|user| user.gitlab().cloned()) {
                Some(account) => account,
                None => {
                    let known_accounts = but_gitlab::list_known_gitlab_accounts(storage).await?;
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
    let crate::forge::ForgeRepoInfo { forge, owner, repo, .. } = forge_repo_info;
    let reviews = match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github().cloned());

            // Clone owned data for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let storage = storage.clone();

            let pulls = std::thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(but_github::pr::list(
                    preferred_account.as_ref(),
                    &owner,
                    &repo,
                    &storage,
                ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            pulls.into_iter().map(ForgeReview::from).collect::<Vec<ForgeReview>>()
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab().cloned());

            // Clone owned data for thread
            let project_id = GitLabProjectId::new(owner, repo);
            let storage = storage.clone();

            let mrs = std::thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(but_gitlab::mr::list(
                    preferred_account.as_ref(),
                    project_id,
                    &storage,
                ))
            })
            .join()
            .map_err(|e| anyhow::anyhow!("Failed to join thread: {e:?}"))??;

            mrs.into_iter().map(ForgeReview::from).collect::<Vec<ForgeReview>>()
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
#[serde(rename_all = "camelCase")]
pub enum ForgeReviewFilter {
    Today,
    ThisWeek,
    ThisMonth,
    #[default]
    All,
}

pub async fn list_forge_reviews_for_branch(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    branch: &str,
    storage: &but_forge_storage::Controller,
    filter: Option<ForgeReviewFilter>,
) -> Result<Vec<ForgeReview>> {
    let filter = filter.unwrap_or_default();
    let crate::forge::ForgeRepoInfo { forge, owner, repo, .. } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github().cloned());
            let prs =
                but_github::pr::list_all_for_branch(preferred_account.as_ref(), owner, repo, branch, storage).await?;

            let prs = filter_prs(prs, &filter);

            Ok(prs.into_iter().map(ForgeReview::from).collect())
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab().cloned());
            let project_id = GitLabProjectId::new(owner, repo);
            let mrs =
                but_gitlab::mr::list_all_for_target(preferred_account.as_ref(), project_id, branch, storage).await?;
            let mrs = filter_mrs(mrs, &filter);
            Ok(mrs.into_iter().map(ForgeReview::from).collect())
        }
        _ => Err(Error::msg(format!(
            "Listing reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

fn filter_prs(prs: Vec<but_github::PullRequest>, filter: &ForgeReviewFilter) -> Vec<but_github::PullRequest> {
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
                        let week_start = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
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

fn filter_mrs(mrs: Vec<but_gitlab::MergeRequest>, filter: &ForgeReviewFilter) -> Vec<but_gitlab::MergeRequest> {
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
                        let week_start = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
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

/// Get a specific review (e.g. pull request) for a given forge repository
pub async fn get_forge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReview> {
    let crate::forge::ForgeRepoInfo { forge, owner, repo, .. } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr = but_github::pr::get(preferred_account, owner, repo, review_number, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        ForgeName::GitLab => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let project_id = GitLabProjectId::new(owner, repo);
            let mr = but_gitlab::mr::get(preferred_account, project_id, review_number, storage).await?;
            Ok(ForgeReview::from(mr))
        }
        _ => Err(Error::msg(format!(
            "Getting reviews for forge {forge:?} is not implemented yet.",
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

/// Create a new review (e.g. pull request) for a given forge repository
pub async fn create_forge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    params: &CreateForgeReviewParams,
    storage: &but_forge_storage::Controller,
) -> Result<ForgeReview> {
    let crate::forge::ForgeRepoInfo { forge, owner, repo, .. } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            // TODO: handle forks better
            let head = format!("{}:{}", owner, params.source_branch);
            let pr_params = but_github::CreatePullRequestParams {
                owner,
                repo,
                title: &params.title,
                body: &params.body,
                head: &head,
                base: &params.target_branch,
                draft: params.draft,
            };
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr = but_github::pr::create(preferred_account, pr_params, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        ForgeName::GitLab => {
            let project_id = GitLabProjectId::new(owner, repo);
            // TODO: handle forks better
            // TODO: handle draft properly
            let mr_params = but_gitlab::CreateMergeRequestParams {
                project_id,
                title: &params.title,
                body: &params.body,
                source_branch: &params.source_branch,
                target_branch: &params.target_branch,
            };
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let mr = but_gitlab::mr::create(preferred_account, mr_params, storage).await?;
            Ok(ForgeReview::from(mr))
        }
        _ => Err(Error::msg(format!(
            "Creating reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewDescriptionUpdate {
    /// The unique identifier number for this review within its repository. This can be a PR or MR number.
    pub number: i64,
    /// The current body/description of the review, which may be None if no description is set.
    pub body: Option<String>,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for MRs).
    pub unit_symbol: String,
}

impl From<ForgeReview> for ForgeReviewDescriptionUpdate {
    fn from(review: ForgeReview) -> Self {
        ForgeReviewDescriptionUpdate {
            number: review.number,
            body: review.body,
            unit_symbol: review.unit_symbol,
        }
    }
}

/// Update the review description tables for a set of reviews
pub async fn update_review_description_tables(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    reviews: &[ForgeReviewDescriptionUpdate],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let crate::forge::ForgeRepoInfo { forge, owner, repo, .. } = forge_repo_info;

    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_numbers: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let updated_body = update_body(review.body.as_deref(), review.number, &pr_numbers, &review.unit_symbol);

                let params = but_github::UpdatePullRequestParams {
                    owner,
                    repo,
                    pr_number: review.number,
                    title: None,
                    body: Some(&updated_body),
                    base: None,
                    state: None,
                };

                but_github::pr::update(preferred_account, params, storage).await?;
            }

            Ok(())
        }
        _ => Err(Error::msg(format!(
            "Updating review descriptions for forge {forge:?} is not implemented yet.",
        ))),
    }
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
fn update_body(body: Option<&str>, pr_number: i64, all_pr_numbers: &[i64], symbol: &str) -> String {
    let body = body.unwrap_or("");
    let head = body.split(STACKING_FOOTER_BOUNDARY_TOP).next().unwrap_or("").trim();
    let tail = body.split(STACKING_FOOTER_BOUNDARY_BOTTOM).nth(1).unwrap_or("").trim();

    // If there's only one PR, don't add a footer
    if all_pr_numbers.len() == 1 {
        if tail.is_empty() {
            return head.to_string();
        }
        return format!("{head}\n\n{tail}");
    }

    let footer = generate_footer(pr_number, all_pr_numbers, symbol);
    if tail.is_empty() {
        format!("{head}\n\n{footer}")
    } else {
        format!("{head}\n\n{footer}\n\n{tail}")
    }
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
fn generate_footer(for_pr_number: i64, all_pr_numbers: &[i64], symbol: &str) -> String {
    let stack_length = all_pr_numbers.len();
    let stack_index = all_pr_numbers.iter().position(|&n| n == for_pr_number).unwrap_or(0);
    let nth = stack_length - stack_index;

    let mut footer = String::new();
    footer.push_str(STACKING_FOOTER_BOUNDARY_TOP);
    footer.push('\n');
    footer.push_str("---\n");
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
        assert!(is_valid_review_template_path_github(p("PULL_REQUEST_TEMPLATE.md")));
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
        assert!(is_valid_review_template_path_github(p("PULL_REQUEST_TEMPLATE.md"),));
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
        assert!(is_review_template_gitlab(".gitlab/merge_request_templates/Default.md"));
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
        assert!(!is_review_template_gitlab("merge_request_templates/Default.md"));

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

        assert!(footer.contains("part 6 of 10 in a stack"));

        // Verify all PRs are listed
        for pr in &all_prs {
            assert!(footer.contains(&format!("#{pr}")));
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
        assert!(result.ends_with("Tail content"));
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
}
