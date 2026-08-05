use anyhow::{Context as _, Result};

use crate::client::{GitHubClient, HttpStatusError};

pub async fn list(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gh) = GitHubClient::from_storage(storage, preferred_account) {
        gh.list_open_pulls(owner, repo)
            .await
            .map_err(classify_forge_error)
            .context("Failed to list open pull requests")
    } else {
        Ok(vec![])
    }
}
pub async fn list_all_for_branch(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gh) = GitHubClient::from_storage(storage, preferred_account) {
        gh.list_pulls_for_base(owner, repo, branch)
            .await
            .map_err(classify_forge_error)
            .context("Failed to list pull requests for branch")
    } else {
        Ok(vec![])
    }
}

pub async fn list_for_commit(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    if let Ok(gh) = GitHubClient::from_storage(storage, preferred_account) {
        gh.list_pulls_for_commit(owner, repo, commit_sha)
            .await
            .map_err(classify_forge_error)
            .context("Failed to list pull requests for commit")
    } else {
        Ok(vec![])
    }
}

/// Tag transport / auth failures with a `but_error::Code` so the desktop
/// can present them appropriately (silent for offline, re-auth hint for 401).
/// Only applied to read paths — mutations should still surface failures.
pub(crate) fn classify_forge_error(err: anyhow::Error) -> anyhow::Error {
    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>()
        && crate::is_network_error(reqwest_err)
    {
        return err.context(but_error::Context::new_static(
            but_error::Code::NetworkError,
            "Unable to connect to GitHub.",
        ));
    }
    if let Some(http_err) = err.downcast_ref::<HttpStatusError>()
        && http_err.status == reqwest::StatusCode::UNAUTHORIZED
    {
        return err.context(but_error::Context::new_static(
            but_error::Code::GitHubTokenExpired,
            "GitHub authentication failed.",
        ));
    }
    err
}

pub async fn create(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::CreatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr = GitHubClient::from_storage(storage, preferred_account)?
        .create_pull_request(&params)
        .await
        .context("Failed to create pull request")?;
    Ok(pr)
}

pub async fn get(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    let pr = GitHubClient::from_storage(storage, preferred_account)?
        .get_pull_request(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to get pull request")?;
    Ok(pr)
}

pub async fn list_comments(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequestComment>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pull_request_comments(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull request comments")
}

pub async fn list_timeline_events(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequestTimelineEvent>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pull_request_timeline(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull request timeline events")
}

pub async fn list_review_reactions(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::Reaction>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pull_request_reactions(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull request reactions")
}

pub async fn list_comment_reactions(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    comment_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::Reaction>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_comment_reactions(owner, repo, comment_id)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list comment reactions")
}

pub async fn add_review_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    content: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::Reaction> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .add_pull_request_reaction(owner, repo, pr_number, content)
        .await
        .context("Failed to add pull request reaction")
}

pub async fn remove_review_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    reaction_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .delete_pull_request_reaction(owner, repo, pr_number, reaction_id)
        .await
        .context("Failed to remove pull request reaction")
}

pub async fn add_comment_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    comment_id: i64,
    content: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::Reaction> {
    GitHubClient::from_storage(storage, preferred_account)?
        .add_comment_reaction(owner, repo, comment_id, content)
        .await
        .context("Failed to add comment reaction")
}

pub async fn remove_comment_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    comment_id: i64,
    reaction_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .delete_comment_reaction(owner, repo, comment_id, reaction_id)
        .await
        .context("Failed to remove comment reaction")
}

pub async fn list_repo_labels(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::GitHubPrLabel>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_repo_labels(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list repository labels")
}

pub async fn add_labels(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    labels: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::GitHubPrLabel>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .add_labels_to_pull_request(owner, repo, pr_number, labels)
        .await
        .context("Failed to add labels to pull request")
}

pub async fn remove_label(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    label: &str,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .remove_label_from_pull_request(owner, repo, pr_number, label)
        .await
        .context("Failed to remove label from pull request")
}

pub async fn list_reviewer_candidates(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::GitHubUser>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_assignable_users(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list reviewer candidates")
}

pub async fn request_reviewers(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    reviewers: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .request_reviewers(owner, repo, pr_number, reviewers)
        .await
        .context("Failed to request reviewers")
}

pub async fn remove_requested_reviewers(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    reviewers: &[String],
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .remove_requested_reviewers(owner, repo, pr_number, reviewers)
        .await
        .context("Failed to withdraw review request")
}

pub async fn update_comment(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    comment_id: i64,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequestComment> {
    GitHubClient::from_storage(storage, preferred_account)?
        .update_pull_request_comment(owner, repo, comment_id, body)
        .await
        .context("Failed to update pull request comment")
}

pub async fn delete_comment(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    comment_id: i64,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .delete_pull_request_comment(owner, repo, comment_id)
        .await
        .context("Failed to delete pull request comment")
}

pub async fn list_pr_reviews(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequestReview>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pull_request_reviews(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull request reviews")
}

pub async fn create_comment(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequestComment> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .create_pull_request_comment(owner, repo, pr_number, body)
        .await
        .context("Failed to create pull request comment")
}

pub async fn update(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::UpdatePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequest> {
    let pr = GitHubClient::from_storage(storage, preferred_account)?
        .update_pull_request(&params)
        .await
        .context("Failed to update pull request")?;
    Ok(pr)
}

pub async fn merge(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::MergePullRequestParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .merge_pull_request(&params)
        .await
        .context("Failed to merge PR")
}

pub async fn set_draft_state(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::SetPullRequestDraftStateParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .set_pull_request_draft_state(&params)
        .await
        .context("Failed to update PR draft state")
}

pub async fn set_auto_merge(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    params: crate::client::SetPullRequestAutoMergeParams<'_>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .set_pull_request_auto_merge(&params)
        .await
        .context("Failed to update PR auto-merge state")
}
