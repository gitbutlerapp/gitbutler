use but_forge::{
    ForgeReview, cache_review, preferred_review, review_associations_by_head, review_for_head_ref,
    reviews_by_head,
};

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
fn empty_cache_resolves_to_none() {
    let (_tmp, db) = test_db();
    assert!(review_for_head_ref(&db, "feature").unwrap().is_none());
}

#[test]
fn resolves_from_the_cache() {
    let (_tmp, mut db) = test_db();
    cache_review(&mut db, &review(7, "feature")).unwrap();

    let resolved = review_for_head_ref(&db, "feature").unwrap();
    assert_eq!(resolved.map(|r| r.number), Some(7));

    assert!(
        review_for_head_ref(&db, "missing").unwrap().is_none(),
        "an unpublished branch resolves to no review"
    );
}

#[test]
fn reviews_by_head_dedups_preferring_open() {
    let (_tmp, mut db) = test_db();
    cache_review(&mut db, &merged(review(1, "feature"))).unwrap();
    cache_review(&mut db, &review(2, "feature")).unwrap();

    let map = reviews_by_head(&db).unwrap();
    assert_eq!(map.get("feature").map(|r| r.number), Some(2));
}

#[test]
fn reviews_by_head_dedups_preferring_highest_number() {
    let (_tmp, mut db) = test_db();
    cache_review(&mut db, &review(2, "feature")).unwrap();
    cache_review(&mut db, &review(1, "feature")).unwrap();

    let map = reviews_by_head(&db).unwrap();
    assert_eq!(map.get("feature").map(|r| r.number), Some(2));
}

#[test]
fn review_associations_include_settled_state() {
    let (_tmp, mut db) = test_db();
    let mut settled = merged(review(1, "feature"));
    // Seed a recently merged review rather than the fixed timestamp from `merged()`.
    settled.merged_at = Some(chrono::Utc::now().to_rfc3339());
    cache_review(&mut db, &settled).unwrap();

    assert_eq!(
        review_associations_by_head(&db).unwrap().get("feature"),
        // The fixture's sha is empty, so no merged head is exposed.
        Some(&(1, false, None))
    );
}
