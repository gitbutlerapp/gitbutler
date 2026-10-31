//! Resolve cached forge review (PR/MR) candidates for local branches.
//!
//! The join key is the branch's remote/pushed short name (e.g. `"my-feature"`),
//! which is exactly [`ForgeReview::source_branch`]. Callers derive that key from
//! the branch's remote-tracking ref (see
//! `but_core::extract_remote_name_and_short_name`) so the association follows the
//! branch that was actually pushed, which can differ from the local branch name.
//!
//! Everything here reads only from the cache and is safe inside a workspace
//! projection. Callers decide whether a cache candidate represents an active
//! review or a migration fallback for an integrated branch with no durable link.

use std::collections::HashMap;

use crate::ForgeReview;

/// Resolve the forge review associated with a branch whose remote/pushed head
/// short name is `head_ref_short`, reading only from the cache.
///
/// When several cached reviews share a `source_branch`, `preference` decides
/// the winner.
pub fn review_for_head_ref(
    db: &but_db::DbHandle,
    head_ref_short: &str,
) -> anyhow::Result<Option<ForgeReview>> {
    let reviews = crate::list_cached_forge_reviews(db)?;
    Ok(best_match(&reviews, head_ref_short).cloned())
}

/// A cached review candidate for branch association: `(number, is open,
/// merged head)`. The merged head is the merged review's source commit
/// (lowercased hex), absent for open/closed reviews and unreported heads.
pub type ReviewAssociation = (usize, bool, Option<String>);

/// Build a lookup from a remote-tracking branch's short name in `repo`, like
/// `origin/my-feature`, to the preferred cached review's `(number, is open,
/// merged head)`.
///
/// A review from a fork is listed only under remotes that fetch from or push to
/// that fork, so a branch never picks up a same-named review from someone else's
/// fork. Every other review is listed under every remote and matches by branch
/// name alone, as are fork reviews when the forge doesn't report the head
/// repository or a remote's URLs name no forge repository.
///
/// The merged head is the review's source commit (lowercased hex) when the
/// review merged, letting the projection recognize a branch whose tip landed
/// exactly while integration detection still lags a fetch.
pub fn review_associations_by_head(
    db: &but_db::DbHandle,
    repo: &gix::Repository,
) -> anyhow::Result<HashMap<String, ReviewAssociation>> {
    let reviews = crate::list_cached_forge_reviews(db)?;
    let remotes = remote_repos(repo);
    let mut fork_repos = HashMap::new();
    let mut by_head: HashMap<String, &ForgeReview> = HashMap::new();
    for review in &reviews {
        let fork_repo = fork_repo(review, &mut fork_repos);
        for (remote, remote_repos) in &remotes {
            if !remote_may_have(fork_repo.as_ref(), remote_repos) {
                continue;
            }
            let head = format!("{remote}/{}", review.source_branch);
            if is_preferred(review, by_head.get(&head).copied()) {
                by_head.insert(head, review);
            }
        }
    }
    Ok(by_head
        .into_iter()
        .filter_map(|(head, review)| {
            let number = usize::try_from(review.number).ok()?;
            let merged_head = review
                .is_merged()
                .then(|| review.sha.to_ascii_lowercase())
                .filter(|sha| !sha.is_empty());
            Some((head, (number, review.is_open(), merged_head)))
        })
        .collect())
}

/// `reviews` without those from forks that no remote of `repo` fetches from or
/// pushes to, which are someone else's whatever their branch is called. Use it
/// before matching reviews to branches by name alone.
pub fn without_unrelated_forks(
    mut reviews: Vec<ForgeReview>,
    repo: &gix::Repository,
) -> Vec<ForgeReview> {
    let remotes = remote_repos(repo);
    let mut fork_repos = HashMap::new();
    reviews.retain(|review| {
        let fork_repo = fork_repo(review, &mut fork_repos);
        fork_repo.is_none()
            || (remotes.iter())
                .any(|(_, remote_repos)| remote_may_have(fork_repo.as_ref(), remote_repos))
    });
    reviews
}

/// Each remote of `repo` by name, with the forge repositories it fetches from and
/// pushes to, as a remote can fetch from one repository and push to a fork.
fn remote_repos(repo: &gix::Repository) -> Vec<(String, Vec<crate::ForgeRepoInfo>)> {
    (repo.remote_names().iter())
        .map(|name| {
            let remote = repo.find_remote(&**name).ok();
            let remote_repos = [gix::remote::Direction::Fetch, gix::remote::Direction::Push]
                .into_iter()
                .filter_map(|direction| {
                    let url = remote.as_ref()?.url(direction)?.to_bstring().to_string();
                    crate::derive_forge_repo_info(&url, None)
                })
                .collect();
            (name.to_string(), remote_repos)
        })
        .collect()
}

/// The fork `review` comes from, if it reports one, parsed once per fork into
/// `fork_repos` as forks open many reviews each. Base-repository reviews have
/// none, so a remote whose URL renames the repository, like an old name or an
/// SSH alias, keeps them.
fn fork_repo(
    review: &ForgeReview,
    fork_repos: &mut HashMap<String, Option<crate::ForgeRepoInfo>>,
) -> Option<crate::ForgeRepoInfo> {
    let url = (review.repository_https_url.as_ref())
        .or(review.repository_ssh_url.as_ref())
        .filter(|_| review.head_repo_is_fork)?;
    (fork_repos.entry(url.clone()))
        .or_insert_with(|| crate::derive_forge_repo_info(url, None))
        .clone()
}

/// Whether a branch on a remote with `remote_repos` can have a review from
/// `fork_repo`. Without a fork, or when the remote's URLs name no forge
/// repository, it can, and the branch name alone decides.
fn remote_may_have(
    fork_repo: Option<&crate::ForgeRepoInfo>,
    remote_repos: &[crate::ForgeRepoInfo],
) -> bool {
    fork_repo.is_none_or(|fork_repo| {
        remote_repos.is_empty()
            || (remote_repos.iter()).any(|remote_repo| same_owner_and_name(fork_repo, remote_repo))
    })
}

/// Whether `a` and `b` have the same owner and name, ignoring case as the forges
/// do. Hosts aren't compared, as an SSH alias like `github.com-work` renames them.
fn same_owner_and_name(a: &crate::ForgeRepoInfo, b: &crate::ForgeRepoInfo) -> bool {
    a.owner.eq_ignore_ascii_case(&b.owner) && a.repo.eq_ignore_ascii_case(&b.repo)
}

/// Build a lookup from a branch's remote/pushed short name to its associated
/// review, so a projection can resolve many branches in one pass without a cache
/// read per branch.
///
/// When several cached reviews share a `source_branch`, `preference` decides
/// which one the key maps to.
pub fn reviews_by_head(db: &but_db::DbHandle) -> anyhow::Result<HashMap<String, ForgeReview>> {
    let reviews = crate::list_cached_forge_reviews(db)?;
    let mut map: HashMap<String, ForgeReview> = HashMap::new();
    for review in reviews {
        if is_preferred(&review, map.get(&review.source_branch)) {
            map.insert(review.source_branch.clone(), review);
        }
    }
    Ok(map)
}

/// Whether `review` should replace `existing` for the same head.
fn is_preferred(review: &ForgeReview, existing: Option<&ForgeReview>) -> bool {
    existing.is_none_or(|existing| preference(review) >= preference(existing))
}

/// Ranking used to pick between multiple cached reviews that share a
/// `source_branch` (a closed PR plus a freshly opened one, or a same-named
/// branch on a fork). Higher is preferred:
///
/// 1. an open review over a merged/closed one,
/// 2. a merged review over a closed-without-merging one — the merged review
///    is the branch's landed fate; an abandoned closed twin must not shadow
///    it,
/// 3. a review whose head is in the base repo over one in a fork — a local
///    branch normally pushes to the base repo, so this avoids latching onto a
///    fork's same-named branch when a base-repo review also exists, and
/// 4. the highest review number, which is deterministic and usually the newest
///    review among otherwise equivalent matches.
fn preference(review: &ForgeReview) -> (bool, bool, bool, i64) {
    (
        review.is_open(),
        review.is_merged(),
        !review.head_repo_is_fork,
        review.number,
    )
}

/// Pick the preferred review from an already-associated set using the same
/// deterministic ranking as cache-derived branch associations.
pub fn preferred_review(reviews: &[ForgeReview]) -> Option<&ForgeReview> {
    preferred_review_from_iter(reviews.iter())
}

fn preferred_review_from_iter<'a>(
    reviews: impl Iterator<Item = &'a ForgeReview>,
) -> Option<&'a ForgeReview> {
    reviews.max_by_key(|review| preference(review))
}

/// Pick the best cached review whose `source_branch` equals `head_ref_short`,
/// or `None` if none match.
fn best_match<'a>(reviews: &'a [ForgeReview], head_ref_short: &str) -> Option<&'a ForgeReview> {
    preferred_review_from_iter(
        reviews
            .iter()
            .filter(|review| review.source_branch == head_ref_short),
    )
}

#[cfg(test)]
mod tests {
    use super::best_match;
    use crate::ForgeReview;

    /// An open, non-fork review on `source_branch` with the given `number`.
    fn review(number: i64, source_branch: &str) -> ForgeReview {
        ForgeReview {
            html_url: String::new(),
            number,
            title: String::new(),
            body: None,
            author: None,
            labels: Vec::new(),
            draft: false,
            source_branch: source_branch.to_string(),
            target_branch: "main".to_string(),
            sha: String::new(),
            integration_commit_shas: Vec::new(),
            created_at: None,
            modified_at: None,
            merged_at: None,
            closed_at: None,
            repository_ssh_url: None,
            repository_https_url: None,
            repo_owner: None,
            head_repo_is_fork: false,
            reviewers: Vec::new(),
            auto_merge_enabled: false,
            unit_symbol: "#".to_string(),
            last_sync_at: chrono::Local::now().naive_local(),
        }
    }

    fn merged(mut review: ForgeReview) -> ForgeReview {
        review.merged_at = Some("2026-01-01T00:00:00Z".to_string());
        review
    }

    fn from_fork(mut review: ForgeReview) -> ForgeReview {
        review.head_repo_is_fork = true;
        review
    }

    fn closed(mut review: ForgeReview) -> ForgeReview {
        review.closed_at = Some("2026-01-01T00:00:00Z".to_string());
        review
    }

    #[test]
    fn matches_by_source_branch() {
        let reviews = vec![review(1, "other"), review(2, "feature")];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(2));
    }

    #[test]
    fn no_match_returns_none() {
        let reviews = vec![review(1, "other"), review(2, "feature")];
        assert!(best_match(&reviews, "missing").is_none());
    }

    #[test]
    fn empty_cache_returns_none() {
        assert!(best_match(&[], "feature").is_none());
    }

    #[test]
    fn prefers_open_over_merged() {
        let reviews = vec![merged(review(1, "feature")), review(2, "feature")];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(2));
    }

    #[test]
    fn prefers_merged_over_closed_without_merging() {
        // The higher number must not win here: the abandoned closed review
        // would otherwise shadow the branch's actual landed fate.
        let reviews = vec![closed(review(9, "feature")), merged(review(1, "feature"))];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(1));
    }

    #[test]
    fn prefers_base_repo_over_fork() {
        let reviews = vec![from_fork(review(1, "feature")), review(2, "feature")];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(2));
    }

    #[test]
    fn falls_back_to_a_fork_match_when_only_one_exists() {
        let reviews = vec![from_fork(review(1, "feature"))];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(1));
    }

    #[test]
    fn prefers_highest_number_when_other_preferences_match() {
        let reviews = vec![review(2, "feature"), review(1, "feature")];
        assert_eq!(best_match(&reviews, "feature").map(|r| r.number), Some(2));
    }
}
