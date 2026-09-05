use anyhow::{Context as _, Result};

use crate::client::{GitHubClient, HttpStatusError};

const GITHUB_ORG_SAML_RESTRICTION_MESSAGE: &str = "This GitHub organization requires SAML SSO. Authorize the GitButler OAuth app on the organization's SSO page, or authorize your personal access token in GitHub's token SSO settings, then try again.";
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
        if http_err.status == reqwest::StatusCode::FORBIDDEN {
            // `ensure_success` keeps GitHub's response body in the chain.
            let contains =
                |needle: &str| err.chain().any(|cause| cause.to_string().contains(needle));
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn other_403s_stay_unclassified() {
        // Only production-observed wordings are classified; the rest keep
        // their raw message and stay visible in telemetry as `Unknown`.
        for body in [
            r#"403 Forbidden: {"message":"Resource not accessible by integration"}"#,
            r#"403 Forbidden: {"message":"API rate limit exceeded for user ID 1."}"#,
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
