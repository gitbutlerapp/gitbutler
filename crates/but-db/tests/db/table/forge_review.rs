use but_db::ForgeReview;

use crate::table::in_memory_db;

#[test]
fn list_all_empty() -> anyhow::Result<()> {
    let db = in_memory_db();

    let reviews = db.forge_reviews().list_all()?;
    assert!(reviews.is_empty());

    Ok(())
}

#[test]
fn set_all_and_read() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review1 = forge_review(1, "First PR", "feature-branch");
    let review2 = forge_review(2, "Second PR", "fix-branch");

    db.forge_reviews_mut()?
        .set_all(vec![review1.clone(), review2.clone()])?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 2);
    assert!(reviews.contains(&review1));
    assert!(reviews.contains(&review2));

    Ok(())
}

#[test]
fn set_all_replaces_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review1 = forge_review(1, "First PR", "feature-branch");
    let review2 = forge_review(2, "Second PR", "fix-branch");
    let review3 = forge_review(3, "Third PR", "new-feature");

    db.forge_reviews_mut()?
        .set_all(vec![review1.clone(), review2.clone()])?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 2);

    db.forge_reviews_mut()?.set_all(vec![review3.clone()])?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0], review3);

    Ok(())
}

#[test]
fn set_all_empty_clears_table() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review1 = forge_review(1, "First PR", "feature-branch");
    db.forge_reviews_mut()?.set_all(vec![review1])?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 1);

    db.forge_reviews_mut()?.set_all(vec![])?;

    let reviews = db.forge_reviews().list_all()?;
    assert!(reviews.is_empty());

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review1 = forge_review(1, "First PR", "feature-branch");
    let review2 = forge_review(2, "Second PR", "fix-branch");

    let mut trans = db.transaction()?;
    trans
        .forge_reviews_mut()?
        .set_all(vec![review1.clone(), review2.clone()])?;

    let reviews = trans.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 2);

    trans.commit()?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 2);
    assert_eq!(reviews, [review1, review2]);

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review1 = forge_review(1, "First PR", "feature-branch");
    db.forge_reviews_mut()?.set_all(vec![review1.clone()])?;

    let mut trans = db.transaction()?;
    trans.forge_reviews_mut()?.set_all(vec![])?;
    trans.rollback()?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0], review1);

    Ok(())
}

#[test]
fn handles_optional_fields() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let review = ForgeReview {
        html_url: "https://github.com/owner/repo/pull/1".to_string(),
        number: 1,
        title: "Test PR".to_string(),
        body: None,
        author: None,
        labels: "[]".to_string(),
        draft: false,
        source_branch: "feature".to_string(),
        target_branch: "main".to_string(),
        sha: "abc123".to_string(),
        created_at: None,
        modified_at: None,
        merged_at: None,
        closed_at: None,
        repository_ssh_url: None,
        repository_https_url: None,
        repo_owner: None,
        reviewers: "[]".to_string(),
        unit_symbol: "test".to_string(),
        last_sync_at: chrono::DateTime::from_timestamp(1000000, 0).unwrap().naive_utc(),
        struct_version: 1,
    };

    db.forge_reviews_mut()?.set_all(vec![review.clone()])?;

    let reviews = db.forge_reviews().list_all()?;
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0], review);

    Ok(())
}

fn forge_review(number: i64, title: &str, source_branch: &str) -> ForgeReview {
    ForgeReview {
        html_url: format!("https://github.com/owner/repo/pull/{number}"),
        number,
        title: title.to_string(),
        body: Some("PR body".to_string()),
        author: Some("test-author".to_string()),
        labels: "[]".to_string(),
        draft: false,
        source_branch: source_branch.to_string(),
        target_branch: "main".to_string(),
        sha: "abc123def456".to_string(),
        created_at: Some(chrono::DateTime::from_timestamp(1000000, 0).unwrap().naive_utc()),
        modified_at: Some(chrono::DateTime::from_timestamp(1000100, 0).unwrap().naive_utc()),
        merged_at: None,
        closed_at: None,
        repository_ssh_url: Some("git@github.com:owner/repo.git".to_string()),
        repository_https_url: Some("https://github.com/owner/repo.git".to_string()),
        repo_owner: Some("owner".to_string()),
        reviewers: "[]".to_string(),
        unit_symbol: "test".to_string(),
        last_sync_at: chrono::DateTime::from_timestamp(1000000, 0).unwrap().naive_utc(),
        struct_version: 1,
    }
}
