use but_forge::{ForgeReview, list_cached_forge_reviews};

fn review(number: i64, source_branch: &str, last_sync_at: chrono::NaiveDateTime) -> ForgeReview {
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
        last_sync_at,
    }
}

fn test_db() -> (tempfile::TempDir, but_db::DbHandle) {
    let tmp = tempfile::tempdir().unwrap();
    let db = but_db::DbHandle::new_in_directory(tmp.path()).unwrap();
    (tmp, db)
}

#[test]
fn current_version_corruption_is_an_error() {
    let (_tmp, mut db) = test_db();
    let now = chrono::Local::now().naive_local();
    let mut corrupt: but_db::ForgeReview = review(1, "corrupt", now).try_into().unwrap();
    corrupt.labels = "not-json".to_string();
    db.forge_reviews_mut()
        .unwrap()
        .set_all(vec![corrupt])
        .unwrap();

    assert!(
        list_cached_forge_reviews(&db).is_err(),
        "current-version corruption must not be treated as a cache miss"
    );
}
