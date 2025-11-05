use std::path::{self};

use anyhow::{Error, Result};
use gitbutler_fs::list_files;
use serde::{Deserialize, Serialize};

use crate::forge::ForgeName;

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
#[serde(rename_all = "camelCase")]
pub struct ForgeReviewLabel {
    pub name: String,
    pub description: Option<String>,
    pub color: String,
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
            reviewers: pr
                .requested_reviewers
                .into_iter()
                .map(ForgeUser::from)
                .collect(),
            unit_symbol: "#".to_string(),
        }
    }
}

/// List the open reviews (e.g. pull requests) for a given forge repository
pub async fn list_forge_reviews(
    preferred_forge_user: &Option<crate::ForgeUser>,
    forge_repo_info: &crate::forge::ForgeRepoInfo,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<ForgeReview>> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
    match forge {
        ForgeName::GitHub => {
            let preferred_account = preferred_forge_user.as_ref().and_then(|user| user.github());
            let pulls = but_github::pr::list(preferred_account, owner, repo, storage).await?;
            Ok(pulls.into_iter().map(ForgeReview::from).collect())
        }
        _ => Err(Error::msg(format!(
            "Listing reviews for forge {:?} is not implemented yet.",
            forge,
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
    storage: &but_forge_storage::controller::Controller,
) -> Result<ForgeReview> {
    let crate::forge::ForgeRepoInfo {
        forge, owner, repo, ..
    } = forge_repo_info;
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
        _ => Err(Error::msg(format!(
            "Creating reviews for forge {:?} is not implemented yet.",
            forge,
        ))),
    }
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
}
