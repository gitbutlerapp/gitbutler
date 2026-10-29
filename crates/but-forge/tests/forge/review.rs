use anyhow::Result;
use but_forge::{
    CacheConfig, ForgeName, ForgeRepoInfo, ForgeReview, ForgeReviewTargetUpdate, ForgeReviewThread,
    ForgeReviewThreadComment, ForgeReviewThreadSide, ForgeReviewUpdate, GitHubStackingMode,
    ReviewStackingDescription, ReviewState, ReviewUpdatePayload, compute_review_target_updates,
    list_cached_forge_reviews, list_forge_reviews_with_cache, prepare_review_target_updates,
    restore_native_stacks, sync_reviews, update_review,
};

fn cached_review(number: i64, merged_at: Option<&str>, closed_at: Option<&str>) -> ForgeReview {
    ForgeReview {
        html_url: format!("https://github.com/o/r/pull/{number}"),
        number,
        title: format!("PR {number}"),
        body: None,
        author: None,
        labels: vec![],
        draft: false,
        source_branch: format!("branch-{number}"),
        target_branch: "main".to_string(),
        sha: "0000000000000000000000000000000000000000".to_string(),
        integration_commit_shas: vec![],
        created_at: None,
        modified_at: None,
        merged_at: merged_at.map(Into::into),
        closed_at: closed_at.map(Into::into),
        repository_ssh_url: None,
        repository_https_url: None,
        repo_owner: None,
        head_repo_is_fork: false,
        reviewers: vec![],
        auto_merge_enabled: false,
        unit_symbol: "#".to_string(),
        last_sync_at: chrono::NaiveDateTime::default(),
    }
}

#[test]
fn missing_credentials_do_not_turn_a_stale_cache_into_an_empty_listing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut db = but_db::DbHandle::new_in_directory(tmp.path()).unwrap();
    // No accounts are stored, so client construction fails before any
    // network request — the listing must not pretend the forge is empty.
    let storage = but_forge_storage::Controller::from_path(tmp.path());
    // `cached_review` stamps `last_sync_at` at the epoch, well past the
    // fallback window, so the cached read attempts a live refresh.
    db.forge_reviews_mut()
        .unwrap()
        .set_all(vec![cached_review(1, None, None).try_into().unwrap()])
        .unwrap();

    let reviews = list_forge_reviews_with_cache(
        None,
        &repo_info("owner", "repo"),
        &storage,
        &mut db,
        Some(CacheConfig::CacheWithFallback {
            max_age_seconds: 300,
        }),
    )
    .unwrap();

    let listed: Vec<i64> = reviews.iter().map(|review| review.number).collect();
    assert_eq!(
        listed,
        vec![1],
        "a failed credential lookup must serve the last known open reviews"
    );
    let persisted: Vec<i64> = list_cached_forge_reviews(&db)
        .unwrap()
        .iter()
        .map(|review| review.number)
        .collect();
    assert_eq!(
        persisted,
        vec![1],
        "a failed credential lookup must not delete the last known open reviews"
    );
}

#[test]
fn github_review_threads_map_side_and_outdated_anchor() {
    let thread = |diff_side: &str, line: Option<i64>| but_github::PullRequestReviewThread {
        id: "PRRT_1".to_string(),
        is_resolved: true,
        is_outdated: line.is_none(),
        path: "src/lib.rs".to_string(),
        line,
        start_line: line,
        original_line: Some(42),
        diff_side: diff_side.to_string(),
        comments: Vec::new(),
    };

    let current = ForgeReviewThread::from(thread("RIGHT", Some(17)));
    assert_eq!(current.side, ForgeReviewThreadSide::New);
    assert_eq!(current.line, Some(17));
    assert!(
        current.is_resolved,
        "resolution carries over from the forge"
    );

    let outdated = ForgeReviewThread::from(thread("LEFT", None));
    assert_eq!(outdated.side, ForgeReviewThreadSide::Old);
    assert_eq!(
        (outdated.line, outdated.original_line),
        (None, Some(42)),
        "an outdated thread keeps only where it was originally left"
    );
}

#[test]
fn github_thread_comments_keep_their_review() {
    let comment = but_github::PullRequestReviewThreadComment {
        id: 1,
        body: "@you take a look".to_string(),
        author: None,
        created_at: Some("2026-08-29T09:36:02Z".to_string()),
        modified_at: None,
        html_url: "https://example.invalid/c/1".to_string(),
        diff_hunk: Some("@@ -1 +1 @@".to_string()),
        review_id: Some(99),
    };

    let mapped = ForgeReviewThreadComment::from(comment);
    assert_eq!(
        mapped.review_id,
        Some(99),
        "the submission id is what files a thread under its verdict"
    );
    assert_eq!(mapped.body, "@you take a look");
}

fn repo_info(owner: &str, repo: &str) -> ForgeRepoInfo {
    ForgeRepoInfo {
        forge: ForgeName::GitHub,
        host: "github.com".to_string(),
        port: None,
        owner: owner.to_string(),
        repo: repo.to_string(),
        protocol: "https".to_string(),
    }
}

fn unusable_storage(tmp: &tempfile::TempDir) -> but_forge_storage::Controller {
    let path = tmp.path().join("unusable-account-storage");
    std::fs::write(&path, "not a directory").unwrap();
    but_forge_storage::Controller::from_path(path)
}

#[test]
fn cached_reviews_and_empty_sync_do_not_require_accounts() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let storage = unusable_storage(&tmp);
    let mut db = but_db::DbHandle::new_in_directory(tmp.path())?;
    let mut repo = repo_info("owner", "repo");
    repo.host = "forge.example.com".into();
    let mut review = cached_review(1, None, None);
    review.last_sync_at = chrono::Local::now().naive_local();
    db.forge_reviews_mut()?.set_all(vec![review.try_into()?])?;
    for cache in [
        CacheConfig::CacheOnly,
        CacheConfig::CacheWithFallback {
            max_age_seconds: 300,
        },
    ] {
        let reviews = list_forge_reviews_with_cache(None, &repo, &storage, &mut db, Some(cache))?;
        assert_eq!(
            (reviews.len(), reviews[0].number),
            (1, 1),
            "cache-only and fresh-cache hits must not read account storage"
        );
    }
    tokio::runtime::Runtime::new()?.block_on(async {
        for forge in [ForgeName::GitHub, ForgeName::GitLab, ForgeName::Bitbucket] {
            repo.forge = forge;
            sync_reviews(
                &None,
                &repo,
                &None,
                &[],
                &storage,
                ReviewStackingDescription::Bottom,
                GitHubStackingMode::Auto,
            )
            .await?;
        }
        repo.forge = ForgeName::Bitbucket;
        update_review(
            &None,
            &repo,
            usize::MAX,
            ReviewUpdatePayload::new(None, None, None, None),
            &storage,
        )
        .await?;
        for payload in [
            ReviewUpdatePayload::new(None, None, Some(ReviewState::Open), None),
            ReviewUpdatePayload::new(
                Some("Updated title".into()),
                Some("Updated body".into()),
                Some(ReviewState::Open),
                Some("main".into()),
            ),
        ] {
            let error = update_review(&None, &repo, usize::MAX, payload, &storage)
                .await
                .unwrap_err();
            assert_eq!(
                error.to_string(),
                "Bitbucket does not support reopening a declined pull request via the API.",
                "reopening must fail before account storage or metadata updates are attempted"
            );
        }
        repo.forge = ForgeName::GitHub;
        restore_native_stacks(&None, &repo, &[], &storage).await?;
        let reviews = [ForgeReviewUpdate::from(cached_review(1, None, None))];
        for (reviews, push, mode) in [
            (&[][..], None, GitHubStackingMode::Auto),
            (&reviews[..], None, GitHubStackingMode::Disabled),
            (
                &reviews[..],
                Some(repo_info("fork-owner", "repo")),
                GitHubStackingMode::Auto,
            ),
        ] {
            let stacks =
                prepare_review_target_updates(&None, &repo, &push, reviews, &storage, mode).await?;
            assert!(
                stacks.is_empty(),
                "empty, disabled and fork stacking must not read account storage"
            );
        }
        Ok(())
    })
}

#[test]
fn gitlab_review_preserves_source_project_clone_urls() {
    let review = ForgeReview::from(but_gitlab::MergeRequest {
        web_url: "https://gitlab.example/acme/widgets/-/merge_requests/42".into(),
        iid: 42,
        title: "Fork MR".into(),
        description: None,
        author: None,
        labels: vec![],
        draft: false,
        source_branch: "fork-feature".into(),
        target_branch: "main".into(),
        sha: "1234567890abcdef1234567890abcdef12345678".into(),
        integration_commit_shas: vec![],
        created_at: None,
        updated_at: None,
        merged_at: None,
        closed_at: None,
        project_id: 10,
        source_project_id: Some(20),
        target_project_id: Some(10),
        repository_ssh_url: Some("git@gitlab.example:contributor/widgets.git".into()),
        repository_https_url: Some("https://gitlab.example/contributor/widgets.git".into()),
        repo_owner: Some("contributor".into()),
        source_project_is_fork: true,
        assignees: vec![],
        reviewers: vec![],
        auto_merge_enabled: false,
    });

    assert_eq!(
        review.repository_https_url.as_deref(),
        Some("https://gitlab.example/contributor/widgets.git"),
        "GitLab review apply needs the source project HTTPS URL"
    );
    assert_eq!(
        review.repository_ssh_url.as_deref(),
        Some("git@gitlab.example:contributor/widgets.git"),
        "GitLab review apply needs the source project SSH URL"
    );
    assert_eq!(
        review.repo_owner.as_deref(),
        Some("contributor"),
        "GitLab fork remotes should be named from the source project namespace"
    );
    assert!(
        review.head_repo_is_fork,
        "GitLab fork MRs should preserve that the source project differs from the target"
    );
}

// --- compute_review_target_updates tests ---

fn heads(specs: &[(&str, Option<i64>)]) -> Vec<(String, Option<i64>)> {
    specs
        .iter()
        .map(|(name, id)| (name.to_string(), *id))
        .collect()
}

#[test]
fn target_updates_empty_stack() {
    let result = compute_review_target_updates(&[], "main");
    assert!(result.is_empty());
}

#[test]
fn target_updates_no_reviews() {
    let h = heads(&[("branch-a", None), ("branch-b", None)]);
    let result = compute_review_target_updates(&h, "main");
    assert!(result.is_empty());
}

#[test]
fn target_updates_single_branch_with_review() {
    let h = heads(&[("branch-a", Some(1))]);
    let result = compute_review_target_updates(&h, "main");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 1);
    assert_eq!(result[0].target_branch, "main");
}

#[test]
fn target_updates_stacked_reviews_point_to_parent() {
    // bottom-to-top: branch-a -> branch-b -> branch-c
    let h = heads(&[
        ("branch-a", Some(1)),
        ("branch-b", Some(2)),
        ("branch-c", Some(3)),
    ]);
    let result = compute_review_target_updates(&h, "main");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].number, 1);
    assert_eq!(result[0].target_branch, "main");
    assert_eq!(result[1].number, 2);
    assert_eq!(result[1].target_branch, "branch-a");
    assert_eq!(result[2].number, 3);
    assert_eq!(result[2].target_branch, "branch-b");
}

#[test]
fn target_updates_request_remote_description_hydration() {
    let update: ForgeReviewUpdate = ForgeReviewTargetUpdate {
        number: 42,
        target_branch: "parent".to_string(),
    }
    .into();

    assert!(
        !update.update_description,
        "target-only updates ask sync_reviews to hydrate the remote description"
    );
    assert_eq!(update.body, None);
    assert_eq!(update.target_branch.as_deref(), Some("parent"));
}

#[test]
fn review_update_defaults_to_description_sync_when_field_is_omitted() {
    let omitted: ForgeReviewUpdate = serde_json::from_value(serde_json::json!({
        "number": 42,
        "body": "Caller-provided description",
        "unitSymbol": "#",
        "targetBranch": null
    }))
    .expect("an update from an older caller should deserialize");
    let explicit_false: ForgeReviewUpdate = serde_json::from_value(serde_json::json!({
        "number": 42,
        "body": null,
        "updateDescription": false,
        "unitSymbol": "",
        "targetBranch": "parent"
    }))
    .expect("a target-only update should deserialize");

    assert!(
        omitted.update_description,
        "older callers that omit the field should synchronize their provided description"
    );
    assert!(
        !explicit_false.update_description,
        "target-only callers should still be able to request remote hydration"
    );
}

#[test]
fn target_updates_skips_branches_without_reviews() {
    // branch-a has no review, so branch-b must target the trunk.
    let h = heads(&[("branch-a", None), ("branch-b", Some(5))]);
    let result = compute_review_target_updates(&h, "main");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 5);
    assert_eq!(result[0].target_branch, "main");
}

#[test]
fn target_updates_gap_in_middle() {
    // a(PR) -> b(no PR) -> c(PR): c should skip b and target a.
    let h = heads(&[
        ("branch-a", Some(1)),
        ("branch-b", None),
        ("branch-c", Some(3)),
    ]);
    let result = compute_review_target_updates(&h, "main");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].number, 1);
    assert_eq!(result[0].target_branch, "main");
    assert_eq!(result[1].number, 3);
    assert_eq!(result[1].target_branch, "branch-a");
}

#[test]
fn forge_review_from_bitbucket_pull_request_maps_fields() {
    let pr = but_bitbucket::BitbucketPullRequest {
        html_url: "https://bitbucket.org/ws/repo/pull-requests/7".into(),
        id: 7,
        title: "Add feature".into(),
        description: Some("body".into()),
        state: "MERGED".into(),
        draft: false,
        source_branch: "feature".into(),
        target_branch: "main".into(),
        source_commit_hash: "deadbeef".into(),
        merge_commit_hash: Some("cafef00d".into()),
        created_on: Some("2026-06-01T00:00:00Z".into()),
        updated_on: Some("2026-06-02T00:00:00Z".into()),
        comment_count: 0,
        author: Some(but_bitbucket::BitbucketUser {
            id: 0,
            username: "alice".into(),
            name: Some("Alice".into()),
            email: None,
            avatar_url: Some("https://avatar".into()),
            is_bot: false,
        }),
        reviewers: vec![but_bitbucket::BitbucketUser {
            id: 0,
            username: "bob".into(),
            name: None,
            email: None,
            avatar_url: None,
            is_bot: false,
        }],
        head_repo_is_fork: false,
        repo_owner: None,
    };

    let review = ForgeReview::from(pr);

    assert_eq!(review.number, 7);
    assert_eq!(
        review.html_url,
        "https://bitbucket.org/ws/repo/pull-requests/7"
    );
    assert_eq!(review.title, "Add feature");
    assert_eq!(review.body.as_deref(), Some("body"));
    assert_eq!(review.source_branch, "feature");
    assert_eq!(review.target_branch, "main");
    assert_eq!(review.sha, "deadbeef");
    assert_eq!(review.integration_commit_shas, vec!["cafef00d".to_string()]);
    assert_eq!(review.merged_at.as_deref(), Some("2026-06-02T00:00:00Z"));
    assert_eq!(review.closed_at, None);
    assert!(review.labels.is_empty());
    assert!(!review.draft);
    assert!(!review.head_repo_is_fork);
    assert_eq!(review.unit_symbol, "#");
    assert_eq!(
        review.author.as_ref().map(|a| a.login.as_str()),
        Some("alice")
    );
    assert_eq!(review.reviewers.len(), 1);
    assert_eq!(review.reviewers[0].login, "bob");
}
