use anyhow::{Context as _, Result};

use crate::client::{CheckRun, GitHubClient, HttpStatusError};
use crate::pr::classify_forge_error;

/// Fetch a branch ref's CI checks: its check runs, then its commit statuses.
///
/// Returns `None` when GitHub can't resolve the ref (a 422 — a branch that was
/// deleted, renamed, or hasn't propagated yet after a push). That is an expected
/// "no checks" state for display, but because it can be transient the caller
/// must not let it overwrite a cached result. A resolvable ref returns
/// `Some(runs)`, where an empty vec is an authoritative "this ref has no checks"
/// that should replace the cache. A token that may not read commit statuses gets
/// the check runs alone. Every other failure is classified (transport →
/// `NetworkError`, 401 → `GitHubTokenExpired`) so the desktop can present it.
pub async fn list_for_ref(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    reference: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Vec<CheckRun>>> {
    let gh = GitHubClient::from_storage(storage, preferred_account)?;
    let mut runs = match gh.list_checks_for_ref(owner, repo, reference).await {
        Ok(runs) => runs,
        Err(err) if is_unresolvable_ref(&err) => return Ok(None),
        Err(err) => {
            return Err(classify_forge_error(err)).context("Failed to list checks for ref");
        }
    };
    match gh
        .list_commit_statuses_for_ref(owner, repo, reference)
        .await
    {
        Ok(statuses) => runs.extend(statuses),
        Err(err) if is_unresolvable_ref(&err) => return Ok(None),
        Err(err) if is_commit_status_access_refused(&err) => {
            tracing::warn!(
                "{owner}/{repo}: this token can't read commit statuses; showing check runs only"
            );
        }
        Err(err) => {
            return Err(classify_forge_error(err))
                .context("Failed to list commit statuses for ref");
        }
    }
    Ok(Some(runs))
}

/// A 422 on `commits/{ref}/check-runs` or `commits/{ref}/status` — GitHub
/// couldn't resolve the ref.
fn is_unresolvable_ref(err: &anyhow::Error) -> bool {
    err.downcast_ref::<HttpStatusError>()
        .is_some_and(|http_err| http_err.status == reqwest::StatusCode::UNPROCESSABLE_ENTITY)
}

/// A 403 "Resource not accessible" on `commits/{ref}/status` — a fine-grained
/// token can be granted check runs without commit statuses. Rate limits answer
/// 403 too, and must not pass for this.
fn is_commit_status_access_refused(err: &anyhow::Error) -> bool {
    err.downcast_ref::<HttpStatusError>()
        .is_some_and(|http_err| http_err.status == reqwest::StatusCode::FORBIDDEN)
        && err
            .chain()
            .any(|cause| cause.to_string().contains("Resource not accessible"))
}

#[cfg(test)]
mod tests {
    use super::{is_commit_status_access_refused, is_unresolvable_ref};
    use crate::client::HttpStatusError;

    fn http_error(status: reqwest::StatusCode) -> anyhow::Error {
        HttpStatusError { status }.into()
    }

    #[test]
    fn unprocessable_entity_is_an_unresolvable_ref() {
        assert!(is_unresolvable_ref(&http_error(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY
        )));
    }

    #[test]
    fn other_statuses_are_not_unresolvable_refs() {
        for status in [
            reqwest::StatusCode::FORBIDDEN,
            reqwest::StatusCode::NOT_FOUND,
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            reqwest::StatusCode::UNAUTHORIZED,
        ] {
            assert!(!is_unresolvable_ref(&http_error(status)));
        }
    }

    #[test]
    fn a_resource_not_accessible_refusal_is_refused_commit_status_access() {
        let err = http_error(reqwest::StatusCode::FORBIDDEN).context(
            r#"403 Forbidden: {"message":"Resource not accessible by personal access token"}"#,
        );
        assert!(
            is_commit_status_access_refused(&err),
            "GitHub refuses a fine-grained token without the commit statuses permission like this"
        );
    }

    #[test]
    fn a_rate_limit_is_not_refused_commit_status_access() {
        let err = http_error(reqwest::StatusCode::FORBIDDEN)
            .context(r#"403 Forbidden: {"message":"API rate limit exceeded for user ID 1."}"#);
        assert!(
            !is_commit_status_access_refused(&err),
            "a rate limit must surface rather than hide the commit statuses"
        );
    }
}
