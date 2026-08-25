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

/// Build a lookup from a branch's remote/pushed short name to the preferred
/// cached review's `(number, is open, merged head)`.
///
/// The merged head is the review's source commit (lowercased hex) when the
/// review merged, letting the projection recognize a branch whose tip landed
/// exactly while integration detection still lags a fetch.
pub fn review_associations_by_head(
    db: &but_db::DbHandle,
) -> anyhow::Result<HashMap<String, ReviewAssociation>> {
    Ok(reviews_by_head(db)?
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
        let replace = map
            .get(&review.source_branch)
            .is_none_or(|existing| preference(&review) >= preference(existing));
        if replace {
            map.insert(review.source_branch.clone(), review);
        }
    }
    Ok(map)
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
    use super::{
        best_match, preferred_review, review_associations_by_head, review_for_head_ref,
        reviews_by_head,
    };
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

    #[test]
    fn preferred_review_uses_cache_association_ranking() {
        let reviews = vec![merged(review(9, "feature")), review(2, "feature")];
        assert_eq!(
            preferred_review(&reviews).map(|review| review.number),
            Some(2)
        );
    }

    fn test_db() -> (tempfile::TempDir, but_db::DbHandle) {
        let tmp = tempfile::tempdir().unwrap();
        let db = but_db::DbHandle::new_in_directory(tmp.path()).unwrap();
        (tmp, db)
    }

    #[test]
    fn resolves_from_the_cache() {
        let (_tmp, mut db) = test_db();
        crate::db::cache_reviews(&mut db, &[review(7, "feature")]).unwrap();

        let resolved = review_for_head_ref(&db, "feature").unwrap();
        assert_eq!(resolved.map(|r| r.number), Some(7));

        assert!(
            review_for_head_ref(&db, "missing").unwrap().is_none(),
            "an unpublished branch resolves to no review"
        );
    }

    #[test]
    fn empty_cache_resolves_to_none() {
        let (_tmp, db) = test_db();
        assert!(review_for_head_ref(&db, "feature").unwrap().is_none());
    }

    #[test]
    fn reviews_by_head_dedups_preferring_open() {
        let (_tmp, mut db) = test_db();
        crate::db::cache_reviews(
            &mut db,
            &[merged(review(1, "feature")), review(2, "feature")],
        )
        .unwrap();

        let map = reviews_by_head(&db).unwrap();
        assert_eq!(map.get("feature").map(|r| r.number), Some(2));
    }

    #[test]
    fn reviews_by_head_dedups_preferring_highest_number() {
        let (_tmp, mut db) = test_db();
        crate::db::cache_reviews(&mut db, &[review(2, "feature"), review(1, "feature")]).unwrap();

        let map = reviews_by_head(&db).unwrap();
        assert_eq!(map.get("feature").map(|r| r.number), Some(2));
    }

    #[test]
    fn review_associations_include_settled_state() {
        let (_tmp, mut db) = test_db();
        let mut settled = merged(review(1, "feature"));
        // Not redundant with `merged()`: its fixed 2026 timestamp is past the
        // merged-retention window, so `cache_reviews` would prune the row.
        settled.merged_at = Some(chrono::Utc::now().to_rfc3339());
        crate::db::cache_reviews(&mut db, &[settled]).unwrap();

        assert_eq!(
            review_associations_by_head(&db).unwrap().get("feature"),
            // The fixture's sha is empty, so no merged head is exposed.
            Some(&(1, false, None))
        );
    }
}
