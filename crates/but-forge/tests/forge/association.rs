use but_forge::{
    ForgeReview, cache_review, preferred_review, review_associations_by_head, review_for_head_ref,
    reviews_by_head, without_unrelated_forks,
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

/// `review` with its head in the fork at `url`.
fn from_fork(mut review: ForgeReview, url: &str) -> ForgeReview {
    review.repository_https_url = Some(url.to_string());
    review.head_repo_is_fork = true;
    review
}

/// A new repository at `path` with `config` added to its Git config.
fn repo_with_config(path: &std::path::Path, config: &str) -> gix::Repository {
    use std::io::Write as _;
    let repo = gix::init(path).unwrap();
    (std::fs::OpenOptions::new().append(true))
        .open(repo.git_dir().join("config"))
        .unwrap()
        .write_all(config.as_bytes())
        .unwrap();
    gix::open(path).unwrap()
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

/// A directory for a repository next to the database in `tmp`.
fn repo_dir(tmp: &tempfile::TempDir) -> std::path::PathBuf {
    tmp.path().join("repo")
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
    let (tmp, mut db) = test_db();
    let mut settled = merged(review(1, "feature"));
    // Seed a recently merged review rather than the fixed timestamp from `merged()`.
    settled.merged_at = Some(chrono::Utc::now().to_rfc3339());
    cache_review(&mut db, &settled).unwrap();
    let repo = repo_with_config(
        &repo_dir(&tmp),
        "[remote \"origin\"]\n\turl = https://github.com/acme/app.git\n",
    );

    assert_eq!(
        review_associations_by_head(&db, &repo)
            .unwrap()
            .get("origin/feature"),
        // The fixture's sha is empty, so no merged head is exposed.
        Some(&(1, false, None))
    );
}

#[test]
fn review_associations_follow_the_head_repository() {
    let (tmp, mut db) = test_db();
    let repo = repo_with_config(
        &repo_dir(&tmp),
        r#"
[remote "origin"]
	url = git@github.com:acme/app.git
[remote "alice"]
	url = git@github.com-alice:Alice/app.git
"#,
    );
    cache_review(
        &mut db,
        &from_fork(review(1, "feature"), "https://github.com/alice/app.git"),
    )
    .unwrap();
    cache_review(
        &mut db,
        &from_fork(review(2, "feature"), "https://github.com/mallory/app.git"),
    )
    .unwrap();
    let mut renamed = review(3, "renamed");
    renamed.repository_https_url = Some("https://github.com/acme/new-app.git".into());
    cache_review(&mut db, &renamed).unwrap();

    let by_head = review_associations_by_head(&db, &repo).unwrap();
    let number = |head: &str| by_head.get(head).map(|(number, ..)| *number);
    assert_eq!(
        number("alice/feature"),
        Some(1),
        "a fork's branch associates with the review opened from that fork, through an SSH alias and whatever the case"
    );
    assert_eq!(
        number("origin/feature"),
        None,
        "a same-named review from someone else's fork must not attach to the base repository's branch"
    );
    assert_eq!(
        (number("origin/renamed"), number("alice/renamed")),
        (Some(3), Some(3)),
        "a base-repository review matches by branch name alone, so a remote on the repository's old name keeps it"
    );
}

#[test]
fn review_associations_follow_a_remote_that_pushes_to_a_fork() {
    let (tmp, mut db) = test_db();
    let repo = repo_with_config(
        &repo_dir(&tmp),
        r#"
[remote "origin"]
	url = https://github.com/acme/app.git
	pushurl = git@github.com:alice/app.git
"#,
    );
    cache_review(
        &mut db,
        &from_fork(review(1, "feature"), "https://github.com/alice/app.git"),
    )
    .unwrap();

    assert_eq!(
        review_associations_by_head(&db, &repo)
            .unwrap()
            .get("origin/feature")
            .map(|(number, ..)| *number),
        Some(1),
        "a branch published to a fork through the remote's push URL keeps that fork's review"
    );
}

#[test]
fn unrelated_forks_are_dropped_before_matching_by_name() {
    let tmp = tempfile::tempdir().unwrap();
    let reviews = || {
        vec![
            from_fork(review(1, "feature"), "https://github.com/alice/app.git"),
            from_fork(review(2, "feature"), "https://github.com/mallory/app.git"),
            review(3, "feature"),
        ]
    };
    let numbers = |reviews: Vec<ForgeReview>| reviews.iter().map(|r| r.number).collect::<Vec<_>>();
    let config = r#"
[remote "origin"]
	url = git@github.com:acme/app.git
[remote "alice"]
	url = https://github.com/alice/app.git
"#;

    let repo = repo_with_config(&tmp.path().join("known"), config);
    assert_eq!(
        numbers(without_unrelated_forks(reviews(), &repo)),
        vec![1, 3],
        "a fork no remote points at is someone else's, while yours and the base repository's stay"
    );

    let with_local = format!("{config}[remote \"gb-local\"]\n\turl = /some/path\n");
    let repo = repo_with_config(&tmp.path().join("local"), &with_local);
    assert_eq!(
        numbers(without_unrelated_forks(reviews(), &repo)),
        vec![1, 2, 3],
        "a remote whose URL names no forge repository might be the fork, so nothing is dropped"
    );
}
