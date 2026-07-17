//! GitHub's native stacked-pull-requests API.
//!
//! This is a preview feature enabled per repository; every endpoint returns
//! 404 on repositories that don't have it, which doubles as the detection
//! mechanism. GitHub models a stack exactly like GitButler does — each PR
//! targets the branch of the PR below it — so PRs published by GitButler can
//! be registered as-is.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use crate::client::GitHubClient;

/// A stack of pull requests, ordered bottom (closest to the base branch) to top.
#[derive(Debug, Clone, Deserialize)]
pub struct Stack {
    /// The repository-scoped number used to address the stack.
    pub number: i64,
    /// Member pull requests, bottom to top. Merged PRs remain members.
    pub pull_requests: Vec<StackPullRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StackPullRequest {
    pub number: i64,
}

/// Outcome of asking GitHub for the stack containing a given PR.
#[derive(Debug, Clone)]
pub enum StackLookup {
    /// The repository does not have native stacks enabled.
    Unsupported,
    /// Native stacks are enabled; the PR may or may not be in a stack yet.
    Supported(Option<Stack>),
}

/// Repositories known to not support native stacks, so repeated syncs don't
/// re-probe. Only negative results are cached: supported repositories need a
/// fresh lookup for the stack state anyway, and a repository gaining the
/// feature mid-process is picked up on the next probe.
static UNSUPPORTED_CACHE: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Find the stack containing `pr_number`, detecting on the way whether the
/// repository supports native stacks at all.
pub async fn lookup(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pr_number: i64,
    storage: &but_forge_storage::Controller,
) -> Result<StackLookup> {
    let gh = GitHubClient::from_storage(storage, preferred_account)?;
    // Keyed by host too: the same owner/repo can exist on github.com and an
    // Enterprise host with different enablement.
    let cache_key = format!("{}/{owner}/{repo}", gh.base_url);
    if UNSUPPORTED_CACHE.lock().unwrap().contains(&cache_key) {
        return Ok(StackLookup::Unsupported);
    }
    match gh
        .stacks_for_pull_request(owner, repo, pr_number)
        .await
        .context("Failed to look up GitHub stacks")?
    {
        None => {
            UNSUPPORTED_CACHE.lock().unwrap().insert(cache_key);
            Ok(StackLookup::Unsupported)
        }
        Some(stacks) => Ok(StackLookup::Supported(stacks.into_iter().next())),
    }
}

/// Make sure `pull_requests` (ordered bottom to top) are registered as a
/// stack: create one, or append the members `existing` doesn't have yet.
/// Each PR must target the head branch of the one before it (the bottom-most
/// targets the base branch) — GitHub validates the chain.
pub async fn ensure(
    preferred_account: Option<&crate::GithubAccountIdentifier>,
    owner: &str,
    repo: &str,
    pull_requests: &[i64],
    existing: Option<&Stack>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let gh = GitHubClient::from_storage(storage, preferred_account)?;
    match existing {
        Some(stack) => {
            let missing = members_to_add(stack, pull_requests)?;
            if missing.is_empty() {
                Ok(())
            } else {
                gh.add_to_stack(owner, repo, stack.number, &missing)
                    .await
                    .context("Failed to add pull requests to GitHub stack")
            }
        }
        None => gh
            .create_stack(owner, repo, pull_requests)
            .await
            .context("Failed to create GitHub stack"),
    }
}

/// The desired PRs that aren't members of the stack yet, in the given order.
///
/// The add endpoint can only append above the current top, so this errors when
/// the missing PRs aren't exactly the tail of `desired` (e.g. a PR was
/// inserted mid-stack) instead of sending a request GitHub would reject.
fn members_to_add(stack: &Stack, desired: &[i64]) -> Result<Vec<i64>> {
    let members: HashSet<i64> = stack.pull_requests.iter().map(|pr| pr.number).collect();
    let missing: Vec<i64> = desired
        .iter()
        .copied()
        .filter(|number| !members.contains(number))
        .collect();
    if desired[desired.len() - missing.len()..] != missing[..] {
        bail!("the stack's members changed below its top; only appending is supported");
    }
    Ok(missing)
}

#[derive(serde::Serialize)]
struct StackMembersBody<'a> {
    pull_requests: &'a [i64],
}

impl GitHubClient {
    /// List the stacks containing a PR. `Ok(None)` means the repository does
    /// not have native stacks enabled (the endpoint 404s).
    async fn stacks_for_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: i64,
    ) -> Result<Option<Vec<Stack>>> {
        let url = format!("{}/repos/{}/{}/stacks", self.base_url, owner, repo);
        let response = self
            .client
            .get(&url)
            .query(&[("pull_request", pr_number)])
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            bail!("Failed to list stacks: {}", response.status());
        }
        Ok(Some(response.json().await?))
    }

    async fn create_stack(&self, owner: &str, repo: &str, pull_requests: &[i64]) -> Result<()> {
        let url = format!("{}/repos/{}/{}/stacks", self.base_url, owner, repo);
        let response = self
            .client
            .post(&url)
            .json(&StackMembersBody { pull_requests })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to create stack: {}", response.status());
        }
        Ok(())
    }

    async fn add_to_stack(
        &self,
        owner: &str,
        repo: &str,
        stack_number: i64,
        pull_requests: &[i64],
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/stacks/{}/add",
            self.base_url, owner, repo, stack_number
        );
        let response = self
            .client
            .post(&url)
            .json(&StackMembersBody { pull_requests })
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("Failed to add to stack: {}", response.status());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_to_add_skips_existing_members_and_keeps_order() {
        let stack = Stack {
            number: 7,
            pull_requests: vec![
                StackPullRequest { number: 1 },
                StackPullRequest { number: 2 },
            ],
        };
        assert_eq!(members_to_add(&stack, &[1, 2, 3, 4]).unwrap(), vec![3, 4]);
        assert!(members_to_add(&stack, &[1, 2]).unwrap().is_empty());
    }

    #[test]
    fn members_to_add_rejects_mid_stack_insertions() {
        let stack = Stack {
            number: 7,
            pull_requests: vec![
                StackPullRequest { number: 1 },
                StackPullRequest { number: 3 },
            ],
        };
        assert!(
            members_to_add(&stack, &[1, 2, 3]).is_err(),
            "PR 2 sits below the stack top and can't be appended"
        );
    }
}
