use anyhow::{Context as _, Result};

use crate::client::{CheckRun, GitHubClient, HttpStatusError};
use crate::pr::classify_forge_error;

/// Fetch CI check runs for a branch ref.
///
/// Returns `None` when GitHub can't resolve the ref (a 422 — a branch that was
/// deleted, renamed, or hasn't propagated yet after a push). That is an expected
/// "no checks" state for display, but because it can be transient the caller
/// must not let it overwrite a cached result. A resolvable ref returns
/// `Some(runs)`, where an empty vec is an authoritative "this ref has no checks"
/// that should replace the cache. Every other failure is classified (transport →
/// `NetworkError`, 401 → `GitHubTokenExpired`) so the desktop can present it.
pub async fn list_for_ref(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    reference: &str,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Vec<CheckRun>>> {
    let gh = GitHubClient::from_storage(storage, preferred_account)?;
    match gh.list_checks_for_ref(owner, repo, reference).await {
        Ok(runs) => Ok(Some(runs)),
        Err(err) if is_unresolvable_ref(&err) => Ok(None),
        Err(err) => Err(classify_forge_error(err)).context("Failed to list checks for ref"),
    }
}

/// A 422 on `commits/{ref}/check-runs` — GitHub couldn't resolve the ref.
fn is_unresolvable_ref(err: &anyhow::Error) -> bool {
    err.downcast_ref::<HttpStatusError>()
        .is_some_and(|http_err| http_err.status == reqwest::StatusCode::UNPROCESSABLE_ENTITY)
}

#[cfg(test)]
mod tests {
    use super::is_unresolvable_ref;
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
}
