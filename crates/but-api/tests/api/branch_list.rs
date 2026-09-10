//! Tests for [`but_api::branch::branch_list()`].

use but_branches::ListedStackStatus;
use but_forge::ForgeReview;
use gix::bstr::ByteSlice;

use crate::support::{repo_with_feature_branch, set_project_target_to_feature};

fn cached_review(source_branch: &str, number: i64) -> ForgeReview {
    ForgeReview {
        html_url: format!("https://example.com/pull/{number}"),
        number,
        title: format!("Review of {source_branch}"),
        body: None,
        author: None,
        labels: Vec::new(),
        draft: false,
        source_branch: source_branch.to_string(),
        target_branch: "main".to_string(),
        sha: "0000000000000000000000000000000000000000".to_string(),
        integration_commit_shas: Vec::new(),
        created_at: None,
        modified_at: None,
        merged_at: None,
        closed_at: None,
        repository_ssh_url: None,
        repository_https_url: None,
        repo_owner: None,
        head_repo_is_fork: false,
        auto_merge_enabled: false,
        reviewers: Vec::new(),
        unit_symbol: "#".to_string(),
        last_sync_at: chrono::NaiveDateTime::default(),
    }
}

#[test]
fn listing_serializes_only_the_review_metadata_it_needs() -> anyhow::Result<()> {
    let mut review = cached_review("feature", 42);
    review.body = Some("A long review description".into());
    review.labels.push(but_forge::ForgeReviewLabel {
        name: "rust".into(),
        description: Some("Rust changes".into()),
        color: Some("dea584".into()),
    });
    review.author = Some(but_forge::ForgeReviewUser {
        id: 7,
        login: "octocat".into(),
        name: Some("The Octocat".into()),
        email: Some("octocat@example.com".into()),
        avatar_url: None,
        is_bot: false,
    });
    review.created_at = Some("2026-09-01T12:00:00Z".into());

    let json = serde_json::to_value(but_api::branch::json::ListedForgeReview::from(&review))?;
    assert_eq!(
        json["labels"][0]["name"], "rust",
        "labels survive transport"
    );
    assert_eq!(
        json["labels"][0]["color"], "dea584",
        "label colors survive transport"
    );
    assert_eq!(
        json["author"],
        serde_json::json!({ "login": "octocat", "name": "The Octocat" }),
        "listing authors include only the login and display name used by the UI and search"
    );
    assert_eq!(
        json["createdAt"], "2026-09-01T12:00:00Z",
        "opening time survives transport"
    );
    assert!(
        json.get("body").is_none(),
        "listing responses omit review bodies"
    );

    let json = serde_json::to_value(but_api::branch::json::ListedForgeReview::from(
        &cached_review("feature", 43),
    ))?;
    assert_eq!(
        json["labels"],
        serde_json::json!([]),
        "unlabeled reviews carry an empty list"
    );
    assert!(
        json["author"].is_null() && json["createdAt"].is_null(),
        "missing forge metadata stays absent"
    );
    Ok(())
}

#[test]
fn groups_classifies_and_enriches_from_cache() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    set_project_target_to_feature(&repo)?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    {
        let mut db = ctx.db.get_cache_mut()?;
        but_forge::cache_review(&mut db, &cached_review("feature", 42))?;
    }

    let stacks = but_api::branch::branch_list(&ctx)?;
    let statuses: Vec<(ListedStackStatus, Vec<&str>)> = stacks
        .iter()
        .map(|stack| {
            (
                stack.status,
                stack
                    .branches
                    .iter()
                    .map(|branch| {
                        branch
                            .branch
                            .display_name
                            .to_str()
                            .expect("fixture branch names are valid UTF-8")
                    })
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        format!("{statuses:?}"),
        r#"[(Standalone, ["feature"]), (Applied, ["main"])]"#,
        "the checked-out branch is the applied ad-hoc stack, everything else is standalone, \
         ordered by recency rather than by status"
    );

    let feature = &stacks[0].branches[0];
    assert!(feature.branch.has_local, "feature exists as a local branch");
    assert_eq!(
        feature.branch.commit_count,
        Some(0),
        "feature sits on the target commit and contributes nothing on top"
    );
    let review = feature
        .review
        .as_ref()
        .expect("the cached review is associated by branch name");
    assert_eq!(review.number, 42, "the seeded review number is returned");
    assert_eq!(
        (
            review.title.as_str(),
            review.html_url.as_str(),
            review.unit_symbol.as_str()
        ),
        ("Review of feature", "https://example.com/pull/42", "#"),
        "the listing carries what it takes to display and open the review"
    );
    assert!(
        !review.draft && review.closed_at.is_none() && !review.is_merged(),
        "the cached review is open"
    );

    let main = &stacks[1].branches[0];
    assert!(
        main.review.is_none(),
        "no cached review exists for the main branch"
    );
    assert_eq!(
        main.branch.commits_ahead_of_target,
        Some(1),
        "main has one commit on top of the target"
    );
    Ok(())
}
