use anyhow::{Context as _, Result};

use crate::client::{GitHubClient, HttpStatusError};

const GITHUB_RATE_LIMIT_MESSAGE: &str = "GitHub's API rate limit was exceeded. Automatic refreshes are paused; the limit usually resets within an hour.";
const GITHUB_ORG_SAML_RESTRICTION_MESSAGE: &str = "This GitHub organization requires SAML SSO. Authorize the GitButler OAuth app on the organization's SSO page, or authorize your personal access token in GitHub's token SSO settings, then try again.";
/// The wording shared by GitHub's classic and fine-grained token-lifetime
/// refusals; the surrounding sentence names the organization and a token URL,
/// which never reach the user.
const GITHUB_TOKEN_LIFETIME_PHRASE: &str = "if the token's lifetime is greater than";
const GITHUB_TOKEN_LIFETIME_RESTRICTION_MESSAGE: &str = "A GitHub organization limits how long personal access tokens may stay valid. Create a token with a shorter expiration that meets the organization's policy, then reconnect GitHub with it.";
const GITHUB_IP_ALLOW_LIST_MESSAGE: &str = "A GitHub organization's IP allow list blocks access from your current network. Connect from an allowed network or IP address, or ask an organization owner to add your address to the allow list, then try again.";
pub async fn list(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_open_pulls(owner, repo)
        .await
        .map_err(classify_review_list_error)
        .context("Failed to list open pull requests")
}

/// A 404 on the open-PR listing means the repository is gone or invisible to
/// this account, so retrying cannot succeed: it is tagged as a permission
/// problem right here. Every other error goes through [`classify_forge_error`].
fn classify_review_list_error(err: anyhow::Error) -> anyhow::Error {
    if err
        .downcast_ref::<HttpStatusError>()
        .is_some_and(|http_err| http_err.status == reqwest::StatusCode::NOT_FOUND)
    {
        return err.context(but_error::Context::new_static(
            but_error::Code::GitHubInsufficientPermissions,
            "GitHub could not find this repository. Check that it still exists and that your account can access it.",
        ));
    }
    classify_forge_error(err)
}
pub async fn list_recently_closed(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_recently_closed_pulls(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list recently closed pull requests")
}

pub async fn list_all_for_branch(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    branch: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pulls_for_base(owner, repo, branch)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull requests for branch")
}

pub async fn list_for_commit(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequest>> {
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pulls_for_commit(owner, repo, commit_sha)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull requests for commit")
}

/// Tag selected transport, auth, and permission failures with a
/// `but_error::Code` so callers can present actionable guidance.
pub(crate) fn classify_forge_error(err: anyhow::Error) -> anyhow::Error {
    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>()
        && crate::is_network_error(reqwest_err)
    {
        return err.context(but_error::Context::new_static(
            but_error::Code::NetworkError,
            "Unable to connect to GitHub.",
        ));
    }
    if let Some(http_err) = err.downcast_ref::<HttpStatusError>() {
        if http_err.status == reqwest::StatusCode::UNAUTHORIZED {
            return err.context(but_error::Context::new_static(
                but_error::Code::GitHubTokenExpired,
                "GitHub authentication failed.",
            ));
        }
        // `ensure_success` keeps GitHub's response body in the chain.
        let contains = |needle: &str| err.chain().any(|cause| cause.to_string().contains(needle));
        // Primary limits answer 403 "API rate limit exceeded for …"; secondary
        // limits answer 403 or 429 "You have exceeded a secondary rate limit".
        if http_err.status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || (http_err.status == reqwest::StatusCode::FORBIDDEN && contains("rate limit"))
        {
            return err.context(but_error::Context::new_static(
                but_error::Code::GitHubRateLimited,
                GITHUB_RATE_LIMIT_MESSAGE,
            ));
        }
        if http_err.status == reqwest::StatusCode::FORBIDDEN {
            let context = if contains("OAuth App access restrictions") {
                Some(but_error::Context::new_static(
                    but_error::Code::GitHubOrgOAuthRestricted,
                    "A GitHub organization has restricted access for the GitButler OAuth app. Ask an organization owner to approve it, or authenticate with a personal access token instead.",
                ))
            } else if contains("Resource protected by organization SAML enforcement") {
                Some(but_error::Context::new_static(
                    but_error::Code::GitHubOrgSamlRestricted,
                    GITHUB_ORG_SAML_RESTRICTION_MESSAGE,
                ))
            } else if contains(GITHUB_TOKEN_LIFETIME_PHRASE) {
                Some(but_error::Context::new_static(
                    but_error::Code::GitHubTokenLifetimeRestricted,
                    GITHUB_TOKEN_LIFETIME_RESTRICTION_MESSAGE,
                ))
            } else if contains("IP allow list enabled")
                && contains("not permitted to access this resource")
            {
                Some(but_error::Context::new_static(
                    but_error::Code::GitHubInsufficientPermissions,
                    GITHUB_IP_ALLOW_LIST_MESSAGE,
                ))
            } else if contains("Resource not accessible by personal access token") {
                Some(but_error::Context::new_static(
                    but_error::Code::GitHubInsufficientPermissions,
                    "Your GitHub token doesn't have permission to read this. Grant the token the missing repository read permission (such as Checks), or reconnect GitHub with different credentials.",
                ))
            } else {
                None
            };
            if let Some(context) = context {
                return err.context(context);
            }
        }
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

pub async fn get_merge_status(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequestMergeStatus> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .get_pull_request_merge_status(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to fetch PR merge status")
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

pub async fn add_pr_review_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    review_id: i64,
    content: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::Reaction> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .add_pull_request_review_reaction(owner, repo, pr_number, review_id, content)
        .await
        .map_err(classify_forge_error)
        .context("Failed to add review reaction")
}

pub async fn remove_pr_review_reaction(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    review_id: i64,
    content: &str,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .remove_pull_request_review_reaction(owner, repo, pr_number, review_id, content)
        .await
        .map_err(classify_forge_error)
        .context("Failed to remove review reaction")
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
    let client = GitHubClient::from_storage(storage, preferred_account)?;
    let mut users = client
        .list_assignable_users(owner, repo)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list reviewer candidates")?;
    // Best effort: a failed profile lookup must not take the collaborators with it.
    match client.get_copilot_reviewer().await {
        Ok(copilot) => users.extend(copilot),
        Err(err) => tracing::warn!("Skipping Copilot as a reviewer candidate: {err:#}"),
    }
    Ok(users)
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

/// Set the resolution state of a review conversation.
pub async fn set_review_thread_resolved(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    thread_id: &str,
    resolved: bool,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    GitHubClient::from_storage(storage, preferred_account)?
        .set_review_thread_resolved(thread_id, resolved)
        .await
        .map_err(classify_forge_error)
        .context("Failed to change review thread resolution")
}

/// Reply into an existing review thread, returning the comment it made.
pub async fn create_review_thread_reply(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    thread_id: &str,
    body: &str,
    storage: &but_forge_storage::Controller,
) -> Result<crate::client::PullRequestReviewThreadComment> {
    GitHubClient::from_storage(storage, preferred_account)?
        .add_review_thread_reply(thread_id, body)
        .await
        .map_err(classify_forge_error)
        .context("Failed to reply to the review thread")
}

/// List the diff-anchored review threads on a pull request, oldest first.
pub async fn list_review_threads(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: usize,
    storage: &but_forge_storage::Controller,
) -> Result<Vec<crate::client::PullRequestReviewThread>> {
    let pr_number = pr_number.try_into().context("PR number is too large")?;
    GitHubClient::from_storage(storage, preferred_account)?
        .list_pull_request_review_threads(owner, repo, pr_number)
        .await
        .map_err(classify_forge_error)
        .context("Failed to list pull request review threads")
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
    let client = GitHubClient::from_storage(storage, preferred_account)?;
    update_with_client(&client, &params).await
}

async fn update_with_client(
    client: &GitHubClient,
    params: &crate::client::UpdatePullRequestParams<'_>,
) -> Result<crate::client::PullRequest> {
    let pr = client
        .update_pull_request(params)
        .await
        .map_err(classify_forge_error)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn update_classifies_saml_refusal_and_preserves_the_error_chain() {
        use std::io::{Read as _, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let body = r#"{"message":"Resource protected by organization SAML enforcement. You must grant your OAuth token access to this organization."}"#;
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "request timed out");
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("failed to accept request: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let mut buffer = [0; 1024];
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "request ended before its headers");
                request.extend_from_slice(&buffer[..read]);
            }
            let request = String::from_utf8_lossy(&request);
            assert!(
                request.starts_with("PATCH /repos/o/r/pulls/7 "),
                "the update uses the pull request mutation endpoint"
            );
            write!(
                stream,
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let client = GitHubClient::new_with_host_override(
            &but_secret::Sensitive("token".to_string()),
            &format!("http://{address}"),
        )
        .unwrap();
        let params = crate::client::UpdatePullRequestParams {
            owner: "o",
            repo: "r",
            pr_number: 7,
            title: None,
            body: None,
            base: Some("main"),
            state: None,
        };

        let err = update_with_client(&client, &params).await.unwrap_err();
        server.join().unwrap();

        let context = err
            .downcast_ref::<but_error::Context>()
            .expect("the update path classifies the SAML refusal");
        assert_eq!(
            (context.code, context.message.as_deref()),
            (
                but_error::Code::GitHubOrgSamlRestricted,
                Some(GITHUB_ORG_SAML_RESTRICTION_MESSAGE)
            ),
            "the update reports the canonical code and static guidance"
        );
        assert!(
            err.downcast_ref::<HttpStatusError>()
                .is_some_and(|cause| cause.status == reqwest::StatusCode::FORBIDDEN),
            "the original HTTP status remains in the chain"
        );
        assert!(
            err.chain().any(|cause| cause
                .to_string()
                .contains("Resource protected by organization SAML enforcement")),
            "the original provider refusal remains in the chain"
        );
    }

    /// Shape the error like `ensure_success` does: the status-carrying error
    /// wrapped by what the forge said in the response body.
    fn http_error(status: reqwest::StatusCode, body: &str) -> anyhow::Error {
        anyhow::Error::from(HttpStatusError { status }).context(body.to_string())
    }

    #[test]
    fn org_oauth_restriction_403_gets_dedicated_code() {
        let err = classify_forge_error(http_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"403 Forbidden: {"message":"Although you appear to have the correct authorization credentials, the organization has enabled OAuth App access restrictions."}"#,
        ));
        let ctx = err.downcast_ref::<but_error::Context>();
        assert_eq!(
            ctx.map(|c| c.code),
            Some(but_error::Code::GitHubOrgOAuthRestricted),
            "the frontend keys its presentation off this code"
        );
    }

    #[test]
    fn saml_enforcement_403_gets_dedicated_code_and_static_message() {
        let bodies = [
            r#"403 Forbidden: {"message":"Resource protected by organization SAML enforcement. You must grant your OAuth token access to this organization."}"#,
            r#"403 Forbidden: {"message":"Resource protected by organization SAML enforcement. You must grant your OAuth token access to an organization within this enterprise. Visit https://example.invalid/orgs/example/sso?authorization_request=redacted and try again."}"#,
            // Compatibility fixture for GitHub's PAT-token wording.
            r#"403 Forbidden: {"message":"Resource protected by organization SAML enforcement. You must grant your Personal Access token access to this organization."}"#,
        ];
        for body in bodies {
            let err = classify_forge_error(http_error(reqwest::StatusCode::FORBIDDEN, body));
            let ctx = err
                .downcast_ref::<but_error::Context>()
                .expect("a SAML enforcement 403 needs a frontend context");
            assert_eq!(
                (ctx.code, ctx.message.as_deref()),
                (
                    but_error::Code::GitHubOrgSamlRestricted,
                    Some(GITHUB_ORG_SAML_RESTRICTION_MESSAGE)
                ),
                "SAML responses need a dedicated code and static guidance"
            );
            let message = ctx.message.as_deref().expect("SAML guidance is present");
            assert!(
                !["authorization_request", "/sso?"]
                    .iter()
                    .any(|detail| message.contains(detail)),
                "the classifier must discard per-request SSO details"
            );
        }
    }

    #[test]
    fn saml_phrase_requires_reqwest_http_403() {
        let body = r#"Resource protected by organization SAML enforcement. You must authorize this credential."#;
        let code = |err: anyhow::Error| {
            classify_forge_error(err)
                .downcast_ref::<but_error::Context>()
                .map(|ctx| ctx.code)
        };
        assert_eq!(
            code(http_error(reqwest::StatusCode::UNAUTHORIZED, body)),
            Some(but_error::Code::GitHubTokenExpired),
            "401 retains its authentication classification"
        );
        assert_eq!(
            code(http_error(reqwest::StatusCode::NOT_FOUND, body)),
            None,
            "a phrase-bearing 404 stays unclassified"
        );
        // GraphQL errors returned with HTTP 200 have no HttpStatusError.
        assert_eq!(
            code(anyhow::anyhow!(body)),
            None,
            "GraphQL 200 errors stay outside the status classifier"
        );
    }

    #[test]
    fn pat_permission_403_gets_dedicated_code() {
        let err = classify_forge_error(http_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"403 Forbidden: {"message":"Resource not accessible by personal access token"}"#,
        ));
        let ctx = err.downcast_ref::<but_error::Context>();
        assert_eq!(
            ctx.map(|c| c.code),
            Some(but_error::Code::GitHubInsufficientPermissions),
            "a PAT permission 403 is terminal and needs its remediation surfaced"
        );
        assert!(
            ctx.and_then(|c| c.message.as_deref())
                .is_some_and(|message| message.starts_with("Your GitHub token")),
            "PAT refusals keep their token guidance"
        );
    }

    #[test]
    fn ip_allow_list_403_gets_actionable_terminal_classification() {
        let bodies = [
            r#"403 Forbidden: {"message":"Although you appear to have the correct authorization credentials, the `example-org` organization has an IP allow list enabled, and your IP address is not permitted to access this resource."}"#,
            // Synthetic variants: a literal address and an enterprise-level allow list.
            r#"403 Forbidden: {"message":"Although you appear to have the correct authorization credentials, the `example-org` organization has an IP allow list enabled, and 203.0.113.1 is not permitted to access this resource."}"#,
            r#"403 Forbidden: {"message":"Although you appear to have the correct authorization credentials, the `example-ent` enterprise has an IP allow list enabled, and your IP address is not permitted to access this resource."}"#,
        ];
        for body in bodies {
            let err = classify_forge_error(http_error(reqwest::StatusCode::FORBIDDEN, body));
            let ctx = err
                .downcast_ref::<but_error::Context>()
                .expect("an IP allow-list rejection needs a frontend context");
            assert_eq!(
                (ctx.code, ctx.message.as_deref()),
                (
                    but_error::Code::GitHubInsufficientPermissions,
                    Some(GITHUB_IP_ALLOW_LIST_MESSAGE)
                ),
                "IP allow-list refusals are terminal and need static guidance"
            );
            let message = ctx.message.as_deref().expect("guidance is present");
            assert!(
                !["example-org", "example-ent", "203.0.113", "CI"]
                    .iter()
                    .any(|detail| message.contains(detail)),
                "the classifier must discard the organization, address, and operation"
            );
        }
    }

    #[test]
    fn ip_allow_list_phrase_requires_403_and_yields_to_oauth() {
        let code = |status, body: &str| {
            classify_forge_error(http_error(status, body))
                .downcast_ref::<but_error::Context>()
                .map(|ctx| ctx.code)
        };
        let ip = r#"{"message":"The organization has an IP allow list enabled, and your IP address is not permitted to access this resource."}"#;
        assert_eq!(
            code(reqwest::StatusCode::UNAUTHORIZED, ip),
            Some(but_error::Code::GitHubTokenExpired),
            "401 retains its authentication classification"
        );
        for status in [
            reqwest::StatusCode::NOT_FOUND,
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            assert_eq!(
                code(status, ip),
                None,
                "a phrase-bearing {status} stays unclassified"
            );
        }
        assert_eq!(
            code(
                reqwest::StatusCode::FORBIDDEN,
                r#"{"message":"The organization has enabled OAuth App access restrictions and has an IP allow list enabled, and your IP address is not permitted to access this resource."}"#
            ),
            Some(but_error::Code::GitHubOrgOAuthRestricted),
            "OAuth restrictions keep precedence"
        );
    }

    #[test]
    fn review_list_404_classification_is_operation_local() {
        let list_err = classify_review_list_error(http_error(
            reqwest::StatusCode::NOT_FOUND,
            r#"404 Not Found: {"message":"Not Found"}"#,
        ));
        assert_eq!(
            list_err
                .downcast_ref::<but_error::Context>()
                .map(|ctx| ctx.code),
            Some(but_error::Code::GitHubInsufficientPermissions),
            "a review-list 404 needs repository access before retrying can succeed"
        );

        let other_err = classify_forge_error(http_error(
            reqwest::StatusCode::NOT_FOUND,
            r#"404 Not Found: {"message":"Not Found"}"#,
        ));
        assert!(
            other_err.downcast_ref::<but_error::Context>().is_none(),
            "other GitHub read operations keep their existing 404 semantics"
        );
    }

    #[test]
    fn rate_limit_responses_get_dedicated_code_and_static_message() {
        let cases = [
            (
                reqwest::StatusCode::FORBIDDEN,
                r#"403 Forbidden: {"message":"API rate limit exceeded for user ID 1. If you reach out to GitHub Support for help, please include the request ID ABCD:1234."}"#,
            ),
            (
                reqwest::StatusCode::FORBIDDEN,
                r#"403 Forbidden: {"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again."}"#,
            ),
            (
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                r#"429 Too Many Requests: {"message":"You have exceeded a secondary rate limit."}"#,
            ),
        ];
        for (status, body) in cases {
            let err = classify_forge_error(http_error(status, body));
            let ctx = err.downcast_ref::<but_error::Context>().unwrap_or_else(|| {
                panic!("a rate-limit response needs a frontend context: {body}")
            });
            assert_eq!(
                (ctx.code, ctx.message.as_deref()),
                (
                    but_error::Code::GitHubRateLimited,
                    Some(GITHUB_RATE_LIMIT_MESSAGE)
                ),
                "rate limits need a dedicated code and guidance without user or request ids"
            );
        }
    }

    #[test]
    fn token_lifetime_policy_403_gets_dedicated_code_and_static_message() {
        let bodies = [
            // Reported classic-PAT wording with the organization and token id replaced.
            r#"403 Forbidden: {"message":"The 'example-org' organization forbids access via a personal access tokens (classic) if the token's lifetime is greater than 180 days. Please adjust your token's lifetime at the following URL: https://github.com/settings/tokens/123456"}"#,
            // Synthetic fine-grained fixture sharing the same policy phrase; not observed in production.
            r#"403 Forbidden: {"message":"The 'example-org' organization forbids access via a fine-grained personal access token if the token's lifetime is greater than 90 days. Please adjust your token's lifetime at the following URL: https://github.com/settings/personal-access-tokens/123456"}"#,
        ];
        for body in bodies {
            let err = classify_forge_error(http_error(reqwest::StatusCode::FORBIDDEN, body));
            let ctx = err
                .downcast_ref::<but_error::Context>()
                .expect("a token-lifetime 403 needs a frontend context");
            assert_eq!(
                (ctx.code, ctx.message.as_deref()),
                (
                    but_error::Code::GitHubTokenLifetimeRestricted,
                    Some(GITHUB_TOKEN_LIFETIME_RESTRICTION_MESSAGE)
                ),
                "lifetime refusals need a dedicated code and static guidance"
            );
            let message = ctx
                .message
                .as_deref()
                .expect("lifetime guidance is present");
            assert!(
                ![
                    "example-org",
                    "settings/tokens",
                    "settings/personal-access-tokens"
                ]
                .iter()
                .any(|detail| message.contains(detail)),
                "the classifier must discard the organization name and token URL"
            );
        }
    }

    #[test]
    fn token_lifetime_phrase_requires_403_and_yields_to_rate_limits() {
        let code = |status, body: &str| {
            classify_forge_error(http_error(status, body))
                .downcast_ref::<but_error::Context>()
                .map(|ctx| ctx.code)
        };
        let lifetime = r#"{"message":"The organization forbids access if the token's lifetime is greater than 30 days."}"#;
        assert_eq!(
            code(reqwest::StatusCode::UNAUTHORIZED, lifetime),
            Some(but_error::Code::GitHubTokenExpired),
            "401 retains its authentication classification"
        );
        assert_eq!(
            code(
                reqwest::StatusCode::FORBIDDEN,
                r#"{"message":"API rate limit exceeded for user ID 1 if the token's lifetime is greater than 1 day."}"#
            ),
            Some(but_error::Code::GitHubRateLimited),
            "rate limits keep precedence over the lifetime phrase"
        );
        assert_eq!(
            code(
                reqwest::StatusCode::FORBIDDEN,
                r#"{"message":"This token's lifetime cannot be extended."}"#
            ),
            None,
            "an unrelated lifetime phrase stays unclassified"
        );
    }

    #[test]
    fn other_403s_stay_unclassified() {
        // Only production-observed wordings are classified; the rest keep
        // their raw message and stay visible in telemetry as `Unknown`.
        for body in [
            r#"403 Forbidden: {"message":"Resource not accessible by integration"}"#,
            r#"403 Forbidden: {"message":"See the SAML setup guide","documentation_url":"https://example.invalid/docs/saml-enforcement"}"#,
            r#"403 Forbidden: {"message":"Repository access blocked"}"#,
        ] {
            let err = classify_forge_error(http_error(reqwest::StatusCode::FORBIDDEN, body));
            assert!(
                err.downcast_ref::<but_error::Context>().is_none(),
                "an unrecognized 403 must not be misclassified: {body}"
            );
        }
    }
}
