use std::{
    fmt::Display,
    path::{self},
};

use anyhow::{Context as _, Error, Result};
use but_fs::list_files;
use but_github::CredentialCheckResult;
use but_gitea::CredentialCheckResult as GiteaCredentialCheckResult;
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
                SupportedTemplateDirectory::Custom(".github"),
                SupportedTemplateDirectory::Custom(".docs"),
                SupportedTemplateDirectory::ProjectRoot,
            ],
        },
        ForgeName::GitLab => ReviewTemplateFunctions {
            is_review_template: is_review_template_gitlab,
            get_root: get_gitlab_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_gitlab,
            supported_template_directories: &[
                SupportedTemplateDirectory::ForgeRoot,
                SupportedTemplateDirectory::Custom(".gitlab"),
                SupportedTemplateDirectory::ProjectRoot,
            ],
        },
        ForgeName::Gitea => ReviewTemplateFunctions {
            is_review_template: is_review_template_gitea,
            get_root: get_gitea_directory_path,
            is_valid_review_template_path: is_valid_review_template_path_gitea,
            supported_template_directories: &[
                SupportedTemplateDirectory::ForgeRoot,
                SupportedTemplateDirectory::Custom(".gitea"),
                SupportedTemplateDirectory::ProjectRoot,
            ],
        },
    }
}

pub fn is_review_template_github(file_name: &str) -> bool {
    let lower_file_name = file_name.to_lowercase();
    lower_file_name.contains("pull_request_template") && lower_file_name.ends_with(".md")
}

pub fn is_review_template_gitlab(file_name: &str) -> bool {
    let lower_file_name = file_name.to_lowercase();
    lower_file_name.contains("merge_request_template") && lower_file_name.ends_with(".md")
}

pub fn is_review_template_gitea(file_name: &str) -> bool {
    let lower_file_name = file_name.to_lowercase();
    lower_file_name.contains("pull_request_template") && lower_file_name.ends_with(".md")
}

pub fn get_github_directory_path(root_path: &path::Path) -> path::PathBuf {
    root_path.join(".github")
}

pub fn get_gitlab_directory_path(root_path: &path::Path) -> path::PathBuf {
    root_path.join(".gitlab")
}

pub fn get_gitea_directory_path(root_path: &path::Path) -> path::PathBuf {
    root_path.join(".gitea")
}

pub fn is_valid_review_template_path_github(file_path: &path::Path) -> bool {
    let components: Vec<_> = file_path.components().collect();

    // Check if it's in the root
    if components.len() == 1 {
        let file_name = components[0].as_os_str().to_string_lossy();
        return is_review_template_github(&file_name);
    }

    // Check if it's in .github or .docs
    if components.len() >= 2 {
        let dir_name = components[0].as_os_str().to_string_lossy();
        let file_name = components[components.len() - 1]
            .as_os_str()
            .to_string_lossy();

        if (dir_name == ".github" || dir_name == ".docs") && is_review_template_github(&file_name) {
            // Special case for .github/PULL_REQUEST_TEMPLATE/
            if dir_name == ".github" && components.len() == 3 {
                let sub_dir_name = components[1].as_os_str().to_string_lossy();
                return sub_dir_name == "PULL_REQUEST_TEMPLATE";
            }
            return components.len() == 2;
        }
    }

    false
}

pub fn is_valid_review_template_path_gitlab(file_path: &path::Path) -> bool {
    let components: Vec<_> = file_path.components().collect();

    // Check if it's in the root
    if components.len() == 1 {
        let file_name = components[0].as_os_str().to_string_lossy();
        return is_review_template_gitlab(&file_name);
    }

    // Check if it's in .gitlab
    if components.len() >= 2 {
        let dir_name = components[0].as_os_str().to_string_lossy();
        let file_name = components[components.len() - 1]
            .as_os_str()
            .to_string_lossy();

        if dir_name == ".gitlab" && is_review_template_gitlab(&file_name) {
            // Special case for .gitlab/merge_request_templates/
            if components.len() == 3 {
                let sub_dir_name = components[1].as_os_str().to_string_lossy();
                return sub_dir_name == "merge_request_templates";
            }
            return components.len() == 2;
        }
    }

    false
}

pub fn is_valid_review_template_path_gitea(file_path: &path::Path) -> bool {
    let components: Vec<_> = file_path.components().collect();

    // Check if it's in the root
    if components.len() == 1 {
        let file_name = components[0].as_os_str().to_string_lossy();
        return is_review_template_gitea(&file_name);
    }

    // Check if it's in .gitea
    if components.len() >= 2 {
        let dir_name = components[0].as_os_str().to_string_lossy();
        let file_name = components[components.len() - 1]
            .as_os_str()
            .to_string_lossy();

        if dir_name == ".gitea" && is_review_template_gitea(&file_name) {
            // Special case for .gitea/pull_request_template/
            if components.len() == 3 {
                let sub_dir_name = components[1].as_os_str().to_string_lossy();
                return sub_dir_name == "pull_request_template";
            }
            return components.len() == 2;
        }
    }

    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ForgeReviewUser {
    pub username: String,
    pub name: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewUser);

impl From<but_github::User> for ForgeReviewUser {
    fn from(user: but_github::User) -> Self {
        ForgeReviewUser {
            username: user.login,
            name: user.name,
        }
    }
}

impl From<but_gitlab::User> for ForgeReviewUser {
    fn from(user: but_gitlab::User) -> Self {
        ForgeReviewUser {
            username: user.username,
            name: Some(user.name),
        }
    }
}

impl From<but_gitea::User> for ForgeReviewUser {
    fn from(user: but_gitea::User) -> Self {
        ForgeReviewUser {
            username: user.login,
            name: Some(user.full_name),
        }
    }
}

impl Display for ForgeReviewUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{} ({})", name, self.username),
            None => write!(f, "{}", self.username),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ForgeReviewLabel {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewLabel);

impl From<but_github::Label> for ForgeReviewLabel {
    fn from(label: but_github::Label) -> Self {
        ForgeReviewLabel {
            name: label.name,
            color: label.color,
            description: label.description,
        }
    }
}

impl From<but_gitlab::Label> for ForgeReviewLabel {
    fn from(label: but_gitlab::Label) -> Self {
        ForgeReviewLabel {
            name: label.name,
            color: label.color.trim_start_matches('#').to_string(),
            description: label.description,
        }
    }
}

impl From<but_gitea::Label> for ForgeReviewLabel {
    fn from(label: but_gitea::Label) -> Self {
        ForgeReviewLabel {
            name: label.name,
            color: label.color.trim_start_matches('#').to_string(),
            description: label.description,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub struct ForgeReview {
    pub html_url: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub author: Option<ForgeReviewUser>,
    pub labels: Vec<ForgeReviewLabel>,
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
    pub reviewers: Vec<ForgeReviewUser>,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for GitLab merge requests).
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

impl From<but_gitea::PullRequest> for ForgeReview {
    fn from(pr: but_gitea::PullRequest) -> Self {
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
            created_at: pr.created_at,
            modified_at: pr.modified_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: vec![],
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
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
            created_at: pr.created_at,
            modified_at: pr.modified_at,
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
            repository_ssh_url: pr.repository_ssh_url,
            repository_https_url: pr.repository_https_url,
            repo_owner: pr.repo_owner,
            reviewers: pr
                .requested_reviewers
                .into_iter()
                .map(ForgeReviewUser::from)
                .collect(),
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
            created_at: mr.created_at,
            modified_at: mr.updated_at,
            merged_at: mr.merged_at,
            closed_at: mr.closed_at,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            reviewers: mr
                .reviewers
                .into_iter()
                .map(ForgeReviewUser::from)
                .collect(),
            unit_symbol: "!".to_string(),
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
    NoCredentials,
    Invalid,
}

impl From<CredentialCheckResult> for ForgeAccountValidity {
    fn from(value: CredentialCheckResult) -> Self {
        match value {
            CredentialCheckResult::Invalid => ForgeAccountValidity::Invalid,
            CredentialCheckResult::NoCredentials => ForgeAccountValidity::NoCredentials,
            CredentialCheckResult::Valid => ForgeAccountValidity::Valid,
        }
    }
}

impl From<GiteaCredentialCheckResult> for ForgeAccountValidity {
    fn from(value: GiteaCredentialCheckResult) -> Self {
        match value {
            GiteaCredentialCheckResult::Invalid => ForgeAccountValidity::Invalid,
            GiteaCredentialCheckResult::NoCredentials => ForgeAccountValidity::NoCredentials,
            GiteaCredentialCheckResult::Valid => ForgeAccountValidity::Valid,
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
        ForgeName::Gitea => {
            let preferred_account = match preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitea().cloned())
            {
                Some(account) => account,
                None => {
                    let known_accounts = but_gitea::list_known_gitea_accounts(storage)?;
                    match known_accounts.first() {
                        Some(account) => account.clone(),
                        None => {
                            return Ok(ForgeAccountValidity::NoCredentials);
                        }
                    }
                }
            };

            but_gitea::check_credentials(&preferred_account, storage)
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
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitea().cloned());

            // Clone owned data for thread
            let owner = owner.clone();
            let repo = repo.clone();
            let storage = storage.clone();

            let pulls = std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(but_gitea::pr::list(
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

/// List all reviews for a repository and branch
pub async fn list_forge_reviews_for_branch(
    preferred_forge_user: Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    branch: &str,
    filter: ForgeReviewFilter,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgeReview>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github().cloned());

            let prs = but_github::pr::list_all_for_target(
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
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitea().cloned());
            let prs = but_gitea::pr::list(
                preferred_account.as_ref(),
                owner,
                repo,
                storage,
            )
            .await?;
            let prs: Vec<_> = prs.into_iter().filter(|pr| pr.target_branch == branch).collect();
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
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitea());
            let pr =
                but_gitea::pr::get(preferred_account, owner, repo, review_number, storage).await?;
            Ok(ForgeReview::from(pr))
        }
        _ => Err(Error::msg(format!(
            "Getting reviews for forge {forge:?} is not implemented yet.",
        ))),
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

/// Merge a review to it's target branch
pub async fn merge_review(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    review_number: usize,
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
                merge_method: None,
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
                merge_commit_message: None,
                squash_commit_message: None,
                squash: None,
            };

            but_gitlab::mr::merge(preferred_account, params, storage).await
        }
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitea());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_gitea::MergePullRequestParams {
                owner,
                repo,
                pr_number,
                merge_method: None,
            };
            but_gitea::pr::merge(preferred_account, params, storage).await
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
        ForgeName::Gitea => {
            // Gitea doesn't seem to have a simple auto-merge API in the same way.
            Err(Error::msg(format!(
                "Setting the auto-merge state of reviews for forge {forge:?} is not implemented yet.",
            )))
        }
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
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitea());
            let pr_number = review_number
                .try_into()
                .context("PR: Failed to cast usize to i64, somehow")?;
            let params = but_gitea::SetPullRequestDraftStateParams {
                owner,
                repo,
                pr_number,
                draft,
            };
            but_gitea::pr::set_draft_state(preferred_account, params, storage).await
        }
        _ => Err(Error::msg(format!(
            "Setting the draftiness of reviews for forge {forge:?} is not implemented yet.",
        ))),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateForgeReviewParams {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CreateForgeReviewParams);

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
    forge_push_repo_info: &Option<crate::ForgeRepoInfo>,
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
        ForgeName::Gitea => {
            let pr_params = but_gitea::CreatePullRequestParams {
                owner,
                repo,
                title: &params.title,
                body: &params.body,
                head: &params.source_branch,
                base: &params.target_branch,
            };
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitea());
            let pr = but_gitea::pr::create(preferred_account, pr_params, storage).await?;
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
pub struct ForgeReviewDescriptionUpdate {
    /// The unique identifier number for this review within its repository. This can be a PR or MR number.
    pub number: i64,
    /// The current body/description of the review, which may be None if no description is set.
    pub body: Option<String>,
    /// The platform-specific symbol for this review type (e.g., "#" for GitHub pull requests and "!" for MRs).
    pub unit_symbol: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeReviewDescriptionUpdate);

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
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;

    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pr_numbers: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let updated_body = update_body(
                    review.body.as_deref(),
                    review.number,
                    &pr_numbers,
                    &review.unit_symbol,
                );

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
        ForgeName::GitLab => {
            let project_id = GitLabProjectId::new(owner, repo);
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            let mr_iids: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let updated_body = update_body(
                    review.body.as_deref(),
                    review.number,
                    &mr_iids,
                    &review.unit_symbol,
                );

                let params = but_gitlab::UpdateMergeRequestParams {
                    project_id: project_id.clone(),
                    mr_iid: review.number,
                    title: None,
                    description: Some(&updated_body),
                    target_branch: None,
                    state_event: None,
                };

                but_gitlab::mr::update(preferred_account, params, storage).await?;
            }

            Ok(())
        }
        ForgeName::Gitea => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.gitea());
            let pr_numbers: Vec<i64> = reviews.iter().map(|r| r.number).collect();

            for review in reviews {
                let updated_body = update_body(
                    review.body.as_deref(),
                    review.number,
                    &pr_numbers,
                    &review.unit_symbol,
                );

                let params = but_gitea::UpdatePullRequestParams {
                    owner,
                    repo,
                    pr_number: review.number,
                    title: None,
                    body: Some(&updated_body),
                };

                but_gitea::pr::update(preferred_account, params, storage).await?;
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
/// * `body` - The existing PR body text (max be None or empty)
/// * `pr_number` - The PR number for which to update the body
/// * `all_pr_numbers` - All PR numbers in the stack (ordered from base to top)
/// * `symbol` - The symbol to use before the PR number (e.g., "#" or "!")
///
/// # Returns
/// The updated body with the footer replaced, inserted, or removed
fn update_body(body: Option<&str>, pr_number: i64, all_pr_numbers: &[i64], symbol: &str) -> String {
    let body = body.unwrap_or("");
    let head = body
        .split(STACKING_FOOTER_BOUNDARY_TOP)
        .next()
        .unwrap_or("")
        .trim();
    let tail = body
        .split(STACKING_FOOTER_BOUNDARY_BOTTOM)
        .nth(1)
        .unwrap_or("")
        .trim();

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
    let stack_index = all_pr_numbers
        .iter()
        .position(|&n| n == for_pr_number)
        .unwrap_or(0);
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
            ".gitlab/merge_request_templates/something.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab/merge_request_template.md"
        )));
        assert!(is_valid_review_template_path_gitlab(p(
            "merge_request_template.md"
        )));
        assert!(!is_valid_review_template_path_gitlab(p("README.md"),));
    }

    #[test]
    fn test_is_valid_review_template_path_gitlab_windows() {
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_templates\\something.md"
        ),));
        assert!(is_valid_review_template_path_gitlab(p(
            ".gitlab\\merge_request_template.md"
        ),));
        assert!(is_valid_review_template_path_gitlab(p(
            "merge_request_template.md"
        ),));
        assert!(!is_valid_review_template_path_gitlab(p("README.md"),));
    }

    #[test]
    fn test_is_valid_review_template_path_gitea() {
        assert!(is_valid_review_template_path_gitea(p(
            ".gitea/pull_request_template/something.md"
        )));
        assert!(is_valid_review_template_path_gitea(p(
            ".gitea/pull_request_template.md"
        )));
        assert!(is_valid_review_template_path_gitea(p(
            "pull_request_template.md"
        )));
        assert!(!is_valid_review_template_path_gitea(p("README.md"),));
    }

    #[test]
    fn test_is_valid_review_template_path_gitea_windows() {
        assert!(is_valid_review_template_path_gitea(p(
            ".gitea\\pull_request_template\\something.md"
        ),));
        assert!(is_valid_review_template_path_gitea(p(
            ".gitea\\pull_request_template.md"
        ),));
        assert!(is_valid_review_template_path_gitea(p(
            "pull_request_template.md"
        ),));
        assert!(!is_valid_review_template_path_gitea(p("README.md"),));
    }

    #[test]
    fn test_update_body() {
        let old_footer = generate_footer(123, &[123, 124], "#");
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

        assert!(result.contains("Head content"));
        assert!(result.contains("Tail content"));
        assert!(!result.contains(STACKING_FOOTER_BOUNDARY_TOP));
    }
}
